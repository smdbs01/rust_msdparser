use std::{fmt, io::Read};

use regex::Regex;

#[derive(Debug, PartialEq, Clone, Copy, Hash, PartialOrd)]
pub enum MSDToken {
    Text,
    StartParameter,
    NextComponent,
    EndParameter,
    Escape,
    Comment,
}

impl fmt::Display for MSDToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[derive(Debug, Clone)]
struct LexerPattern {
    regex: Regex,
    token_outside_param: MSDToken,
    token_inside_param: MSDToken,
    escapes: Option<bool>,
}

impl LexerPattern {
    fn new(pattern: &str, token_outside: MSDToken, token_inside: MSDToken, escapes: Option<bool>) -> Self {
        Self {
            regex: Regex::new(pattern).unwrap(),
            token_outside_param: token_outside,
            token_inside_param: token_inside,
            escapes,
        }
    }
}

const ESCAPED_TEXT: &str = r"^[^\\\/:;#]+";
const UNESCAPED_TEXT: &str = r"^[^\/:;#]+";
const POUND: &str = r"^#";
const COLON: &str = r"^:";
const SEMICOLON: &str = r"^;";
const ESCAPE: &str = r"^(?s)\\.";
const COMMENT: &str = r"^//[^\r\n]*";
const SLASH: &str = r"^/";

lazy_static::lazy_static! {
    static ref LEXER_PATTERNS: Vec<LexerPattern> = vec![
        LexerPattern::new(ESCAPED_TEXT, MSDToken::Text, MSDToken::Text, Some(true)),
        LexerPattern::new(UNESCAPED_TEXT, MSDToken::Text, MSDToken::Text, Some(false)),
        LexerPattern::new(POUND, MSDToken::StartParameter, MSDToken::Text, None),
        LexerPattern::new(COLON, MSDToken::Text, MSDToken::NextComponent, None),
        LexerPattern::new(SEMICOLON, MSDToken::Text, MSDToken::EndParameter, None),
        LexerPattern::new(ESCAPE, MSDToken::Text, MSDToken::Escape, Some(true)),
        LexerPattern::new(COMMENT, MSDToken::Comment, MSDToken::Comment, None),
        LexerPattern::new(SLASH, MSDToken::Text, MSDToken::Text, None),
    ];
}

/// Buffer size for reading
const BUFFER_SIZE: usize = 4096;

/// Match for a LexerPattern
#[derive(Debug, PartialEq, Clone, Hash, PartialOrd)]
pub struct MSDTokenMatch {
    pub token: MSDToken,
    pub text: String,
}

impl MSDTokenMatch {
    fn new(token: MSDToken, text: String) -> Self {
        Self {
            token,
            text
        }
    }
}

/// Lexer for MSD files.
/// 
/// Implements an [`Iterator`] that yields [`MSDTokenMatch`]s
#[derive(Debug, Clone)]
pub struct MSDLexer<R> {
    reader: R,
    msd_buffer: String,
    read_buffer: [u8; BUFFER_SIZE],
    inside_parameter: bool,
    done_reading: bool,
    last_text_token: Option<String>,
    lexer_patterns: Vec<LexerPattern>
}

impl<R: Read> MSDLexer<R> {
    /// Create a new MSDLexer from a Read instance and whether or not to escape special characters.
    pub fn new(reader: R, escapes: bool) -> Self {
        Self {
            reader,
            
            msd_buffer: String::new(),
            read_buffer: [0; BUFFER_SIZE],

            inside_parameter: false,
            done_reading: false,
            last_text_token: None,
            
            lexer_patterns: {
                LEXER_PATTERNS.iter()
                    .filter(|x| x.escapes == Some(escapes) || x.escapes.is_none())
                    .cloned()
                    .collect()
            },
        }
    }

    /// Read the next token from the input stream.
    /// 
    /// Returns None if the end of the stream has been reached or no patterns match.
    pub fn next_token(&mut self) -> Option<MSDTokenMatch> {
        // End until both stream and buffer are empty
        while !(self.done_reading && self.msd_buffer.is_empty()) {
            // Read the next chunk
            let read = self.reader.read(&mut self.read_buffer).unwrap();

            // End of the stream
            if read == 0 { self.done_reading = true; }

            // Add the next chunk to the buffer
            self.msd_buffer += String::from_utf8_lossy(&self.read_buffer[..read]).as_ref();

            // Enforcing that the MSD buffer always either contains a newline or the rest of the stream,
            // so that comments, escapes, etc. don't get split in half.
            while self.msd_buffer.contains('\n') || self.msd_buffer.contains('\r') || (self.done_reading && self.msd_buffer.len() > 0) {
                for pattern in &self.lexer_patterns {
                    if let Some(m) = pattern.regex.find(&self.msd_buffer) {
                        let matched_text = self.msd_buffer.get(..m.end()).unwrap().to_owned();
                        // Remove the matched section from the buffer
                        self.msd_buffer = self.msd_buffer.get(m.end()..).unwrap().to_string();

                        let mut token = 
                            if self.inside_parameter { pattern.token_inside_param } 
                            else { pattern.token_outside_param };
                        
                        // Recovery from missing `;` at the end of a line
                        if let Some(last_token) = self.last_text_token.clone() {
                            if last_token.ends_with("\n") || last_token.ends_with("\r") {
                                if pattern.regex.as_str() == POUND && token == MSDToken::Text {
                                    token = MSDToken::StartParameter;
                                }
                            }
                        }

                        match token {
                            MSDToken::StartParameter => { self.inside_parameter = true; },
                            MSDToken::EndParameter => { self.inside_parameter = false; },
                            MSDToken::Text => { self.last_text_token = Some(matched_text.to_string()); },
                            _ => {}
                        }
                        
                        return Some(MSDTokenMatch::new(token, matched_text));
                    }
                }
            }
        }
        None
    }
}

impl <R: Read> Iterator for MSDLexer<R> {
    type Item = MSDTokenMatch;

    fn next(&mut self) -> Option<Self::Item> {
        self.next_token()
    }  
}

/// Create a new [`MSDLexer`] from a [`Read`] impl and whether or not to escape special characters.
/// 
/// [`MSDLexer`] is an [`Iterator`] that yields [`MSDTokenMatch`]s, 
/// which consists of a [`MSDToken`] and the matched text.
/// 
/// In practice you don't have to call this function directly, as it is called during [`parse_msd`].
/// 
/// [`parse_msd`]: ../parser/fn.parse_msd.html
pub fn lex_msd<R: Read>(reader: R, escapes: bool) -> MSDLexer<R> {
    MSDLexer::new(reader, escapes)
}


#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    #[test]
    fn test_tokens_with_escapes() {
        let input = "#ABC:DEF\\:GHI;\n#JKL:MNO\nPQR# STU".as_bytes();
        let mut cursor = Cursor::new(input);
        let tokens: Vec<MSDTokenMatch> = lex_msd(&mut cursor, true).collect();
        let expected_tokens = vec![
            MSDTokenMatch::new(MSDToken::StartParameter, "#".to_string()),
            MSDTokenMatch::new(MSDToken::Text, "ABC".to_string()),
            MSDTokenMatch::new(MSDToken::NextComponent, ":".to_string()),
            MSDTokenMatch::new(MSDToken::Text, "DEF".to_string()),
            MSDTokenMatch::new(MSDToken::Escape, "\\:".to_string()),
            MSDTokenMatch::new(MSDToken::Text, "GHI".to_string()),
            MSDTokenMatch::new(MSDToken::EndParameter, ";".to_string()),
            MSDTokenMatch::new(MSDToken::Text, "\n".to_string()),
            MSDTokenMatch::new(MSDToken::StartParameter, "#".to_string()),
            MSDTokenMatch::new(MSDToken::Text, "JKL".to_string()),
            MSDTokenMatch::new(MSDToken::NextComponent, ":".to_string()),
            MSDTokenMatch::new(MSDToken::Text, "MNO\nPQR".to_string()),
            MSDTokenMatch::new(MSDToken::Text, "#".to_string()),
            MSDTokenMatch::new(MSDToken::Text, " STU".to_string()),
        ];

        assert_eq!(tokens, expected_tokens);
    }

    #[test]
    fn test_tokens_without_escapes() {
        let input = "#ABC:DEF\\:GHI;\n#JKL:MNO\nPQR# STU".as_bytes();
        let mut reader = Cursor::new(input);
        let tokens: Vec<MSDTokenMatch> = lex_msd(&mut reader, false).collect();
        let expected_tokens = vec![
            MSDTokenMatch::new(MSDToken::StartParameter, "#".to_string()),
            MSDTokenMatch::new(MSDToken::Text, "ABC".to_string()),
            MSDTokenMatch::new(MSDToken::NextComponent, ":".to_string()),
            MSDTokenMatch::new(MSDToken::Text, "DEF\\".to_string()),
            MSDTokenMatch::new(MSDToken::NextComponent, ":".to_string()),
            MSDTokenMatch::new(MSDToken::Text, "GHI".to_string()),
            MSDTokenMatch::new(MSDToken::EndParameter, ";".to_string()),
            MSDTokenMatch::new(MSDToken::Text, "\n".to_string()),
            MSDTokenMatch::new(MSDToken::StartParameter, "#".to_string()),
            MSDTokenMatch::new(MSDToken::Text, "JKL".to_string()),
            MSDTokenMatch::new(MSDToken::NextComponent, ":".to_string()),
            MSDTokenMatch::new(MSDToken::Text, "MNO\nPQR".to_string()),
            MSDTokenMatch::new(MSDToken::Text, "#".to_string()),
            MSDTokenMatch::new(MSDToken::Text, " STU".to_string()),
        ];

        assert_eq!(expected_tokens, tokens);
    }

    #[test]
    fn test_stray_metacharacters() {
        let input = ":;#A:B;;:#C:D;".as_bytes();
        let mut reader = Cursor::new(input);
        let tokens: Vec<MSDTokenMatch> = lex_msd(&mut reader, true).collect();
        let expected_tokens = vec![
            MSDTokenMatch::new(MSDToken::Text, ":".to_string()),
            MSDTokenMatch::new(MSDToken::Text, ";".to_string()),
            MSDTokenMatch::new(MSDToken::StartParameter, "#".to_string()),
            MSDTokenMatch::new(MSDToken::Text, "A".to_string()),
            MSDTokenMatch::new(MSDToken::NextComponent, ":".to_string()),
            MSDTokenMatch::new(MSDToken::Text, "B".to_string()),
            MSDTokenMatch::new(MSDToken::EndParameter, ";".to_string()),
            MSDTokenMatch::new(MSDToken::Text, ";".to_string()),
            MSDTokenMatch::new(MSDToken::Text, ":".to_string()),
            MSDTokenMatch::new(MSDToken::StartParameter, "#".to_string()),
            MSDTokenMatch::new(MSDToken::Text, "C".to_string()),
            MSDTokenMatch::new(MSDToken::NextComponent, ":".to_string()),
            MSDTokenMatch::new(MSDToken::Text, "D".to_string()),
            MSDTokenMatch::new(MSDToken::EndParameter, ";".to_string()),
        ];

        assert_eq!(expected_tokens, tokens);
    }

    #[test]
    fn test_missing_semicolon() {
        let input = "#A:B\nCD;#E:FGH\n#IJKL// comment\n#M:NOP".as_bytes();
        let mut reader = Cursor::new(input);
        let tokens: Vec<MSDTokenMatch> = lex_msd(&mut reader, true).collect();
        let expected_tokens = vec![
            MSDTokenMatch::new(MSDToken::StartParameter, "#".to_string()),
            MSDTokenMatch::new(MSDToken::Text, "A".to_string()),
            MSDTokenMatch::new(MSDToken::NextComponent, ":".to_string()),
            MSDTokenMatch::new(MSDToken::Text, "B\nCD".to_string()),
            MSDTokenMatch::new(MSDToken::EndParameter, ";".to_string()),
            MSDTokenMatch::new(MSDToken::StartParameter, "#".to_string()),
            MSDTokenMatch::new(MSDToken::Text, "E".to_string()),
            MSDTokenMatch::new(MSDToken::NextComponent, ":".to_string()),
            MSDTokenMatch::new(MSDToken::Text, "FGH\n".to_string()),
            MSDTokenMatch::new(MSDToken::StartParameter, "#".to_string()),
            MSDTokenMatch::new(MSDToken::Text, "IJKL".to_string()),
            MSDTokenMatch::new(MSDToken::Comment, "// comment".to_string()),
            MSDTokenMatch::new(MSDToken::Text, "\n".to_string()),
            MSDTokenMatch::new(MSDToken::StartParameter, "#".to_string()),
            MSDTokenMatch::new(MSDToken::Text, "M".to_string()),
            MSDTokenMatch::new(MSDToken::NextComponent, ":".to_string()),
            MSDTokenMatch::new(MSDToken::Text, "NOP".to_string()),
        ];

        assert_eq!(expected_tokens, tokens);
    }

    #[test]
    fn test_comments() {
        let input = "#A// comment //\r\nBC:D// ; \nEF;//#NO:PE;".as_bytes();
        let mut reader = Cursor::new(input);
        let tokens: Vec<MSDTokenMatch> = lex_msd(&mut reader, true).collect();
        let expected_tokens = vec![
            MSDTokenMatch::new(MSDToken::StartParameter, "#".to_string()),
            MSDTokenMatch::new(MSDToken::Text, "A".to_string()),
            MSDTokenMatch::new(MSDToken::Comment, "// comment //".to_string()),
            MSDTokenMatch::new(MSDToken::Text, "\r\nBC".to_string()),
            MSDTokenMatch::new(MSDToken::NextComponent, ":".to_string()),
            MSDTokenMatch::new(MSDToken::Text, "D".to_string()),
            MSDTokenMatch::new(MSDToken::Comment, "// ; ".to_string()),
            MSDTokenMatch::new(MSDToken::Text, "\nEF".to_string()),
            MSDTokenMatch::new(MSDToken::EndParameter, ";".to_string()),
            MSDTokenMatch::new(MSDToken::Comment, "//#NO:PE;".to_string()),
        ];

        assert_eq!(expected_tokens, tokens);
    }
}