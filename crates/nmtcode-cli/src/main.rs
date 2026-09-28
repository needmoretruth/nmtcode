//! The `nmtcode` command: make an NMT Code symbol as PNG or SVG, and read symbols from a PNG.
//!
//! Exit codes: 0 when everything asked for was done; 1 when a symbol could not be made, read or
//! saved; 2 for a usage error.

mod args;
mod json;
mod make;
mod read;
mod text;

use std::ffi::OsString;
use std::io::Write;
use std::process::ExitCode;

use args::UsageError;

/// What a command did, when it did not fail as a whole.
pub enum Outcome {
    /// Everything was done.
    Done,
    /// Print this help or version text on standard output.
    Help(&'static str),
    /// Some part failed; its messages are already on standard error.
    Failed,
}

/// Why a command failed as a whole.
pub enum Failure {
    /// Wrong arguments.
    Usage(UsageError),
    /// Anything else; the message for standard error.
    Other(String),
}

impl From<UsageError> for Failure {
    fn from(error: UsageError) -> Self {
        Self::Usage(error)
    }
}

const EXIT_FAILED: u8 = 1;
const EXIT_USAGE: u8 = 2;

fn run(args: &[OsString]) -> Result<Outcome, Failure> {
    let rest = args.get(1..).unwrap_or_default();
    match args.first().map(|arg| arg.to_string_lossy()) {
        None => Err(UsageError(text::msg::NO_COMMAND.to_owned()).into()),
        Some(command) => match command.as_ref() {
            "--help" | "-h" | "help" => Ok(Outcome::Help(text::MAIN_HELP)),
            "--version" | "-V" => Ok(Outcome::Help(text::VERSION)),
            "make" => make::run(rest),
            "read" => read::run(rest),
            other => Err(UsageError(text::msg::unknown_command(other)).into()),
        },
    }
}

fn main() -> ExitCode {
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    match run(&args) {
        Ok(Outcome::Help(help)) => {
            let mut stdout = std::io::stdout().lock();
            if stdout.write_all(help.as_bytes()).and_then(|()| stdout.flush()).is_err() {
                return ExitCode::from(EXIT_FAILED);
            }
            ExitCode::SUCCESS
        }
        Ok(Outcome::Done) => ExitCode::SUCCESS,
        Ok(Outcome::Failed) => ExitCode::from(EXIT_FAILED),
        Err(Failure::Usage(UsageError(message))) => {
            let _ =
                writeln!(std::io::stderr(), "{}{message}\n{}", text::ERROR_PREFIX, text::SEE_HELP);
            ExitCode::from(EXIT_USAGE)
        }
        Err(Failure::Other(message)) => {
            let _ = writeln!(std::io::stderr(), "{}{message}", text::ERROR_PREFIX);
            ExitCode::from(EXIT_FAILED)
        }
    }
}
