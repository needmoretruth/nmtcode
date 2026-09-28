//! `nmtcode read`: find the symbols in a PNG image, decode them and present their records.

use std::borrow::Cow;
use std::ffi::OsString;
use std::fmt::Write as _;
use std::fs::OpenOptions;
use std::io::{IsTerminal, Read, Write};
use std::path::{Path, PathBuf};

use nmtcode::{
    DecodeError, DecodeOptions, Decoded, DecodedRecord, Outcome as SpecOutcome, PresentAs,
    ValueNotice,
};

use crate::args::{OptionSpec, UsageError, parse};
use crate::json::Json;
use crate::text::{self, msg};
use crate::{Failure, Outcome};

const SPECS: &[OptionSpec] = &[
    OptionSpec { long: "help", short: Some('h'), takes_value: false },
    OptionSpec { long: "out", short: None, takes_value: true },
    OptionSpec { long: "json", short: None, takes_value: false },
];

/// Longest saved file name in bytes, below the 255-byte limit of common file systems.
const MAX_NAME_BYTES: usize = 200;

/// Characters that are never kept in a saved file name: path separators, and the characters
/// Windows file systems reserve (`:` would name an alternate data stream).
const REPLACED: [char; 9] = ['/', '\\', ':', '*', '?', '"', '<', '>', '|'];

/// The name a file record is saved under: the name from its file name record without
/// directories, control characters, reserved characters and leading dots, at most
/// [`MAX_NAME_BYTES`] long; `fallback` when nothing is left.
pub fn sanitise_file_name(name: Option<&str>, fallback: &str) -> String {
    // The record's name has already lost its directory part by the rule of 3.4.3; the
    // separators are replaced here as well so that this function is safe on its own.
    let cleaned: String = name
        .unwrap_or_default()
        .chars()
        .filter(|c| !c.is_control())
        .map(|c| if REPLACED.contains(&c) { '_' } else { c })
        .collect();
    let trimmed = cleaned.trim().trim_start_matches('.').trim_start();
    let mut end = trimmed.len().min(MAX_NAME_BYTES);
    while !trimmed.is_char_boundary(end) {
        end -= 1;
    }
    let short = trimmed.get(..end).unwrap_or_default().trim_end();
    if short.is_empty() { fallback.to_owned() } else { short.to_owned() }
}

/// Text for a terminal: control characters other than line feed and tab, the Unicode
/// characters that reorder text on screen and the invisible characters of chapter 3 (3.4.3) are
/// shown as `\u{…}` so that a record cannot move the cursor, change colours, hide text or
/// disguise a URL. Text for a pipe or file is left as it is.
fn for_terminal(text: &str, terminal: bool) -> Cow<'_, str> {
    let hidden = |c: char| {
        (c.is_control() && c != '\n' && c != '\t')
            || matches!(
                c,
                '\u{00AD}'
                    | '\u{061C}'
                    | '\u{180E}'
                    | '\u{200B}'..='\u{200F}'
                    | '\u{202A}'..='\u{202E}'
                    | '\u{2060}'..='\u{2064}'
                    | '\u{2066}'..='\u{2069}'
                    | '\u{FEFF}'
            )
    };
    if !terminal || !text.chars().any(hidden) {
        return Cow::Borrowed(text);
    }
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if hidden(c) {
            let _ = write!(out, "\\u{{{:x}}}", u32::from(c));
        } else {
            out.push(c);
        }
    }
    Cow::Owned(out)
}

fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(2 * bytes.len());
    for byte in bytes {
        let _ = write!(out, "{byte:02x}");
    }
    out
}

/// Whether a record is saved as a file with `--out`, rather than printed as text.
const fn is_saved(present_as: PresentAs) -> bool {
    matches!(present_as, PresentAs::Bytes | PresentAs::Unknown | PresentAs::File | PresentAs::Psbt)
}

const fn present_as_name(present_as: PresentAs) -> &'static str {
    match present_as {
        PresentAs::Bytes => "bytes",
        PresentAs::Text => "text",
        PresentAs::Url => "url",
        PresentAs::File => "file",
        PresentAs::Psbt => "psbt",
        PresentAs::FileName => "file_name",
        PresentAs::Unknown => "unknown",
    }
}

const fn outcome_name(outcome: SpecOutcome) -> &'static str {
    match outcome {
        SpecOutcome::Damaged => "damaged",
        SpecOutcome::Unsupported => "unsupported",
        SpecOutcome::Malformed => "malformed",
        SpecOutcome::Presented => "presented",
        SpecOutcome::PresentedBaseOnly => "presented_base_only",
        SpecOutcome::PresentedWithError => "presented_with_error",
    }
}

/// What happened to a record that is saved with `--out`.
enum Saved {
    /// No `--out` was given.
    NotAsked,
    /// Saved at this path.
    At(PathBuf),
    /// Refused or failed; the message is on standard error.
    Failed,
}

/// Saves `bytes` as `dir/name`, never replacing an existing file.
fn save(dir: &Path, name: &str, bytes: &[u8]) -> Result<PathBuf, String> {
    let path = dir.join(name);
    // `create_new` fails when the name exists, also as a link, so nothing is ever overwritten
    // or written through a link.
    let mut file =
        OpenOptions::new().write(true).create_new(true).open(&path).map_err(|error| {
            if error.kind() == std::io::ErrorKind::AlreadyExists {
                msg::exists(&path)
            } else {
                msg::cannot_write(&path, &error)
            }
        })?;
    file.write_all(bytes).map_err(|error| msg::cannot_write(&path, &error))?;
    Ok(path)
}

/// Everything `read` reports, collected before printing.
struct Report {
    stdout: String,
    stderr: Vec<String>,
    json: Vec<Json>,
    failed: bool,
}

impl Report {
    fn note(&mut self, prefix: &str, line: impl AsRef<str>) {
        self.stderr.push(format!("{prefix}{}", line.as_ref()));
    }
}

fn record_json(record: &DecodedRecord, file_name: Option<&str>, saved: &Saved) -> Json {
    let mut fields = vec![
        ("type", Json::Number(u64::from(record.content_type.0))),
        ("type_name", Json::optional(record.content_type.name())),
        ("present_as", Json::string(present_as_name(record.present_as))),
        ("length", Json::Number(u64::try_from(record.value.len()).unwrap_or(u64::MAX))),
    ];
    if let Some(text) = record.text() {
        fields.push(("text", Json::string(text)));
    } else {
        fields.push(("hex", Json::String(hex(&record.value))));
    }
    if is_saved(record.present_as) {
        fields.push(("file_name", Json::optional(file_name)));
        let saved = match saved {
            Saved::At(path) => Json::String(path.display().to_string()),
            Saved::NotAsked | Saved::Failed => Json::Null,
        };
        fields.push(("saved", saved));
    }
    let notice = record.notice.map(|notice| match notice {
        ValueNotice::NotUtf8 => "not_utf8",
        ValueNotice::NotPsbt => "not_psbt",
    });
    fields.push(("notice", Json::optional(notice)));
    Json::Object(fields)
}

fn present(
    report: &mut Report,
    index: usize,
    count: usize,
    decoded: &Decoded,
    out: Option<&Path>,
    terminal: bool,
) {
    let prefix = if count > 1 { msg::symbol_prefix(index) } else { String::new() };
    if let Some(notice) = decoded.notice {
        report.note(&prefix, format!("{}: {}", notice.name(), text::spec_error(notice)));
    }
    let mut records = Vec::new();
    for (position, record) in decoded.records.iter().enumerate() {
        let name = decoded.file_name_for(position);
        let fallback = msg::fallback_file_name((count > 1).then_some(index), position);
        let file_name = sanitise_file_name(name.as_deref(), &fallback);
        match record.notice {
            Some(ValueNotice::NotUtf8) => report.note(&prefix, msg::NOT_UTF8),
            Some(ValueNotice::NotPsbt) => report.note(&prefix, msg::NOT_PSBT),
            None => {}
        }
        let mut saved = Saved::NotAsked;
        match record.present_as {
            PresentAs::Text | PresentAs::Url => {
                let text = record.text().unwrap_or_default();
                report.stdout.push_str(&for_terminal(text, terminal));
                report.stdout.push('\n');
            }
            PresentAs::FileName => {}
            PresentAs::Bytes | PresentAs::Unknown | PresentAs::File | PresentAs::Psbt => {
                if record.present_as == PresentAs::Unknown {
                    report.note(
                        &prefix,
                        msg::unknown_type(record.content_type.0, record.value.len()),
                    );
                }
                if matches!(record.present_as, PresentAs::Bytes | PresentAs::Unknown) {
                    report.stdout.push_str(&hex(&record.value));
                    report.stdout.push('\n');
                }
                if let Some(dir) = out {
                    saved = match save(dir, &file_name, &record.value) {
                        Ok(path) => {
                            report.note(&prefix, msg::saved(&path, record.value.len()));
                            Saved::At(path)
                        }
                        Err(message) => {
                            report.note(&prefix, message);
                            report.failed = true;
                            Saved::Failed
                        }
                    };
                } else if matches!(record.present_as, PresentAs::File | PresentAs::Psbt) {
                    report.note(&prefix, msg::file_not_saved(&file_name, record.value.len()));
                }
            }
        }
        let shown_name = is_saved(record.present_as).then_some(file_name.as_str());
        records.push(record_json(record, shown_name, &saved));
    }
    report.json.push(Json::Object(vec![
        ("width", Json::Number(u64::from(decoded.format.width()))),
        ("height", Json::Number(u64::from(decoded.format.height()))),
        ("level", Json::Number(u64::from(decoded.format.level()))),
        ("colour_profile", Json::Number(u64::from(decoded.format.colour_profile()))),
        ("codec", Json::Number(u64::from(decoded.codec))),
        ("dictionary", Json::Number(u64::from(decoded.dictionary))),
        ("corrected", Json::Number(u64::try_from(decoded.corrected).unwrap_or(u64::MAX))),
        ("outcome", Json::string(outcome_name(decoded.outcome))),
        ("notice", Json::optional(decoded.notice.map(nmtcode::SpecError::name))),
        ("records", Json::Array(records)),
    ]));
}

fn failure(report: &mut Report, index: usize, count: usize, error: DecodeError) {
    let prefix = if count > 1 { msg::symbol_prefix(index) } else { String::new() };
    let message = text::spec_error(error.error());
    report.note(&prefix, format!("{}: {message}", error.name()));
    report.failed = true;
    report.json.push(Json::Object(vec![(
        "error",
        Json::Object(vec![
            ("name", Json::string(error.name())),
            ("outcome", Json::string(outcome_name(error.outcome()))),
            ("message", Json::string(message)),
        ]),
    )]));
}

fn read_input(path: &Path) -> Result<Vec<u8>, Failure> {
    if path == Path::new("-") {
        let mut bytes = Vec::new();
        std::io::stdin()
            .lock()
            .read_to_end(&mut bytes)
            .map_err(|error| Failure::Other(msg::cannot_read_stdin(&error)))?;
        Ok(bytes)
    } else {
        std::fs::read(path).map_err(|error| Failure::Other(msg::cannot_read(path, &error)))
    }
}

/// Runs `nmtcode read` with the arguments after `read`.
pub fn run(args: &[OsString]) -> Result<Outcome, Failure> {
    let parsed = parse(args, SPECS)?;
    if parsed.flag("help")? {
        return Ok(Outcome::Help(text::READ_HELP));
    }
    let json = parsed.flag("json")?;
    let out = parsed.value("out")?.map(PathBuf::from);
    let path = match parsed.positional.as_slice() {
        [] => return Err(UsageError(msg::READ_NEEDS_PATH.to_owned()).into()),
        [path] => PathBuf::from(path),
        [_, ..] => return Err(UsageError(msg::READ_ONE_PATH.to_owned()).into()),
    };
    if let Some(dir) = &out
        && !dir.is_dir()
    {
        return Err(Failure::Other(msg::not_a_directory(dir)));
    }
    let bytes = read_input(&path)?;
    let detection = nmtcode_detect::detect_png(&bytes)
        .map_err(|error| Failure::Other(text::detect_error(&error)))?;
    let grids = detection.symbols;
    if grids.is_empty() && detection.nested == 0 {
        return Err(Failure::Other(msg::NOT_FOUND.to_owned()));
    }
    // Nested symbols (specification 5.11) are reported once, after the others: neither the
    // inner nor the outer symbol is presented.
    let count = grids.len() + usize::from(detection.nested > 0);

    let terminal = !json && std::io::stdout().is_terminal();
    let mut report =
        Report { stdout: String::new(), stderr: Vec::new(), json: Vec::new(), failed: false };
    let options = DecodeOptions::default();
    for (index, grid) in grids.iter().enumerate() {
        match nmtcode::decode(grid, &options) {
            Ok(decoded) => {
                present(&mut report, index, count, &decoded, out.as_deref(), terminal);
            }
            Err(error) => failure(&mut report, index, count, error),
        }
    }
    if detection.nested > 0 {
        failure(&mut report, grids.len(), count, nmtcode::SpecError::NestedSymbol.into());
    }

    let mut stdout = std::io::stdout().lock();
    let written = if json {
        let mut text = String::new();
        Json::Object(vec![("symbols", Json::Array(report.json))]).write(&mut text);
        text.push('\n');
        stdout.write_all(text.as_bytes())
    } else {
        stdout.write_all(report.stdout.as_bytes())
    };
    written
        .and_then(|()| stdout.flush())
        .map_err(|error| Failure::Other(msg::cannot_write_stdout(&error)))?;
    let mut stderr = std::io::stderr().lock();
    for line in &report.stderr {
        let _ = writeln!(stderr, "{}{line}", text::ERROR_PREFIX);
    }
    Ok(if report.failed { Outcome::Failed } else { Outcome::Done })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_names_lose_every_dangerous_part() {
        let fallback = "nmtcode-1.bin";
        for (name, expected) in [
            (Some("report.pdf"), "report.pdf"),
            (Some(".bashrc"), "bashrc"),
            (Some("...hidden"), "hidden"),
            (Some(".."), fallback),
            (Some("."), fallback),
            (Some("a/b\\c"), "a_b_c"),
            (Some("C:evil"), "C_evil"),
            (Some("bell\u{7}\nname\u{0}"), "bellname"),
            (Some("  . spaced  "), "spaced"),
            (Some(""), fallback),
            (None, fallback),
            (Some("안녕.txt"), "안녕.txt"),
        ] {
            assert_eq!(sanitise_file_name(name, fallback), expected, "{name:?}");
        }
        let long = "é".repeat(150);
        let short = sanitise_file_name(Some(&long), fallback);
        assert!(short.len() <= MAX_NAME_BYTES && long.starts_with(&short));
    }

    #[test]
    fn terminal_text_hides_control_characters() {
        assert_eq!(for_terminal("a\u{1b}[2Jb\n", true), "a\\u{1b}[2Jb\n");
        assert_eq!(for_terminal("https://e.org/\u{202e}gpj", true), "https://e.org/\\u{202e}gpj");
        assert_eq!(for_terminal("a\u{1b}b", false), "a\u{1b}b");
        assert_eq!(for_terminal("tab\there", true), "tab\there");
        assert_eq!(for_terminal("pay\u{200B}pal.com", true), "pay\\u{200b}pal.com");
        assert_eq!(for_terminal("a\u{FEFF}b\u{00AD}c", true), "a\\u{feff}b\\u{ad}c");
    }
}
