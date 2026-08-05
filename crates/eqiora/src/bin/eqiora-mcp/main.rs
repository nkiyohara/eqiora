mod framing;
mod protocol;
mod tool;

#[cfg(test)]
mod oracle;

use std::io;
#[cfg(test)]
use std::io::{Read, Write};
#[cfg(any(unix, target_os = "hermit", target_os = "motor", target_os = "wasi"))]
use std::os::fd::AsFd;
#[cfg(windows)]
use std::os::windows::io::AsHandle;
#[cfg(test)]
use std::sync::Arc;

#[cfg(test)]
use eqiora::Diagnostic;
#[cfg(test)]
use eqiora::api::ModelDocument;

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(any(unix, target_os = "hermit", target_os = "motor", target_os = "wasi"))]
fn stdout_writer() -> io::Result<io::PipeWriter> {
    Ok(io::stdout().as_fd().try_clone_to_owned()?.into())
}

#[cfg(windows)]
fn stdout_writer() -> io::Result<io::PipeWriter> {
    Ok(io::stdout().as_handle().try_clone_to_owned()?.into())
}

fn main() -> io::Result<()> {
    protocol::run(
        io::stdin(),
        stdout_writer()?,
        tool::compile_document,
        VERSION,
        protocol::Signals::ordinary(),
    )
}

#[cfg(test)]
fn run_for_oracle<R, W, C>(
    reader: R,
    writer: W,
    compiler: C,
    control: Arc<oracle::OracleControl>,
) -> io::Result<()>
where
    R: Read + Send + 'static,
    W: Write,
    C: Fn(&str, &str) -> Result<ModelDocument, Vec<Diagnostic>> + Send + Sync + 'static,
{
    let before = Arc::clone(&control);
    let cancelled = Arc::clone(&control);
    let decided = Arc::clone(&control);
    protocol::run(
        reader,
        writer,
        compiler,
        VERSION,
        protocol::Signals::testing(
            move || before.before_compile(),
            move |id| cancelled.cancellation_processed(id),
            move |id, commit| decided.response_commit_decided(id, commit),
        ),
    )
}
