//! Command-line frontend for repository verification evidence.

use std::path::Path;
use std::process::ExitCode;

use clap::{Parser, Subcommand, ValueEnum};
use eqiora_verify::{
    CommandKind, EvidenceEnvironment, ExecutionPolicy, Request, SystemEvidenceRunner,
    capability_evidence_index, execute,
};

#[derive(Debug, Parser)]
#[command(version, about = "Validate and run Eqiora verification evidence")]
struct Cli {
    /// Stable report representation.
    #[arg(long, global = true, value_enum, default_value_t = OutputFormat::Human)]
    format: OutputFormat,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum OutputFormat {
    Human,
    Json,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum Environment {
    HostCpu,
    PhysicalMpiCuda,
}

impl From<Environment> for EvidenceEnvironment {
    fn from(value: Environment) -> Self {
        match value {
            Environment::HostCpu => Self::HostCpu,
            Environment::PhysicalMpiCuda => Self::PhysicalMpiCuda,
        }
    }
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Compatibility command: validate all manifests and run executable evidence.
    Verify,
    /// List validated cases in deterministic ID order.
    List {
        /// Limit the report to one exact case ID.
        #[arg(long)]
        case: Option<String>,
    },
    /// Project validated case manifests into a capability-to-evidence index.
    Index {
        /// Limit the index to one exact capability identifier.
        #[arg(long)]
        capability: Option<String>,
    },
    /// Validate manifests, evidence artifacts, and typed targets without running them.
    Check {
        /// Limit validation output to one exact case ID.
        #[arg(long)]
        case: Option<String>,
    },
    /// Run structured evidence targets without invoking a shell.
    Run {
        /// Limit execution to one exact case ID.
        #[arg(long)]
        case: Option<String>,
        /// Continue after an evidence failure.
        #[arg(long, conflicts_with = "fail_fast")]
        keep_going: bool,
        /// Stop after the first evidence failure (the default).
        #[arg(long, conflicts_with = "keep_going")]
        fail_fast: bool,
        /// Run only evidence declared for this exact environment.
        #[arg(long, value_enum)]
        environment: Option<Environment>,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace crate has a repository root");
    let request = match cli.command {
        Command::Index { capability } => {
            let index = capability_evidence_index(root, capability.as_deref());
            match cli.format {
                OutputFormat::Human => print!("{}", index.render_human()),
                OutputFormat::Json => match serde_json::to_string(&index) {
                    Ok(json) => println!("{json}"),
                    Err(error) => {
                        eprintln!("cannot serialize capability evidence index: {error}");
                        return ExitCode::FAILURE;
                    }
                },
            }
            return if index.success {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            };
        }
        Command::Verify => Request::new(CommandKind::Run, None, ExecutionPolicy::FailFast),
        Command::List { case } => Request::new(CommandKind::List, case, ExecutionPolicy::FailFast),
        Command::Check { case } => {
            Request::new(CommandKind::Check, case, ExecutionPolicy::FailFast)
        }
        Command::Run {
            case,
            keep_going,
            fail_fast: _,
            environment,
        } => {
            let request = Request::new(
                CommandKind::Run,
                case,
                if keep_going {
                    ExecutionPolicy::KeepGoing
                } else {
                    ExecutionPolicy::FailFast
                },
            );
            if let Some(environment) = environment {
                request.for_environment(environment.into())
            } else {
                request
            }
        }
    };
    let report = execute(root, &request, &SystemEvidenceRunner::from_environment());

    match cli.format {
        OutputFormat::Human => print!("{}", report.render_human()),
        OutputFormat::Json => match serde_json::to_string(&report) {
            Ok(json) => println!("{json}"),
            Err(error) => {
                eprintln!("cannot serialize verification report: {error}");
                return ExitCode::FAILURE;
            }
        },
    }

    if report.success {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
