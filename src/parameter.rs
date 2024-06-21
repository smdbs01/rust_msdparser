use std::fmt;
use std::io::{self, Write};
use std::vec::Vec;

/// Custom error type for MSD parameters.
#[derive(Debug)]
pub enum MSDParameterError {
    IoError(io::Error),
    SerializeError(String),
}

impl fmt::Display for MSDParameterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MSDParameterError::IoError(e) => write!(f, "IO Error: {}", e),
            MSDParameterError::SerializeError(e) => write!(f, "Serialize Error: {}", e),
        }
    }
}

impl From<io::Error> for MSDParameterError {
    fn from(e: io::Error) -> Self {
        MSDParameterError::IoError(e)
    }
}

impl From<String> for MSDParameterError {
    fn from(e: String) -> Self {
        MSDParameterError::SerializeError(e)
    }
}

/// An MSD parameter, comprised of a key and some values (usually one).
/// 
/// Stringifying an `MSDParameter` converts it back into MSD, escaping
/// any backslashes `\\` or special substrings.
#[derive(Debug, Clone, PartialEq, Hash, PartialOrd)]
pub struct MSDParameter {
    pub components: Vec<String>,
}

impl MSDParameter {
    const MUST_ESCAPE: [&'static str; 3] = ["//", ":", ";"];

    pub fn new(components: Vec<String>) -> Self {
        Self { components }
    }

    /// The first MSD component, the part immediately after the `#` sign.
    /// 
    /// Returns `None` if `self.components` is an empty vector.
    /// ([`parse_msd`] will never produce such a parameter).
    /// 
    /// [`parse_msd`]: ../parser/fn.parse_msd.html
    pub fn key(&self) -> Option<String> {
        self.components.get(0).map(|s| s.clone())
    }
    
    /// The second MSD component, seperated from the key by a `:`
    /// 
    /// Returns `None` if the parameter ends after the key with no `:`.
    /// This rarely happens in practice and is typically treated the same as a blank value.
    pub fn value(&self) -> Option<String> {
        self.components.get(1).map(|s| s.clone())
    }

    /// Serialize an MSD component (key or value).
    /// 
    /// By default, backslashes (`\\`) and special substrings (`:`, `;`, and `//`) are escaped.
    /// Setting `escapes` to `false` will return the component unchanged, unless it contains a special substring,
    /// in which case an error is returned.
    /// 
    /// # Errors
    /// 
    /// Returns an error if `component` contains a special substring and `escapes` is false.
    pub fn serialize_component(component: &str, escapes: bool) -> Result<String, MSDParameterError> {
        if escapes {
            // Escape all special characters
            // Handle double backslashes first to avoid double escaping
            let mut result = component.to_string().replace("\\", "\\\\");
            for &esc in Self::MUST_ESCAPE.iter() {
                result = result.replace(&esc, &format!("\\{}", esc));
            }
            Ok(result)
        } else if Self::MUST_ESCAPE.iter().any(|&esc| component.contains(esc)) {
            Err(MSDParameterError::SerializeError(format!("{} can't be serialized without escapes", component)))
        } else {
            Ok(component.to_string())
        }
    }

    /// Serialize the key/value pair to MSD, including the surrounding `#:;` characters.
    /// 
    /// By default, backslashes (`\\`) and special substrings (`:`, `;`, and `//`) are escaped.
    /// Setting `escapes` to `false` will return the component unchanged, unless it contains a special substring,
    /// in which case an error is returned.
    /// 
    /// # Errors
    /// 
    /// Returns an error if `component` contains a special substring and `escapes` is false.
    pub fn serialize<W: Write>(&self, writer: &mut W, escapes: bool) -> Result<(), MSDParameterError> {
        writer.write_all(b"#")?;
        for (i, component) in self.components.iter().enumerate() {
            writer.write_all(Self::serialize_component(component, escapes)?.as_bytes())?;
            if i != self.components.len() - 1 {
                writer.write_all(b":")?;
            }
        }
        writer.write_all(b";")?;
        Ok(())
    }

    /// An alternative to the `to_string` method, allowing for the `escapes` parameter.
    ///
    /// See [Serialize](struct.MSDParameter.html#method.serialize)
    /// 
    /// # Errors
    /// 
    /// Returns an error if `component` contains a special substring and `escapes` is false.
    pub fn to_string_with_escapes(&self, escapes: bool) -> Result<String, MSDParameterError> {
        let mut output = Vec::new();
        self.serialize(&mut output, escapes)?;
        Ok(String::from_utf8_lossy(&output).to_string())
    }
}

impl fmt::Display for MSDParameter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut output = Vec::new();
        self.serialize(&mut output, true).map_err(|_e| fmt::Error)?;
        write!(f, "{}", String::from_utf8_lossy(&output))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constructor() {
        let param = MSDParameter::new(vec!["key".to_string(), "value".to_string()]);

        assert_eq!("key", param.key().unwrap_or("missing key".to_string()));
        assert_eq!("value", param.value().unwrap_or("missing value".to_string()));
        assert_eq!(param.components[0], "key");
        assert_eq!(param.components[1], "value");
    }

    #[test]
    fn test_key_without_value() {
        let param = MSDParameter::new(vec!["key".to_string()]);

        assert_eq!("key", param.key().unwrap_or("missing key".to_string()));
        assert!(param.value().is_none());
    }

    #[test]
    fn test_str_with_escapes() {
        let param = MSDParameter::new(vec!["key".to_string(), "value".to_string()]);
        let evil_param = MSDParameter::new(vec!["ABC:DEF;GHI//JKL\\MNO".to_string(), "abc:def;ghi//jkl\\mno".to_string()]);

        assert_eq!("#key:value;", param.to_string());
        assert_eq!(
            "#ABC\\:DEF\\;GHI\\//JKL\\\\MNO:abc\\:def\\;ghi\\//jkl\\\\mno;",
            evil_param.to_string()
        )
    }

    #[test]
    fn test_str_without_escapes() -> Result<(), MSDParameterError> {
        let param = MSDParameter::new(vec!["key".to_string(), "value".to_string()]);
        let multi_value_param = MSDParameter::new(vec!["key".to_string(), "abc".to_string(), "def".to_string()]);
        let param_with_literal_backslashes = MSDParameter::new(vec!["ABC\\DEF".to_string(), "abc\\def".to_string()]);

        let invalid_params = vec![
            MSDParameter::new(vec!["ABC:DEF".to_string(), "abcdef".to_string()]),
            MSDParameter::new(vec!["ABC;DEF".to_string(), "abcdef".to_string()]),
            MSDParameter::new(vec!["ABCDEF".to_string(), "abc;def".to_string()]),
            MSDParameter::new(vec!["ABC//DEF".to_string(), "abcdef".to_string()]),
            MSDParameter::new(vec!["ABCDEF".to_string(), "abc//def".to_string()]),
        ];

        assert_eq!("#key:value;", param.to_string_with_escapes(false)?);
        assert_eq!("#key:abc:def;", multi_value_param.to_string_with_escapes(false)?);
        assert_eq!("#ABC\\DEF:abc\\def;", param_with_literal_backslashes.to_string_with_escapes(false)?);
        for param in invalid_params {
            assert!(param.serialize(&mut Vec::new(), false).is_err());
        }

        Ok(())
    }

}