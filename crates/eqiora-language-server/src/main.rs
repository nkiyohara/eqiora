mod protocol;
mod workspace_uri;

use std::{
    error::Error,
    io::{self, Write},
    process::ExitCode,
};

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() -> ExitCode {
    if std::env::args_os()
        .nth(1)
        .is_some_and(|argument| argument == "--version")
    {
        println!("eqiora-language-server {VERSION}");
        return ExitCode::SUCCESS;
    }

    log_event("info", "server_started");
    match serve() {
        Ok(()) => {
            log_event("info", "server_stopped");
            ExitCode::SUCCESS
        }
        Err(_) => {
            log_event("error", "server_failed");
            ExitCode::FAILURE
        }
    }
}

fn serve() -> Result<(), Box<dyn Error + Send + Sync>> {
    let (connection, io_threads) = lsp_server::Connection::stdio();
    protocol::run(connection, VERSION)?;
    io_threads.join()?;
    Ok(())
}

fn log_event(level: &str, event: &str) {
    let entry = serde_json::json!({
        "level": level,
        "target": "eqiora-language-server",
        "event": event,
    });
    let _ = writeln!(io::stderr().lock(), "{entry}");
}
