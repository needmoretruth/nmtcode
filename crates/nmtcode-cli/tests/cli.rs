//! The built `nmtcode` binary, run as a person or a script would run it.

use std::collections::BTreeSet;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

use nmtcode::{ContentType, EncodeOptions, Record, encode};
use nmtcode_render::{RenderOptions, render_png};
use nmtcode_symbol::{Layout, format_positions};

const BIN: &str = env!("CARGO_BIN_EXE_nmtcode");
const URL: &str = "https://github.com/needmoretruth/nmtcode";

/// A fresh directory under the system temporary directory, removed when dropped.
struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> Self {
        static COUNT: AtomicUsize = AtomicUsize::new(0);
        let count = COUNT.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir()
            .join(format!("nmtcode-cli-test-{}-{name}-{count}", std::process::id()));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn run(args: &[&str]) -> Output {
    Command::new(BIN).args(args).stdin(Stdio::null()).output().unwrap()
}

fn run_with_stdin(args: &[&str], input: &[u8]) -> Output {
    let mut child = Command::new(BIN)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(input).unwrap();
    child.wait_with_output().unwrap()
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).unwrap()
}

fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).unwrap()
}

fn assert_ok(output: &Output) {
    assert!(output.status.success(), "status {:?}, stderr: {}", output.status, stderr(output));
}

fn path_str(path: &Path) -> &str {
    path.to_str().unwrap()
}

/// Makes a PNG with `make_args` and reads it back.
fn make_and_read(dir: &TempDir, make_args: &[&str], read_args: &[&str]) -> Output {
    let png = dir.path("symbol.png");
    let mut args = vec!["make", "-o", path_str(&png)];
    args.extend_from_slice(make_args);
    assert_ok(&run(&args));
    let mut args = vec!["read"];
    args.extend_from_slice(read_args);
    args.push(path_str(&png));
    run(&args)
}

fn files_in(dir: &Path) -> BTreeSet<String> {
    std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect()
}

/// Width and height of a PNG from its IHDR chunk.
fn png_size(png: &[u8]) -> (u32, u32) {
    assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
    assert_eq!(&png[12..16], b"IHDR");
    let word = |at: usize| u32::from_be_bytes(png[at..at + 4].try_into().unwrap());
    (word(16), word(20))
}

#[test]
fn version_and_help() {
    let output = run(&["--version"]);
    assert_ok(&output);
    assert_eq!(stdout(&output).trim(), "nmtcode 0.0.1");
    for args in [&["--help"][..], &["make", "--help"], &["read", "--help"], &["-h"]] {
        let output = run(args);
        assert_ok(&output);
        assert!(stdout(&output).contains("Usage"), "{args:?}");
        assert!(output.stderr.is_empty());
    }
    let make = stdout(&run(&["make", "--help"]));
    for option in [
        "--text",
        "--url",
        "--file",
        "--output",
        "--format",
        "--profile",
        "--level",
        "--size",
        "--max-width",
        "--max-height",
        "--module-px",
        "--dpi",
        "--quiet-zone",
        "--qr",
        "--no-qr",
        "--codec",
    ] {
        assert!(make.contains(option), "make --help lacks {option}");
    }
    assert!(make.contains("QR Code is a registered trademark of DENSO WAVE INCORPORATED."));
    let read = stdout(&run(&["read", "--help"]));
    assert!(read.contains("--out") && read.contains("--json"));
}

#[test]
fn png_round_trips_with_and_without_the_qr_code() {
    let dir = TempDir::new("png");
    for qr in [&["--qr"][..], &[], &["--no-qr"]] {
        let mut args = vec!["--url", URL];
        args.extend_from_slice(qr);
        let output = make_and_read(&dir, &args, &[]);
        assert_ok(&output);
        assert_eq!(stdout(&output), format!("{URL}\n"));

        let mut args = vec!["--text", "안녕하세요 NMT Code\nsecond line"];
        args.extend_from_slice(qr);
        let output = make_and_read(&dir, &args, &[]);
        assert_ok(&output);
        assert_eq!(stdout(&output), "안녕하세요 NMT Code\nsecond line\n");
    }
}

#[test]
fn symbol_options_are_applied() {
    let dir = TempDir::new("options");
    // The text takes a 23-byte container with codec 2 and 27 bytes stored. K (4.6) is 16 at
    // 20 x 20, 24 at 24 x 20 and 20 x 24, 34 at 20 x 28 level 0, 28 at 20 x 28 level 1, 24 at
    // 20 x 36 level 3. The recommended size rule (1.5) picks the smallest area with sides within
    // a factor of 2, and the taller of two equal areas.
    let cases: [(&[&str], u32, u32, u8); 7] = [
        (&["--level", "3"], 20, 36, 3),
        (&["--size", "48x20"], 48, 20, 0),
        (&["--max-height", "20"], 24, 20, 0),
        (&["--max-width", "20"], 20, 24, 0),
        (&["--profile", "print"], 20, 28, 1),
        (&["--profile", "lowend", "--quiet-zone", "5"], 20, 28, 1),
        (&["--profile", "screen", "--module-px", "3", "--codec", "stored"], 20, 28, 0),
    ];
    for (options, width, height, level) in cases {
        let mut args = vec!["--text", "0123456789 ABCDEFGH"];
        args.extend_from_slice(options);
        let output = make_and_read(&dir, &args, &["--json"]);
        assert_ok(&output);
        let json = stdout(&output);
        assert!(
            json.contains(&format!("\"width\":{width},\"height\":{height},\"level\":{level},")),
            "{options:?}: {json}"
        );
        assert!(json.contains("\"text\":\"0123456789 ABCDEFGH\""), "{json}");
    }
    let output = make_and_read(&dir, &["--text", "x", "--codec", "0,digits"], &["--json"]);
    assert!(stdout(&output).contains("\"codec\":0,"));
}

#[test]
fn the_qr_code_is_on_by_default_only_with_print() {
    let dir = TempDir::new("qr-default");
    let png = dir.path("symbol.png");
    let size = |extra: &[&str]| {
        let mut args = vec!["make", "--text", "NMT Code", "--size", "24x24", "-o", path_str(&png)];
        args.extend_from_slice(extra);
        assert_ok(&run(&args));
        png_size(&std::fs::read(&png).unwrap())
    };
    // Screen: 24 modules plus 2 + 2 quiet zone at 4 pixels, no QR Code (8.5).
    assert_eq!(size(&[]), (4 * 28, 4 * 28));
    assert_eq!(size(&["--no-qr"]), (4 * 28, 4 * 28));
    // With the QR Code above the square symbol: its 29 modules, the 4-module gap and its 4-module
    // quiet zone above the symbol (8.4.3).
    assert_eq!(size(&["--qr"]), (4 * 37, 4 * (37 + 24 + 2)));
    // Print: on by default, off with --no-qr.
    assert_eq!(size(&["--profile", "print"]), (5 * 37, 5 * 63));
    assert_eq!(size(&["--profile", "print", "--no-qr"]), (5 * 28, 5 * 28));
    // Both flags at once are a usage error.
    let both = run(&["make", "--text", "x", "--qr", "--no-qr", "-o", path_str(&png)]);
    assert_eq!(both.status.code(), Some(2));
}

#[test]
fn print_dpi_sets_the_module_size() {
    let dir = TempDir::new("dpi");
    let png = dir.path("print.png");
    let make = ["make", "--profile", "print", "--dpi", "600", "--no-qr", "--size", "32x32"];
    let mut args = make.to_vec();
    args.extend_from_slice(&["--text", "print", "-o", path_str(&png)]);
    assert_ok(&run(&args));
    // 0.4 mm at 600 dpi is 9.45 dots, so 10 dots per module; 32 modules plus 2 + 2 quiet zone.
    assert_eq!(png_size(&std::fs::read(&png).unwrap()), (360, 360));
    let output = run(&["read", path_str(&png)]);
    assert_ok(&output);
    assert_eq!(stdout(&output), "print\n");
    // Without --dpi the print profile assumes 300 dpi: 5 dots per module.
    let mut args = vec!["make", "--profile", "print", "--no-qr", "--size", "32x32"];
    args.extend_from_slice(&["--text", "print", "-o", path_str(&png)]);
    assert_ok(&run(&args));
    assert_eq!(png_size(&std::fs::read(&png).unwrap()), (180, 180));
}

#[test]
fn standard_input_and_output() {
    let made = run_with_stdin(&["make", "--format", "png"], "from standard input".as_bytes());
    assert_ok(&made);
    let read = run_with_stdin(&["read", "-"], &made.stdout);
    assert_ok(&read);
    assert_eq!(stdout(&read), "from standard input\n");
    let not_text = run_with_stdin(&["make", "--format", "png"], &[0xFF, 0xFE]);
    assert_eq!(not_text.status.code(), Some(1));
}

/// A minimal XML well-formedness check: one root element, balanced tags, quoted attributes
/// without duplicates, and no raw `<` or bare `&` in text or attribute values.
fn check_xml(xml: &str) -> Result<String, String> {
    fn check_text(text: &str) -> Result<(), String> {
        let mut rest = text;
        while let Some(at) = rest.find('&') {
            let after = &rest[at + 1..];
            let end = after.find(';').ok_or("unterminated entity")?;
            let entity = &after[..end];
            let known = ["amp", "lt", "gt", "quot", "apos"].contains(&entity)
                || entity.strip_prefix('#').is_some_and(|n| n.chars().all(|c| c.is_ascii_digit()));
            if !known {
                return Err(format!("unknown entity &{entity};"));
            }
            rest = &after[end + 1..];
        }
        if text.contains('<') { Err("raw < in text".into()) } else { Ok(()) }
    }
    let name_char = |c: char| c.is_ascii_alphanumeric() || matches!(c, ':' | '_' | '-' | '.');
    let mut rest = xml;
    if let Some(prolog) = rest.strip_prefix("<?xml") {
        rest = &prolog[prolog.find("?>").ok_or("unterminated prolog")? + 2..];
    }
    let mut stack: Vec<&str> = Vec::new();
    let mut root = None;
    while let Some(at) = rest.find('<') {
        let text = &rest[..at];
        if stack.is_empty() && !text.trim().is_empty() {
            return Err("text outside the root".into());
        }
        check_text(text)?;
        rest = &rest[at + 1..];
        if let Some(close) = rest.strip_prefix('/') {
            let end = close.find('>').ok_or("unterminated end tag")?;
            let name = close[..end].trim_end();
            if stack.pop() != Some(name) {
                return Err(format!("mismatched </{name}>"));
            }
            rest = &close[end + 1..];
            continue;
        }
        if let Some(comment) = rest.strip_prefix("!--") {
            rest = &comment[comment.find("-->").ok_or("unterminated comment")? + 3..];
            continue;
        }
        let name_len = rest.find(|c: char| !name_char(c)).ok_or("unterminated tag")?;
        let name = &rest[..name_len];
        if name.is_empty() {
            return Err("empty tag name".into());
        }
        if stack.is_empty() {
            if root.is_some() {
                return Err("two root elements".into());
            }
            root = Some(name.to_owned());
        }
        rest = &rest[name_len..];
        let mut attributes = BTreeSet::new();
        loop {
            rest = rest.trim_start();
            if let Some(after) = rest.strip_prefix("/>") {
                rest = after;
                break;
            }
            if let Some(after) = rest.strip_prefix('>') {
                stack.push(name);
                rest = after;
                break;
            }
            let attr_len = rest.find(|c: char| !name_char(c)).ok_or("unterminated attribute")?;
            let attr = &rest[..attr_len];
            if attr.is_empty() || !attributes.insert(attr) {
                return Err(format!("bad or repeated attribute in <{name}>"));
            }
            let value =
                rest[attr_len..].trim_start().strip_prefix('=').ok_or("attribute without =")?;
            let value = value.trim_start();
            let quote =
                value.chars().next().filter(|c| matches!(c, '"' | '\'')).ok_or("unquoted")?;
            let end = value[1..].find(quote).ok_or("unterminated value")?;
            check_text(&value[1..=end])?;
            rest = &value[end + 2..];
        }
    }
    if !stack.is_empty() || !rest.trim().is_empty() {
        return Err("unclosed elements or trailing text".into());
    }
    root.ok_or_else(|| "no root element".into())
}

#[test]
fn svg_output_is_well_formed() {
    assert!(check_xml("<a><b x=\"1\"/></a>").is_ok());
    for broken in
        ["<a><b></a></b>", "<a x=1></a>", "<a x=\"1\" x=\"2\"/>", "<a>&nope;</a>", "<a/><b/>"]
    {
        assert!(check_xml(broken).is_err(), "{broken}");
    }
    let dir = TempDir::new("svg");
    for extra in [&[][..], &["--no-qr"], &["--profile", "print", "--dpi", "1200"]] {
        let svg = dir.path("symbol.svg");
        let mut args = vec!["make", "--text", "<&> \"quoted\" 'text'", "-o", path_str(&svg)];
        args.extend_from_slice(extra);
        assert_ok(&run(&args));
        let text = std::fs::read_to_string(&svg).unwrap();
        assert_eq!(check_xml(&text), Ok("svg".to_owned()), "{extra:?}");
        assert!(text.contains("viewBox=") && !text.contains("<script"));
    }
    let made = run(&["make", "--url", URL, "--format", "svg"]);
    assert_ok(&made);
    assert_eq!(check_xml(&stdout(&made)), Ok("svg".to_owned()));
}

#[test]
fn files_are_saved_only_with_out() {
    let dir = TempDir::new("file");
    let source = dir.path("report.bin");
    let bytes: Vec<u8> = (0..=255).collect();
    std::fs::write(&source, &bytes).unwrap();
    let out = TempDir::new("file-out");

    let without = make_and_read(&dir, &["--file", path_str(&source)], &[]);
    assert_ok(&without);
    assert!(without.stdout.is_empty());
    assert!(stderr(&without).contains("not saved"));
    assert!(files_in(&out.0).is_empty());

    let with = make_and_read(&dir, &["--file", path_str(&source)], &["--out", path_str(&out.0)]);
    assert_ok(&with);
    assert_eq!(files_in(&out.0), BTreeSet::from(["report.bin".to_owned()]));
    assert_eq!(std::fs::read(out.path("report.bin")).unwrap(), bytes);

    let json = make_and_read(&dir, &["--file", path_str(&source)], &["--json"]);
    assert_ok(&json);
    let json = stdout(&json);
    assert!(json.contains("\"present_as\":\"file_name\""), "{json}");
    assert!(json.contains("\"file_name\":\"report.bin\",\"saved\":null"), "{json}");
    let missing = run(&["read", "--out", path_str(&dir.path("missing")), path_str(&source)]);
    assert_eq!(missing.status.code(), Some(1));
}

#[test]
fn existing_files_are_never_overwritten() {
    let dir = TempDir::new("overwrite");
    let source = dir.path("notes.txt");
    std::fs::write(&source, b"new content").unwrap();
    let out = TempDir::new("overwrite-out");
    std::fs::write(out.path("notes.txt"), b"old content").unwrap();

    let output = make_and_read(&dir, &["--file", path_str(&source)], &["--out", path_str(&out.0)]);
    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).contains("already exists"), "{}", stderr(&output));
    assert_eq!(std::fs::read(out.path("notes.txt")).unwrap(), b"old content");
    assert_eq!(files_in(&out.0).len(), 1);

    // A link with the name is not followed either.
    #[cfg(unix)]
    {
        let target = dir.path("target.txt");
        std::fs::write(&target, b"target").unwrap();
        let linked = TempDir::new("overwrite-link");
        std::os::unix::fs::symlink(&target, linked.path("notes.txt")).unwrap();
        let args = ["--out", path_str(&linked.0)];
        let output = make_and_read(&dir, &["--file", path_str(&source)], &args);
        assert_eq!(output.status.code(), Some(1));
        assert_eq!(std::fs::read(&target).unwrap(), b"target");
    }
}

#[test]
fn malicious_file_names_stay_inside_the_directory() {
    let dir = TempDir::new("names");
    let names: [(&str, &str); 9] = [
        ("../../escape.txt", "escape.txt"),
        ("/etc/passwd", "passwd"),
        ("..\\..\\windows.txt", "windows.txt"),
        (".bashrc", "bashrc"),
        ("..", "nmtcode-10.bin"),
        ("\u{1b}[31mred\u{7}.txt", "[31mred.txt"),
        ("C:alternate", "C_alternate"),
        ("a\nb\u{0}c.txt", "abc.txt"),
        ("   ", "nmtcode-18.bin"),
    ];
    let mut records = Vec::new();
    for (name, _) in &names {
        records.push(Record { content_type: ContentType::FILE_NAME, value: name.as_bytes() });
        records.push(Record { content_type: ContentType::FILE, value: b"payload" });
    }
    let symbol = encode(&records, &EncodeOptions::default()).unwrap();
    let options = RenderOptions { bootstrap: false, ..RenderOptions::default() };
    let png = dir.path("names.png");
    std::fs::write(&png, render_png(symbol.grid(), &options).unwrap()).unwrap();

    let out = TempDir::new("names-out");
    let before = files_in(&dir.0);
    let output = run(&["read", "--out", path_str(&out.0), path_str(&png)]);
    assert_ok(&output);
    let expected: BTreeSet<String> = names.iter().map(|(_, saved)| (*saved).to_owned()).collect();
    assert_eq!(files_in(&out.0), expected);
    assert_eq!(files_in(&dir.0), before, "nothing is written next to the output directory");
    for saved in &expected {
        assert_eq!(std::fs::read(out.path(saved)).unwrap(), b"payload");
    }
    assert!(!stderr(&output).contains('\u{1b}'), "no control character reaches the terminal");
}

/// Damages the data modules of `symbol` beyond correction, or its format copies.
fn damaged_png(dir: &TempDir, damage_format: bool) -> PathBuf {
    let symbol = nmtcode::encode_url(URL, &EncodeOptions::default()).unwrap();
    let mut grid = symbol.grid().clone();
    if damage_format {
        let positions = format_positions(grid.width(), grid.height()).unwrap();
        for copy in positions {
            for &(x, y) in &copy[..4] {
                grid.toggle(x, y);
            }
        }
    } else {
        let layout = Layout::new(grid.width(), grid.height()).unwrap();
        // Every other data module: far more than the 4 codewords that level 0 corrects here.
        for (x, y) in layout.placement().step_by(2) {
            grid.toggle(x, y);
        }
    }
    let path = dir.path(if damage_format { "format.png" } else { "data.png" });
    std::fs::write(&path, render_png(&grid, &RenderOptions::default()).unwrap()).unwrap();
    path
}

#[test]
fn damaged_images_give_an_error_and_a_non_zero_exit_code() {
    let dir = TempDir::new("damaged");
    let data = run(&["read", path_str(&damaged_png(&dir, false))]);
    assert_eq!(data.status.code(), Some(1));
    assert!(data.stdout.is_empty());
    let message = stderr(&data);
    assert!(message.contains("E_ECC_FAILED") || message.contains("E_CRC_MISMATCH"), "{message}");

    let format = run(&["read", path_str(&damaged_png(&dir, true))]);
    assert_eq!(format.status.code(), Some(1));
    assert!(stderr(&format).contains("E_FORMAT_UNREADABLE"), "{}", stderr(&format));
    let json = run(&["read", "--json", path_str(&damaged_png(&dir, true))]);
    assert_eq!(json.status.code(), Some(1));
    assert!(
        stdout(&json)
            .contains("\"error\":{\"name\":\"E_FORMAT_UNREADABLE\",\"outcome\":\"damaged\"")
    );

    // A truncated PNG file.
    let good = dir.path("good.png");
    assert_ok(&run(&["make", "--url", URL, "-o", path_str(&good)]));
    let bytes = std::fs::read(&good).unwrap();
    let cut = dir.path("cut.png");
    std::fs::write(&cut, &bytes[..bytes.len() / 2]).unwrap();
    let output = run(&["read", path_str(&cut)]);
    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).contains("PNG"), "{}", stderr(&output));
    // Not a PNG, and an image without a symbol.
    let text = dir.path("text.png");
    std::fs::write(&text, b"not an image").unwrap();
    assert_eq!(run(&["read", path_str(&text)]).status.code(), Some(1));
}

#[test]
fn usage_errors_exit_with_2() {
    let dir = TempDir::new("usage");
    let png = dir.path("x.png");
    let png = path_str(&png);
    for args in [
        &[][..],
        &["frobnicate"],
        &["make", "--nope"],
        &["make", "--text", "a", "--url", "b", "-o", png],
        &["make", "--text", "a", "--quiet-zone", "1", "-o", png],
        &["make", "--text", "a", "--size", "20x20", "--max-width", "40", "-o", png],
        &["make", "--text", "a", "--level", "4", "-o", png],
        &["make", "--text", "a", "--size", "twenty", "-o", png],
        &["make", "--text", "a", "--codec", "zip", "-o", png],
        &["make", "--text", "a", "-o", "x.gif"],
        &["make", "--text", "a", "-o", png, "--format", "svg"],
        &["make", "--text"],
        &["read"],
        &["read", "a.png", "b.png"],
    ] {
        let output = run(args);
        assert_eq!(output.status.code(), Some(2), "{args:?}: {}", stderr(&output));
        assert!(stderr(&output).starts_with("nmtcode: "), "{args:?}");
    }
    // Refusals that are not usage errors: exit code 1 with the reason.
    let colour = run(&["make", "--profile", "color", "--text", "a", "-o", png]);
    assert_eq!(colour.status.code(), Some(1));
    assert!(stderr(&colour).contains("not implemented"));
    let too_small = run(&["make", "--url", URL, "--size", "20x20", "-o", png]);
    assert_eq!(too_small.status.code(), Some(1));
    assert!(stderr(&too_small).contains("holds 16"), "{}", stderr(&too_small));
}

/// Specification 5.11: a symbol drawn inside another is reported as `E_NESTED_SYMBOL`, and
/// neither symbol's content is shown.
#[test]
fn nested_symbols_are_an_error_and_show_nothing() {
    let dir = TempDir::new("nested");
    let options = EncodeOptions {
        size: nmtcode::SizeRule::Exact { width: 96, height: 96 },
        ..EncodeOptions::default()
    };
    let outer = nmtcode::encode_text("outer content", &options).unwrap();
    let inner = nmtcode::encode_text("inner content", &EncodeOptions::default()).unwrap();
    let (iw, ih) = (inner.width(), inner.height());
    let mut grid = outer.grid().clone();
    for y in 0..ih + 4 {
        for x in 0..iw + 4 {
            grid.set(36 + x, 36 + y, false);
        }
    }
    for y in 0..ih {
        for x in 0..iw {
            grid.set(38 + x, 38 + y, inner.grid().get(x, y).unwrap());
        }
    }
    let path = dir.path("nested.png");
    std::fs::write(&path, render_png(&grid, &RenderOptions::default()).unwrap()).unwrap();
    let output = run(&["read", path_str(&path)]);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty(), "{}", stdout(&output));
    assert!(stderr(&output).contains("E_NESTED_SYMBOL"), "{}", stderr(&output));
    let json = run(&["read", "--json", path_str(&path)]);
    assert!(
        stdout(&json).contains("\"error\":{\"name\":\"E_NESTED_SYMBOL\",\"outcome\":\"malformed\""),
        "{}",
        stdout(&json)
    );
}

/// Specification 1.4: a reversed image (light modules on dark) is read from its inverse.
#[test]
fn a_reversed_image_is_read() {
    let dir = TempDir::new("reversed");
    let symbol = nmtcode::encode_url(URL, &EncodeOptions::default()).unwrap();
    let png = render_png(symbol.grid(), &RenderOptions::default()).unwrap();
    let mut decoder = png::Decoder::new(std::io::Cursor::new(png));
    decoder.set_transformations(png::Transformations::EXPAND);
    let mut reader = decoder.read_info().unwrap();
    let mut pixels = vec![0u8; reader.output_buffer_size().unwrap()];
    let frame = reader.next_frame(&mut pixels).unwrap();
    assert_eq!(
        (frame.color_type, frame.bit_depth),
        (png::ColorType::Grayscale, png::BitDepth::Eight)
    );
    for v in &mut pixels {
        *v = 255 - *v;
    }
    let mut out = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut out, frame.width, frame.height);
        encoder.set_color(png::ColorType::Grayscale);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(&pixels[..frame.buffer_size()]).unwrap();
    }
    let path = dir.path("reversed.png");
    std::fs::write(&path, out).unwrap();
    let output = run(&["read", path_str(&path)]);
    assert_ok(&output);
    assert!(stdout(&output).contains(URL), "{}", stdout(&output));
}
