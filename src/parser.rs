use std::{error, fmt};
use std::io::Read;

use crate::lexer::{lex_msd, MSDLexer, MSDToken, MSDTokenMatch};
use crate::parameter::MSDParameter;

/// Custom error type for MSD parsing.
#[derive(Debug, PartialEq, Clone, Hash, PartialOrd)]
pub struct MSDParserError(pub String);

impl fmt::Display for MSDParserError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "MSDParserError: {}", self.0)
    }
}

impl error::Error for MSDParserError {}

/// Parser for MSD data.
/// 
/// Implements the [`Iterator`] trait of type [`Result<MSDParameter, MSDParserError>`].
#[derive(Debug, Clone)]
pub struct MSDParser<R> {
    ignored_stray_text: bool,

    components: Vec<String>,
    inside_parameter: bool,
    last_key: Option<String>,
    tokens: MSDLexer<R>,
}

impl <R: Read> fmt::Display for MSDParser<R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f, 
            "MSDParser: {{\n\tignored_stray_text: {},\n\tcomponents: {:?},\n\tinside_parameter: {},\n\tlast_key: {:?},\n}}", 
            self.ignored_stray_text,
            self.components,
            self.inside_parameter,
            self.last_key
        )
    }
}

impl <R: Read> MSDParser<R> {
    /// Create a new parser from a reader.
    /// 
    /// `escapes` indicates whether or not to escape special text.
    /// `ignore_stray_text` indicates whether or not to ignore stray text.
    pub fn new(reader: R, escapes: bool, ignore_stray_text: bool) -> Self {
        Self {
            ignored_stray_text: ignore_stray_text,

            components: Vec::new(),
            inside_parameter: false,
            last_key: None,
            
            tokens: {lex_msd(reader, escapes)},
        }
    }

    /// Get the next [`MSDParameter`] from the stream. 
    /// 
    /// [`MSDParameter`]: ../parameter/struct.MSDParameter.html
    /// 
    /// # Errors
    /// 
    /// Returns an error if a stray text token is encountered and `ignore_stray_text` is `false`.
    pub fn next_parameter(&mut self) -> Option<Result<MSDParameter, MSDParserError>> {
        while let Some(MSDTokenMatch { token, text }) = self.tokens.next() {
            // println!("{} {}", token, text);
            match token {
                MSDToken::Text | MSDToken::Escape => {
                    let escaped_text = if token == MSDToken::Escape {
                        text[1..].to_owned()
                    } else { 
                        text.to_owned() 
                    };

                    if self.inside_parameter {
                        if let Some(last_component) = self.components.last_mut() {
                            last_component.push_str(&escaped_text);
                        }
                    } else if !self.ignored_stray_text {
                        if !text.trim().is_empty() && text != "\u{feff}" {
                            let at_location = if let Some(key) = &self.last_key {
                                format!("after '{}' parameter", key)
                            } else {
                                "at start of document".to_string()
                            };

                            if let Some(first_char) = text.trim_start().chars().next() {
                                return Some(
                                    Err(MSDParserError(format!("stray '{}' encountered {}", first_char, at_location)))
                                );
                            } else {
                                // Unreachable?
                                return Some(Err(MSDParserError(format!("stray text {} encountered {}", text, at_location))));
                            }
                        }
                    }
                },
                MSDToken::StartParameter => {
                    if self.inside_parameter {
                        let parameter = MSDParameter::new(self.components.drain(..).collect());

                        self.last_key = parameter.key();

                        self.inside_parameter = true;
                        self.components.push(String::new());
                        return Some(Ok(parameter));
                    }

                    self.inside_parameter = true;
                    self.components.push(String::new());
                },
                MSDToken::EndParameter => if self.inside_parameter {
                    let parameter = MSDParameter::new(self.components.drain(..).collect());

                    self.last_key = parameter.key();
                    self.inside_parameter = false;
                    return Some(Ok(parameter));
                },
                MSDToken::NextComponent => if self.inside_parameter {
                    self.inside_parameter = true;
                    self.components.push(String::new());
                },
                MSDToken::Comment => {},
                // _ => Err(MSDParserError(format!("Unexpected token: {:?}", token)))?
            }
        };

        // Handle missing `;` at the end of the input
        if self.inside_parameter {
            let parameter = MSDParameter::new(self.components.drain(..).collect());
            self.last_key = parameter.key();
            self.inside_parameter = false;
            return Some(Ok(parameter));
        }

        None
    }
}

impl <R: Read> Iterator for MSDParser<R> {
    type Item = Result<MSDParameter, MSDParserError>;

    /// Get the next parameter.
    /// 
    /// See [`MSDParser::next_parameter`].
    fn next(&mut self) -> Option<Self::Item> {
        self.next_parameter()
    }   
}

/// Parse an MSD document from a reader.
/// 
/// `escapes` indicates whether or not to escape special text.
/// `ignore_stray_text` indicates whether or not to ignore stray text.
/// 
/// Returns an [`MSDParser`], which is an [`Iterator`] of type [`Result<MSDParameter, MSDParserError>`].
/// 
/// See also [`MSDParser`] and [`MSDParameter`].
/// 
/// [`MSDParser`]: struct.MSDParser.html
/// [`MSDParameter`]: ../parameter/struct.MSDParameter.html
/// 
/// # Examples
/// 
/// ```rust
/// # use msdparser::{MSDParameter, parse_msd};
/// # use std::error::Error;
/// # use std::vec::Vec;
/// # fn main() -> Result<(), Box<dyn Error>> {
/// #
/// let example_input = b"\
/// #VERSION:0.83;
/// #TITLE:Springtime;
/// #SUBTITLE:;
/// #ARTIST:Kommisar;";
/// let mut result: Vec<MSDParameter> = Vec::new();
/// 
/// // here we set `escapes` to true and `ignore_stray_text` to false
/// // which is the default value in the original python library
/// for parameter in parse_msd(example_input.as_ref(), true, false) {
///     match parameter {
///         Ok(parameter) => result.push(parameter),
///         Err(e) => return Err(Box::new(e)),
///     }   
/// }
/// 
/// assert_eq!(result.len(), 4);
/// assert_eq!(result[0].key().ok_or("missing key".to_string())?, "VERSION".to_string());
/// assert_eq!(result[1].value().ok_or("missing value".to_string())?, "Springtime".to_string());
/// assert_eq!(result[2].value().ok_or("missing value".to_string())?, "".to_string());
/// assert_eq!(result[3].key().ok_or("missing key".to_string())?, "ARTIST".to_string());
/// #
/// #   Ok(())
/// # }
/// ```
/// 
/// Below is an example of stray text resulting in an error:
/// 
/// ```rust
/// # use msdparser::{MSDParameter, parse_msd, MSDParserError};
/// # use std::error::Error;
/// # fn main() -> Result<(), Box<dyn Error>> {
/// #
/// let example_input = b"\
/// #A:B;
/// C:D;";
/// 
/// let mut parser = parse_msd(example_input.as_ref(), true, false);
/// 
/// assert_eq!(parser.next(), Some(Ok(MSDParameter::new(vec!["A".to_string(), "B".to_string()]))));
/// assert_eq!(parser.next(), Some(Err(MSDParserError("stray 'C' encountered after 'A' parameter".to_string()))));
/// #
/// #   Ok(())
/// # }
/// ```
/// 
pub fn parse_msd<R: Read>(input: R, escapes: bool, ignore_stray_text: bool) -> MSDParser<R> {
    MSDParser::new(input, escapes, ignore_stray_text)
}


#[cfg(test)]
mod tests {
    use std::{fs, path::Path};

    use super::*;

    fn get_next_parameter(parser: &mut MSDParser<&[u8]>) -> Option<MSDParameter> {
        parser.next().map(|p| p.unwrap_or(MSDParameter::new(Vec::new())))
    }

    #[test]
    fn test_empty() {
        let input = b"";
        let mut parser = parse_msd(input.as_ref(), true, false);
        assert_eq!(None, get_next_parameter(&mut parser));
    }

    #[test]
    fn test_normal_characters() {
        let input = b"#A1,./'\"[]{\\\\}|`~!@#$%^&*()-_=+ \r\n\t:A1,./'\"[]{\\\\}|`~!@#$%^&*()-_=+ \r\n\t:;";
        let mut parser = parse_msd(input.as_ref(), true, false);
        
        let param = get_next_parameter(&mut parser).unwrap();
        
        let expected = vec![
            "A1,./'\"[]{\\}|`~!@#$%^&*()-_=+ \r\n\t".to_string(),
            "A1,./'\"[]{\\}|`~!@#$%^&*()-_=+ \r\n\t".to_string(),
            "".to_string(),
        ];

        assert_eq!(expected, param.components);
    }

    #[test]
    fn test_comment() {
        let input = b"#A// comment //\r\nBC:D// ; \nEF;//#NO:PE;";
        let mut parser = parse_msd(input.as_ref(), true, false);

        let param = get_next_parameter(&mut parser).unwrap();
        let expected = vec!["A\r\nBC".to_string(), "D\nEF".to_string()];
        assert_eq!(expected, param.components);
    }

    #[test]
    fn test_comment_with_no_newline_at_eof() {
        let input = b"#ABC:DEF// eof";
        let mut parser = parse_msd(input.as_ref(), true, false);

        assert_eq!(MSDParameter::new(vec!["ABC".to_string(), "DEF".to_string()]), get_next_parameter(&mut parser).unwrap());
        assert_eq!(None, parser.next());
    }

    #[test]
    fn test_empty_key() {
        let input = b"#:ABC;#:DEF;";
        let mut parser = parse_msd(input.as_ref(), true, false);
        
        assert_eq!(MSDParameter::new(vec!["".to_string(), "ABC".to_string()]), get_next_parameter(&mut parser).unwrap());
        assert_eq!(MSDParameter::new(vec!["".to_string(), "DEF".to_string()]), get_next_parameter(&mut parser).unwrap());
        assert_eq!(None, parser.next());
    }

    #[test]
    fn test_empty_value() {
        let input = b"#ABC:;#DEF:;";
        let mut parser = parse_msd(input.as_ref(), true, false);
        
        assert_eq!(MSDParameter::new(vec!["ABC".to_string(), "".to_string()]), get_next_parameter(&mut parser).unwrap());
        assert_eq!(MSDParameter::new(vec!["DEF".to_string(), "".to_string()]), get_next_parameter(&mut parser).unwrap());
        assert_eq!(None, parser.next());
    }

    #[test]
    fn test_missing_value() {
        let input = b"#ABC;#DEF;";
        let mut parser = parse_msd(input.as_ref(), true, false);

        let param = get_next_parameter(&mut parser).unwrap();
        assert_eq!(MSDParameter::new(vec!["ABC".to_string()]), param);
        assert_eq!(None, param.value());
        assert_eq!(MSDParameter::new(vec!["DEF".to_string()]), get_next_parameter(&mut parser).unwrap());
        assert_eq!(None, parser.next());
    }

    #[test]
    fn test_missing_semicolon() {
        let input = b"#A:B\nCD;#E:FGH\n#IJKL// comment\n#M:NOP";
        let mut parser = parse_msd(input.as_ref(), true, false);

        assert_eq!(MSDParameter::new(vec!["A".to_string(), "B\nCD".to_string()]), get_next_parameter(&mut parser).unwrap());
        assert_eq!(MSDParameter::new(vec!["E".to_string(), "FGH\n".to_string()]), get_next_parameter(&mut parser).unwrap());
        assert_eq!(MSDParameter::new(vec!["IJKL\n".to_string()]), get_next_parameter(&mut parser).unwrap());
        assert_eq!(MSDParameter::new(vec!["M".to_string(), "NOP".to_string()]), get_next_parameter(&mut parser).unwrap());
        assert_eq!(None, parser.next());
    }

    #[test]
    fn test_missing_value_and_semicolon() {
        let input = b"#A\n#B\n#C\n";
        let mut parser = parse_msd(input.as_ref(), true, false);

        assert_eq!(MSDParameter::new(vec!["A\n".to_string()]), get_next_parameter(&mut parser).unwrap());
        assert_eq!(MSDParameter::new(vec!["B\n".to_string()]), get_next_parameter(&mut parser).unwrap());
        assert_eq!(MSDParameter::new(vec!["C\n".to_string()]), get_next_parameter(&mut parser).unwrap());
        assert_eq!(None, parser.next());
    }

    #[test]
    fn test_unicode() {
        let input = "#TITLE:実例;\n#ARTIST:楽士;".as_bytes();
        let mut parser = parse_msd(input.as_ref(), true, false);

        assert_eq!(MSDParameter::new(vec!["TITLE".to_string(), "実例".to_string()]), get_next_parameter(&mut parser).unwrap());
        assert_eq!(MSDParameter::new(vec!["ARTIST".to_string(), "楽士".to_string()]), get_next_parameter(&mut parser).unwrap());
        assert_eq!(None, parser.next());
    }

    #[test]
    fn test_stray_text() {
        let input = b"#A:B;n#C:D;";
        let mut parser = parse_msd(input.as_ref(), true, false);

        assert_eq!(MSDParameter::new(vec!["A".to_string(), "B".to_string()]), get_next_parameter(&mut parser).unwrap());
        assert_eq!(MSDParserError("stray 'n' encountered after 'A' parameter".to_string()), parser.next().unwrap().unwrap_err());
    }

    #[test]
    fn test_stray_text_at_start() {
        let input = b"TITLE:oops;";
        let mut parser = parse_msd(input.as_ref(), true, false);

        assert_eq!(MSDParserError("stray 'T' encountered at start of document".to_string()), parser.next().unwrap().unwrap_err());
    }

    #[test]
    fn test_stray_semicolon() {
        let input = b"#A:B;;#C:D;";
        let mut parser = parse_msd(input.as_ref(), true, false);

        assert_eq!(MSDParameter::new(vec!["A".to_string(), "B".to_string()]), get_next_parameter(&mut parser).unwrap());
        assert_eq!(MSDParserError("stray ';' encountered after 'A' parameter".to_string()), parser.next().unwrap().unwrap_err());
    }

    #[test]
    fn test_stray_text_with_ignore_stray_text() {
        let input = b"#A:B;n#C:D;";
        let mut parser = parse_msd(input.as_ref(), true, true);

        assert_eq!(MSDParameter::new(vec!["A".to_string(), "B".to_string()]), get_next_parameter(&mut parser).unwrap());
        assert_eq!(MSDParameter::new(vec!["C".to_string(), "D".to_string()]), get_next_parameter(&mut parser).unwrap());
        assert_eq!(None, parser.next());
    }

    #[test]
    fn test_escapes() {
        let input = b"#A\\:B:C\\;D;#E\\#F:G\\\\H;#LF:\\\nLF;";
        let mut parser = parse_msd(input.as_ref(), true, false);

        assert_eq!(MSDParameter::new(vec!["A:B".to_string(), "C;D".to_string()]), get_next_parameter(&mut parser).unwrap());
        assert_eq!(MSDParameter::new(vec!["E#F".to_string(), "G\\H".to_string()]), get_next_parameter(&mut parser).unwrap());
        assert_eq!(MSDParameter::new(vec!["LF".to_string(), "\nLF".to_string()]), get_next_parameter(&mut parser).unwrap());
        assert_eq!(None, parser.next());
    }

    #[test]
    fn test_no_escapes() {
        let input = b"#A\\:B:C\\;D;#E\\#F:G\\\\H;#LF:\\\nLF;";
        let mut parser = parse_msd(input.as_ref(), false, true);

        assert_eq!(MSDParameter::new(vec!["A\\".to_string(), "B".to_string(), "C\\".to_string()]), get_next_parameter(&mut parser).unwrap());
        assert_eq!(MSDParameter::new(vec!["E\\#F".to_string(), "G\\\\H".to_string()]), get_next_parameter(&mut parser).unwrap());
        assert_eq!(MSDParameter::new(vec!["LF".to_string(), "\\\nLF".to_string()]), get_next_parameter(&mut parser).unwrap());
        assert_eq!(None, parser.next());
    }

    #[test]
    fn test_file() {
        let path = Path::new("testdata/Springtime.ssc");
        let input = fs::read(path).unwrap();
        
        let mut parser: MSDParser<&[u8]> = parse_msd(input.as_ref(), true, false);
        
        assert_eq!(MSDParameter::new(vec!["VERSION".to_string(), "0.83".to_string()]), get_next_parameter(&mut parser).unwrap());
        assert_eq!(MSDParameter::new(vec!["TITLE".to_string(), "Springtime".to_string()]), get_next_parameter(&mut parser).unwrap());
        assert_eq!(MSDParameter::new(vec!["SUBTITLE".to_string(), "".to_string()]), get_next_parameter(&mut parser).unwrap());
    }
}


