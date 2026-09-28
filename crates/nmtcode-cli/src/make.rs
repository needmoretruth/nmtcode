//! `nmtcode make`: encode content and draw it as PNG or SVG.

use std::ffi::OsString;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use nmtcode::{CodecOptions, EncodeOptions, Profile, SizeConstraints, SizeRule};
use nmtcode_render::{DEFAULT_MODULE_PX, MIN_QUIET_ZONE, RenderOptions};

use crate::args::{OptionSpec, UsageError, parse};
use crate::text::{self, msg};
use crate::{Failure, Outcome};

const fn value(long: &'static str) -> OptionSpec {
    OptionSpec { long, short: None, takes_value: true }
}

const fn flag(long: &'static str) -> OptionSpec {
    OptionSpec { long, short: None, takes_value: false }
}

const SPECS: &[OptionSpec] = &[
    OptionSpec { long: "help", short: Some('h'), takes_value: false },
    value("text"),
    value("url"),
    value("file"),
    OptionSpec { long: "output", short: Some('o'), takes_value: true },
    value("format"),
    value("profile"),
    value("level"),
    value("size"),
    value("max-width"),
    value("max-height"),
    value("module-px"),
    value("dpi"),
    value("quiet-zone"),
    flag("qr"),
    flag("no-qr"),
    value("codec"),
];

/// Module size of the `lowend` profile in pixels: twice the `screen` default, for cameras that
/// see about 2 pixels per module (1.5 gives no number).
const LOWEND_MODULE_PX: u32 = 2 * DEFAULT_MODULE_PX;

/// Printer density assumed by the `print` profile when `--dpi` is not given.
const PRINT_DEFAULT_DPI: u32 = 300;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Format {
    Png,
    Svg,
}

impl Format {
    const fn extension(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Svg => "svg",
        }
    }

    fn from_extension(path: &Path) -> Option<Self> {
        let extension = path.extension()?.to_str()?.to_ascii_lowercase();
        match extension.as_str() {
            "png" => Some(Self::Png),
            "svg" => Some(Self::Svg),
            _ => None,
        }
    }
}

enum Content {
    Text(String),
    Url(String),
    File { name: String, bytes: Vec<u8> },
}

fn positive(text: &str) -> Option<u32> {
    text.parse::<u32>().ok().filter(|&n| n >= 1)
}

fn parse_size(text: &str) -> Option<(u32, u32)> {
    let (width, height) = text.split_once(['x', 'X'])?;
    Some((width.parse().ok()?, height.parse().ok()?))
}

fn parse_codecs(text: &str) -> Option<CodecOptions> {
    let mut codecs = CodecOptions {
        digits: false,
        upper_alphanumeric: false,
        token_model: false,
        hangul: false,
        brotli: false,
    };
    for item in text.split(',').map(str::trim) {
        match item.to_ascii_lowercase().as_str() {
            "0" | "stored" => {}
            "1" | "digits" => codecs.digits = true,
            "2" | "alphanumeric" => codecs.upper_alphanumeric = true,
            "3" | "token" => codecs.token_model = true,
            "4" | "hangul" => codecs.hangul = true,
            "5" | "brotli" => codecs.brotli = true,
            _ => return None,
        }
    }
    Some(codecs)
}

fn parse_profile(text: &str) -> Option<Profile> {
    match text {
        "screen" => Some(Profile::Screen),
        "print" => Some(Profile::Print),
        "lowend" => Some(Profile::LowEnd),
        // Accepted so that the encoder can refuse it with its own error: not implemented.
        "color" => Some(Profile::Color),
        _ => None,
    }
}

fn read_stdin() -> Result<Vec<u8>, Failure> {
    let mut bytes = Vec::new();
    std::io::stdin()
        .lock()
        .read_to_end(&mut bytes)
        .map_err(|error| Failure::Other(msg::cannot_read_stdin(&error)))?;
    Ok(bytes)
}

fn content(parsed: &crate::args::Parsed) -> Result<Content, Failure> {
    let text = parsed.value("text")?;
    let url = parsed.value("url")?;
    let file = parsed.value("file")?;
    if [text.is_some(), url.is_some(), file.is_some()].into_iter().filter(|&given| given).count()
        > 1
    {
        return Err(UsageError(msg::ONE_CONTENT.to_owned()).into());
    }
    if let Some(text) = text {
        let text = text.to_str().ok_or_else(|| UsageError(msg::TEXT_NOT_UTF8.to_owned()))?;
        return Ok(Content::Text(text.to_owned()));
    }
    if let Some(url) = url {
        let url = url.to_str().ok_or_else(|| UsageError(msg::URL_NOT_UTF8.to_owned()))?;
        return Ok(Content::Url(url.to_owned()));
    }
    if let Some(path) = file {
        let path = Path::new(path);
        let bytes =
            std::fs::read(path).map_err(|error| Failure::Other(msg::cannot_read(path, &error)))?;
        let name = match path.file_name() {
            None => String::new(),
            Some(name) => name
                .to_str()
                .ok_or_else(|| UsageError(msg::FILE_NAME_NOT_UTF8.to_owned()))?
                .to_owned(),
        };
        return Ok(Content::File { name, bytes });
    }
    let bytes = read_stdin()?;
    let text =
        String::from_utf8(bytes).map_err(|_| Failure::Other(msg::STDIN_NOT_UTF8.to_owned()))?;
    Ok(Content::Text(text))
}

fn size_rule(parsed: &crate::args::Parsed) -> Result<SizeRule, UsageError> {
    let size = parsed.parsed("size", msg::EXPECT_SIZE, parse_size)?;
    let max_width = parsed.parsed("max-width", msg::EXPECT_POSITIVE, positive)?;
    let max_height = parsed.parsed("max-height", msg::EXPECT_POSITIVE, positive)?;
    match (size, max_width, max_height) {
        (Some(_), Some(_), _) | (Some(_), _, Some(_)) => {
            Err(UsageError(msg::SIZE_WITH_MAXIMUM.to_owned()))
        }
        (Some((width, height)), None, None) => Ok(SizeRule::Exact { width, height }),
        (None, None, None) => Ok(SizeRule::Recommended),
        (None, max_width, max_height) => {
            Ok(SizeRule::Constrained(SizeConstraints { aspect_ratio: None, max_width, max_height }))
        }
    }
}

fn render_options(
    parsed: &crate::args::Parsed,
    profile: Profile,
) -> Result<RenderOptions, Failure> {
    let module_px = parsed.parsed("module-px", msg::EXPECT_POSITIVE, positive)?;
    let dpi = parsed.parsed("dpi", msg::EXPECT_POSITIVE, positive)?;
    let quiet_zone =
        parsed.parsed("quiet-zone", msg::EXPECT_QUIET_ZONE, |text| text.parse::<u32>().ok())?;
    if quiet_zone.is_some_and(|zone| zone < MIN_QUIET_ZONE) {
        return Err(UsageError(msg::QUIET_ZONE_TOO_SMALL.to_owned()).into());
    }
    let mut options = if profile == Profile::Print {
        RenderOptions::print(dpi.unwrap_or(PRINT_DEFAULT_DPI))
            .map_err(|error| Failure::Other(text::render_error(&error)))?
    } else {
        let default_px =
            if profile == Profile::LowEnd { LOWEND_MODULE_PX } else { DEFAULT_MODULE_PX };
        RenderOptions { module_px: default_px, dpi, ..RenderOptions::default() }
    };
    if let Some(module_px) = module_px {
        options.module_px = module_px;
    }
    if let Some(quiet_zone) = quiet_zone {
        options.quiet_zone = quiet_zone;
    }
    // The bootstrap QR Code is on by default only with the print profile (8.5).
    options.bootstrap = match (parsed.flag("qr")?, parsed.flag("no-qr")?) {
        (true, true) => return Err(UsageError(msg::QR_AND_NO_QR.to_owned()).into()),
        (true, false) => true,
        (false, true) => false,
        (false, false) => profile == Profile::Print,
    };
    Ok(options)
}

/// Where the image goes and in which format.
fn output(parsed: &crate::args::Parsed) -> Result<(Option<PathBuf>, Format), UsageError> {
    let format = parsed.parsed("format", msg::EXPECT_FORMAT, |text| {
        match text.to_ascii_lowercase().as_str() {
            "png" => Some(Format::Png),
            "svg" => Some(Format::Svg),
            _ => None,
        }
    })?;
    let path = parsed.value("output")?.filter(|path| *path != "-").map(PathBuf::from);
    match (&path, format) {
        (None, Some(format)) => Ok((None, format)),
        (None, None) => Err(UsageError(msg::FORMAT_FOR_STDOUT.to_owned())),
        (Some(path), format) => {
            let by_extension = Format::from_extension(path);
            match (by_extension, format) {
                (Some(found), Some(asked)) if found != asked => {
                    Err(UsageError(msg::format_conflict(path, asked.extension())))
                }
                (Some(found), _) => Ok((Some(path.clone()), found)),
                (None, Some(asked)) => Ok((Some(path.clone()), asked)),
                (None, None) => Err(UsageError(msg::unknown_extension(path))),
            }
        }
    }
}

/// Runs `nmtcode make` with the arguments after `make`.
pub fn run(args: &[OsString]) -> Result<Outcome, Failure> {
    let parsed = parse(args, SPECS)?;
    if parsed.flag("help")? {
        return Ok(Outcome::Help(text::MAKE_HELP));
    }
    if let Some(extra) = parsed.positional.first() {
        return Err(UsageError(msg::unexpected_argument(&extra.to_string_lossy())).into());
    }
    let profile = parsed.parsed("profile", msg::EXPECT_PROFILE, parse_profile)?.unwrap_or_default();
    let level = parsed.parsed("level", msg::EXPECT_LEVEL, |text| {
        text.parse::<u8>().ok().filter(|&level| level <= 3)
    })?;
    let size = size_rule(&parsed)?;
    let codecs = parsed.parsed("codec", msg::EXPECT_CODEC, parse_codecs)?.unwrap_or_default();
    let render = render_options(&parsed, profile)?;
    let (path, format) = output(&parsed)?;
    let content = content(&parsed)?;

    let options = EncodeOptions { profile, level, size, codecs };
    let symbol = match &content {
        Content::Text(text) => nmtcode::encode_text(text, &options),
        Content::Url(url) => nmtcode::encode_url(url, &options),
        Content::File { name, bytes } => nmtcode::encode_file(name, bytes, &options),
    }
    .map_err(|error| Failure::Other(text::encode_error(&error)))?;

    let image = match format {
        Format::Png => nmtcode_render::render_png(symbol.grid(), &render),
        Format::Svg => nmtcode_render::render_svg(symbol.grid(), &render).map(String::into_bytes),
    }
    .map_err(|error| Failure::Other(text::render_error(&error)))?;

    if let Some(path) = &path {
        std::fs::write(path, &image)
            .map_err(|error| Failure::Other(msg::cannot_write(path, &error)))?;
    } else {
        let mut stdout = std::io::stdout().lock();
        stdout
            .write_all(&image)
            .and_then(|()| stdout.flush())
            .map_err(|error| Failure::Other(msg::cannot_write_stdout(&error)))?;
    }
    let summary = text::made(
        symbol.width(),
        symbol.height(),
        symbol.level(),
        symbol.codec(),
        symbol.container_len(),
        symbol.capacity(),
    );
    let _ = writeln!(std::io::stderr(), "{summary}");
    Ok(Outcome::Done)
}
