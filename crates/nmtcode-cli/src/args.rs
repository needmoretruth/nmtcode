//! A small command-line parser on the standard library: long options with a separate or `=`
//! value, `-o` and `-h`, `--` to end the options, and `-` as an argument.

use std::ffi::{OsStr, OsString};

use crate::text::msg;

/// A usage error: the message for standard error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsageError(pub String);

/// One option a command accepts.
#[derive(Debug, Clone, Copy)]
pub struct OptionSpec {
    /// The long name without `--`.
    pub long: &'static str,
    /// The short name without `-`, if any.
    pub short: Option<char>,
    /// Whether the option takes a value.
    pub takes_value: bool,
}

/// The parsed command line of one command.
#[derive(Debug, Default)]
pub struct Parsed {
    options: Vec<(&'static str, Option<OsString>)>,
    /// The arguments that are not options, in order.
    pub positional: Vec<OsString>,
}

impl Parsed {
    /// The value of option `long`, if given.
    ///
    /// Fails when the option is given more than once.
    pub fn value(&self, long: &str) -> Result<Option<&OsStr>, UsageError> {
        let mut values = self.options.iter().filter(|(name, _)| *name == long);
        let first = values.next();
        if values.next().is_some() {
            return Err(UsageError(msg::repeated(&format!("--{long}"))));
        }
        Ok(first.and_then(|(_, value)| value.as_deref()))
    }

    /// Whether flag `long` is given.
    pub fn flag(&self, long: &str) -> Result<bool, UsageError> {
        let count = self.options.iter().filter(|(name, _)| *name == long).count();
        if count > 1 {
            return Err(UsageError(msg::repeated(&format!("--{long}"))));
        }
        Ok(count == 1)
    }

    /// The value of option `long` parsed by `parse`, which gives `None` for an invalid value.
    pub fn parsed<T>(
        &self,
        long: &str,
        expected: &str,
        parse: impl Fn(&str) -> Option<T>,
    ) -> Result<Option<T>, UsageError> {
        let Some(value) = self.value(long)? else {
            return Ok(None);
        };
        let invalid = || {
            UsageError(msg::invalid_value(&format!("--{long}"), &value.to_string_lossy(), expected))
        };
        let text = value.to_str().ok_or_else(invalid)?;
        parse(text).map(Some).ok_or_else(invalid)
    }
}

/// Parses `args` (without the program and command names) against `specs`.
pub fn parse(args: &[OsString], specs: &[OptionSpec]) -> Result<Parsed, UsageError> {
    let mut parsed = Parsed::default();
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        let Some(text) = arg.to_str() else {
            parsed.positional.push(arg.clone());
            continue;
        };
        if text == "--" {
            parsed.positional.extend(rest.cloned());
            break;
        }
        let (spec, inline) = if let Some(long) = text.strip_prefix("--") {
            let (name, inline) = match long.split_once('=') {
                Some((name, value)) => (name, Some(OsString::from(value))),
                None => (long, None),
            };
            let spec = specs
                .iter()
                .find(|spec| spec.long == name)
                .ok_or_else(|| UsageError(msg::unknown_option(&format!("--{name}"))))?;
            (spec, inline)
        } else if text.len() > 1 && text.starts_with('-') {
            let mut chars = text.chars().skip(1);
            let (Some(short), None) = (chars.next(), chars.next()) else {
                return Err(UsageError(msg::unknown_option(text)));
            };
            let spec = specs
                .iter()
                .find(|spec| spec.short == Some(short))
                .ok_or_else(|| UsageError(msg::unknown_option(text)))?;
            (spec, None)
        } else {
            parsed.positional.push(arg.clone());
            continue;
        };
        let value = match (spec.takes_value, inline) {
            (true, Some(value)) => Some(value),
            (true, None) => Some(
                rest.next()
                    .cloned()
                    .ok_or_else(|| UsageError(msg::missing_value(&format!("--{}", spec.long))))?,
            ),
            (false, Some(_)) => {
                return Err(UsageError(msg::unexpected_value(&format!("--{}", spec.long))));
            }
            (false, None) => None,
        };
        parsed.options.push((spec.long, value));
    }
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SPECS: &[OptionSpec] = &[
        OptionSpec { long: "output", short: Some('o'), takes_value: true },
        OptionSpec { long: "no-qr", short: None, takes_value: false },
        OptionSpec { long: "help", short: Some('h'), takes_value: false },
    ];

    fn args(items: &[&str]) -> Vec<OsString> {
        items.iter().map(OsString::from).collect()
    }

    #[test]
    fn values_flags_and_positionals() {
        let parsed =
            parse(&args(&["-o", "a.png", "--no-qr", "x", "-", "--", "--no-qr"]), SPECS).unwrap();
        assert_eq!(parsed.value("output").unwrap(), Some(OsStr::new("a.png")));
        assert!(parsed.flag("no-qr").unwrap());
        assert!(!parsed.flag("help").unwrap());
        assert_eq!(parsed.positional, args(&["x", "-", "--no-qr"]));
        let parsed = parse(&args(&["--output=b.svg"]), SPECS).unwrap();
        assert_eq!(parsed.value("output").unwrap(), Some(OsStr::new("b.svg")));
    }

    #[test]
    fn errors() {
        assert!(parse(&args(&["--nope"]), SPECS).is_err());
        assert!(parse(&args(&["-x"]), SPECS).is_err());
        assert!(parse(&args(&["-oa"]), SPECS).is_err());
        assert!(parse(&args(&["--output"]), SPECS).is_err());
        assert!(parse(&args(&["--no-qr=1"]), SPECS).is_err());
        let parsed = parse(&args(&["-o", "a", "--output", "b"]), SPECS).unwrap();
        assert!(parsed.value("output").is_err());
    }
}
