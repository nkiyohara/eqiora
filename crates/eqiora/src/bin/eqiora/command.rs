use std::cell::Cell;
use std::ffi::OsString;
use std::fs::File;
use std::io::Read;
use std::path::Path;

use eqiora::Diagnostic;
use eqiora::api::{ModelDocument, StructuralSemanticFingerprint};

use super::terminal;

const SOURCE_LIMIT_BYTES: usize = 8_388_608;
const SOURCE_READ_CEILING_BYTES: usize = 8_388_609;

const INVALID_COMMAND_LINE: &[u8] =
    b"eqiora: invalid command line\nusage: eqiora check <MODEL_PATH>\n";
const INVALID_PATH: &[u8] = b"eqiora: MODEL_PATH must contain 1 to 4096 non-control UTF-8 bytes\n";
const SOURCE_TOO_LARGE: &[u8] = b"eqiora: Model source exceeds 8388608 bytes\n";
const INVALID_UTF8: &[u8] = b"eqiora: Model source is not valid UTF-8\n";
const UNAVAILABLE: &[u8] = b"eqiora: MODEL_PATH does not resolve to a readable regular file\n";
const DIAGNOSTIC_OVERFLOW: &[u8] =
    b"eqiora: compilation rejected; terminal diagnostics exceed the 1048576-byte limit\n";
const INTERNAL: &[u8] = b"eqiora: internal compile/check failure\n";

thread_local! {
    static PROJECTION_PANIC: Cell<Option<&'static str>> = const { Cell::new(None) };
}

#[allow(dead_code)]
#[derive(Clone, Copy)]
pub(crate) enum OraclePanicPoint {
    None,
    BeforeOperation(&'static str),
    DuringProjection(&'static str),
}

#[derive(Debug)]
pub(crate) enum OracleReadError {
    TooLarge,
    Unavailable,
}

#[derive(Clone, Copy)]
pub(crate) enum Destination {
    Stdout,
    Stderr,
}

pub(crate) struct OracleOutcome(pub(crate) i32, pub(crate) Destination, pub(crate) Vec<u8>);

impl OracleOutcome {
    fn stdout(exit: i32, payload: impl Into<Vec<u8>>) -> Self {
        Self(exit, Destination::Stdout, payload.into())
    }

    fn stderr(exit: i32, payload: impl Into<Vec<u8>>) -> Self {
        Self(exit, Destination::Stderr, payload.into())
    }

    #[cfg_attr(test, allow(dead_code))]
    pub(crate) fn payload(&self) -> &[u8] {
        &self.2
    }

    #[cfg(test)]
    #[allow(dead_code)]
    pub(crate) fn into_parts(self) -> (i32, Vec<u8>, Vec<u8>) {
        match self.1 {
            Destination::Stdout => (self.0, self.2, Vec::new()),
            Destination::Stderr => (self.0, Vec::new(), self.2),
        }
    }
}

enum CommandError {
    SourceTooLarge,
    InvalidUtf8,
    Unavailable,
    Internal,
}

impl CommandError {
    fn into_outcome(self) -> OracleOutcome {
        match self {
            Self::SourceTooLarge => OracleOutcome::stderr(65, SOURCE_TOO_LARGE),
            Self::InvalidUtf8 => OracleOutcome::stderr(65, INVALID_UTF8),
            Self::Unavailable => OracleOutcome::stderr(66, UNAVAILABLE),
            Self::Internal => OracleOutcome::stderr(70, INTERNAL),
        }
    }
}

impl From<OracleReadError> for CommandError {
    fn from(error: OracleReadError) -> Self {
        match error {
            OracleReadError::TooLarge => Self::SourceTooLarge,
            OracleReadError::Unavailable => Self::Unavailable,
        }
    }
}

impl From<std::io::Error> for CommandError {
    fn from(_: std::io::Error) -> Self {
        Self::Unavailable
    }
}

impl From<std::str::Utf8Error> for CommandError {
    fn from(_: std::str::Utf8Error) -> Self {
        Self::InvalidUtf8
    }
}

type CommandResult = Result<OracleOutcome, CommandError>;

#[cfg_attr(test, allow(dead_code))]
pub(crate) fn process_args() -> Vec<OsString> {
    std::env::args_os().collect()
}

fn set_hook(hook: fn(&std::panic::PanicHookInfo<'_>)) {
    std::panic::set_hook(Box::new(hook));
}

fn silent_hook(_: &std::panic::PanicHookInfo<'_>) {}

fn cli_command() -> clap::Command {
    use clap::{Arg, ArgAction, Command};

    clap::Command::new("eqiora")
        .disable_help_flag(true)
        .disable_version_flag(true)
        .disable_help_subcommand(true)
        .color(clap::ColorChoice::Never)
        .arg(
            Arg::new("root-help")
                .short('h')
                .long("help")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("root-version")
                .short('V')
                .long("version")
                .action(ArgAction::SetTrue),
        )
        .subcommand(
            Command::new("check")
                .disable_help_flag(true)
                .disable_version_flag(true)
                .arg(
                    Arg::new("check-help")
                        .short('h')
                        .long("help")
                        .action(ArgAction::SetTrue),
                )
                .arg(
                    Arg::new("model-path")
                        .value_parser(clap::builder::OsStringValueParser::new())
                        .num_args(1),
                ),
        )
}

fn root_help() -> CommandResult {
    // ROOT_HELP is frozen at the terminal boundary rather than rendered by clap.
    Ok(OracleOutcome::stdout(
        0,
        b"Compile and check one Eqiora Model source file.\n\nUsage:\n  eqiora check <MODEL_PATH>\n\nCommands:\n  check    Compile/check one local Model source file.\n\nOptions:\n  -h, --help       Print help.\n  -V, --version    Print version.\n",
    ))
}

fn check_help() -> CommandResult {
    // CHECK_HELP is frozen at the terminal boundary rather than rendered by clap.
    Ok(OracleOutcome::stdout(
        0,
        b"Compile/check one local Eqiora Model source file.\n\nUsage:\n  eqiora check <MODEL_PATH>\n\nArguments:\n  <MODEL_PATH>    UTF-8 path to one regular Model source file.\n\nOptions:\n  -h, --help    Print help.\n",
    ))
}

fn package_version() -> CommandResult {
    Ok(OracleOutcome::stdout(
        0,
        format!("eqiora {}\n", env!("CARGO_PKG_VERSION")).into_bytes(),
    ))
}

fn invalid_command_line() -> CommandResult {
    Ok(OracleOutcome::stderr(64, INVALID_COMMAND_LINE))
}

fn invalid_path() -> CommandResult {
    Ok(OracleOutcome::stderr(64, INVALID_PATH))
}

fn accepted_projection(
    fingerprint: Result<StructuralSemanticFingerprint, Diagnostic>,
) -> CommandResult {
    if let Some(payload) = PROJECTION_PANIC.take() {
        panic!("{payload}");
    }
    let fingerprint = fingerprint.map_err(|_| CommandError::Internal)?;
    Ok(OracleOutcome::stdout(
        0,
        format!("accepted {fingerprint}\n").into_bytes(),
    ))
}

fn rejected_projection(rendered: Option<Vec<u8>>) -> CommandResult {
    Ok(match rendered {
        Some(payload) => OracleOutcome::stderr(1, payload),
        None => OracleOutcome::stderr(1, DIAGNOSTIC_OVERFLOW),
    })
}

pub(crate) fn read_bounded(
    reported_len: u64,
    reader: &mut dyn Read,
) -> Result<impl AsRef<[u8]> + use<>, OracleReadError> {
    if reported_len > SOURCE_LIMIT_BYTES as u64 {
        return Err(OracleReadError::TooLarge);
    }
    let mut output = Vec::with_capacity(SOURCE_READ_CEILING_BYTES);
    reader
        .take(SOURCE_READ_CEILING_BYTES as u64)
        .read_to_end(&mut output)
        .map_err(|_| OracleReadError::Unavailable)?;
    if output.len() > SOURCE_LIMIT_BYTES {
        return Err(OracleReadError::TooLarge);
    }
    Ok(output)
}

pub(crate) fn contained_run<F>(
    args: fn() -> Vec<OsString>,
    operation: F,
    panic_point: OraclePanicPoint,
) -> OracleOutcome
where
    F: FnOnce(&str, &str) -> Result<ModelDocument, Vec<Diagnostic>>,
{
    set_hook(silent_hook);
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> CommandResult {
        let arguments = args();
        let trailing_bare_delimiter = arguments
            .get(1..)
            .and_then(|arguments| arguments.split_last())
            .is_some_and(|(last, preceding)| {
                last == "--" && !preceding.iter().any(|argument| argument == "--")
            });
        if trailing_bare_delimiter {
            return invalid_command_line();
        }
        let matches = match cli_command().try_get_matches_from(arguments) {
            Ok(matches) => matches,
            Err(_) => return invalid_command_line(),
        };
        let root_help_requested = matches.get_flag("root-help");
        let version_requested = matches.get_flag("root-version");
        if root_help_requested {
            if version_requested || matches.subcommand().is_some() {
                return invalid_command_line();
            }
            return root_help();
        }
        if version_requested {
            if matches.subcommand().is_some() {
                return invalid_command_line();
            }
            return package_version();
        }
        let Some(("check", check)) = matches.subcommand() else {
            return invalid_command_line();
        };
        let check_help_requested = check.get_flag("check-help");
        let model_path = check.get_one::<OsString>("model-path");
        if check_help_requested {
            if model_path.is_some() {
                return invalid_command_line();
            }
            return check_help();
        }
        let Some(model_path) = model_path else {
            return invalid_command_line();
        };
        let Some(name) = model_path.to_str() else {
            return invalid_path();
        };
        if name.is_empty()
            || name.len() > 4096
            || name.chars().count() > 4096
            || name.chars().any(char::is_control)
        {
            return invalid_path();
        }
        if !Path::new(name).is_file() {
            return Err(CommandError::Unavailable);
        }
        let mut file = File::open(name)?;
        let (metadata, regular_file) = {
            let opened = file.metadata()?;
            let regular_file = opened.file_type().is_file();
            (opened, regular_file)
        };
        if !regular_file {
            return Err(CommandError::Unavailable);
        }
        let source_bytes = read_bounded(metadata.len(), &mut file)?;
        let source = std::str::from_utf8(source_bytes.as_ref())?;
        PROJECTION_PANIC.set(None);
        match panic_point {
            OraclePanicPoint::None => {}
            OraclePanicPoint::BeforeOperation(payload) => panic!("{payload}"),
            OraclePanicPoint::DuringProjection(payload) => PROJECTION_PANIC.set(Some(payload)),
        };
        match operation(name, source) {
            Ok(document) => {
                let fingerprint = document.structural_fingerprint();
                accepted_projection(fingerprint)
            }
            Err(diagnostics) => {
                let rendered = terminal::render_diagnostics(&diagnostics);
                rejected_projection(rendered)
            }
        }
    })) {
        Ok(Ok(outcome)) => outcome,
        Ok(Err(error)) => error.into_outcome(),
        Err(_) => CommandError::Internal.into_outcome(),
    }
}
