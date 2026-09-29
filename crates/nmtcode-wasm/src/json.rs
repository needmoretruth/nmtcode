//! A minimal JSON writer (RFC 8259) for the reading result: objects keep the order their
//! fields are added in, and strings escape every character that JSON requires.

use core::fmt::Write as _;

/// A JSON value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Json {
    /// `null`.
    Null,
    /// `true` or `false`.
    Bool(bool),
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

    /// A number from a `usize`, saturating at `u64::MAX`.
    pub fn count(value: usize) -> Self {
        Self::Number(u64::try_from(value).unwrap_or(u64::MAX))
    }

    /// The value as JSON text.
    pub fn to_text(&self) -> String {
        let mut out = String::new();
        self.write(&mut out);
        out
    }

    fn write(&self, out: &mut String) {
        match self {
            Self::Null => out.push_str("null"),
            Self::Bool(value) => out.push_str(if *value { "true" } else { "false" }),
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

/// Writes `text` as a JSON string. Quotes, backslashes and U+0000 to U+001F are escaped as
/// RFC 8259 requires; U+2028 and U+2029 are escaped too so the text is also valid JavaScript.
fn write_string(text: &str, out: &mut String) {
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if u32::from(c) < 0x20 || c == '\u{2028}' || c == '\u{2029}' => {
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
            ("a", Json::string("q\"b\\n\n\u{1}é\u{2028}")),
            ("b", Json::Array(vec![Json::Number(1), Json::Null, Json::Bool(true)])),
            ("c", Json::optional(None::<String>)),
        ]);
        assert_eq!(value.to_text(), r#"{"a":"q\"b\\n\n\u0001é\u2028","b":[1,null,true],"c":null}"#);
    }
}
