//! A minimal JSON writer for `nmtcode read --json` (RFC 8259): objects are written in the
//! order their fields are added.

use std::fmt::Write as _;

/// A JSON value.
#[derive(Debug, Clone, PartialEq)]
pub enum Json {
    /// `null`.
    Null,
    /// A non-negative integer.
    Number(u64),
    /// A string.
    String(String),
    /// An array.
    Array(Vec<Json>),
    /// An object, fields in order.
    Object(Vec<(&'static str, Json)>),
}

impl Json {
    /// A string value.
    pub fn string(text: impl Into<String>) -> Self {
        Self::String(text.into())
    }

    /// A string value, or `null`.
    pub fn optional(text: Option<impl Into<String>>) -> Self {
        text.map_or(Self::Null, |text| Self::String(text.into()))
    }

    /// Writes the value into `out`.
    pub fn write(&self, out: &mut String) {
        match self {
            Self::Null => out.push_str("null"),
            Self::Number(value) => {
                let _ = write!(out, "{value}");
            }
            Self::String(text) => write_string(text, out),
            Self::Array(items) => {
                out.push('[');
                for (index, item) in items.iter().enumerate() {
                    if index > 0 {
                        out.push(',');
                    }
                    item.write(out);
                }
                out.push(']');
            }
            Self::Object(fields) => {
                out.push('{');
                for (index, (name, value)) in fields.iter().enumerate() {
                    if index > 0 {
                        out.push(',');
                    }
                    write_string(name, out);
                    out.push(':');
                    value.write(out);
                }
                out.push('}');
            }
        }
    }
}

/// Writes `text` as a JSON string: quotes, backslashes and control characters escaped.
fn write_string(text: &str, out: &mut String) {
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if u32::from(c) < 0x20 || c == '\u{7F}' => {
                let _ = write!(out, "\\u{:04x}", u32::from(c));
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_and_nesting() {
        let value = Json::Object(vec![
            ("a", Json::string("q\"b\\n\n\u{1}é")),
            ("b", Json::Array(vec![Json::Number(1), Json::Null])),
            ("c", Json::optional(None::<String>)),
        ]);
        let mut out = String::new();
        value.write(&mut out);
        assert_eq!(out, r#"{"a":"q\"b\\n\n\u0001é","b":[1,null],"c":null}"#);
    }
}
