use std::io::Write;

use eqiora::{Diagnostic, Severity};

use super::command::{Destination, OracleOutcome};

const TERMINAL_LIMIT_BYTES: usize = 1_048_576;

#[cfg(unix)]
#[cfg_attr(test, allow(dead_code))]
fn output_stream(destination: Destination) -> std::io::Result<Box<dyn Write>> {
    use std::os::fd::AsFd;

    let owned = match destination {
        Destination::Stdout => std::io::stdout().as_fd().try_clone_to_owned()?,
        Destination::Stderr => std::io::stderr().as_fd().try_clone_to_owned()?,
    };
    Ok(Box::new(std::fs::File::from(owned)))
}

#[cfg(not(unix))]
#[cfg_attr(test, allow(dead_code))]
fn output_stream(destination: Destination) -> std::io::Result<Box<dyn Write>> {
    Ok(match destination {
        Destination::Stdout => Box::new(std::io::stdout()),
        Destination::Stderr => Box::new(std::io::stderr()),
    })
}

fn bounded_member(value: &str, scalar_limit: usize, byte_limit: usize, nonempty: bool) -> bool {
    (!nonempty || !value.is_empty())
        && value.len() <= byte_limit
        && value.chars().count() <= scalar_limit
}

fn append(output: &mut Vec<u8>, bytes: &[u8]) -> Option<()> {
    let next = output.len().checked_add(bytes.len())?;
    if next > TERMINAL_LIMIT_BYTES {
        return None;
    }
    output.extend_from_slice(bytes);
    Some(())
}

fn append_escaped(output: &mut Vec<u8>, value: &str) -> Option<()> {
    for character in value.chars() {
        match character {
            '\u{20}'..='\u{7e}' if character != '\\' => {
                append(output, &[character as u8])?;
            }
            '\\' => append(output, b"\\\\")?,
            other => {
                let escape = format!("\\u{{{:x}}}", other as u32);
                append(output, escape.as_bytes())?;
            }
        }
    }
    Some(())
}

pub(crate) fn severity_label(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
        Severity::Note => "note",
    }
}

pub(crate) fn render_diagnostics(diagnostics: &[Diagnostic]) -> Option<Vec<u8>> {
    if diagnostics.len() > 1024 {
        return None;
    }
    let mut output = Vec::new();
    for diagnostic in diagnostics {
        let code = diagnostic.code().to_string();
        let code_bytes = code.as_bytes();
        if code_bytes.len() != 6
            || !code_bytes[..2].iter().all(u8::is_ascii_uppercase)
            || !code_bytes[2..].iter().all(u8::is_ascii_digit)
            || !bounded_member(diagnostic.message(), 1_048_576, 1_048_576, true)
        {
            return None;
        }
        if let Some(span) = diagnostic.source_span() {
            if span.end < span.start || !bounded_member(&span.file, 4096, 4096, false) {
                return None;
            }
            append_escaped(&mut output, &span.file)?;
            append(&mut output, b":")?;
            append(&mut output, span.start.to_string().as_bytes())?;
            append(&mut output, b":")?;
            append(&mut output, span.end.to_string().as_bytes())?;
            append(&mut output, b": ")?;
        }
        append(
            &mut output,
            severity_label(diagnostic.severity()).as_bytes(),
        )?;
        append(&mut output, b"[")?;
        append(&mut output, code_bytes)?;
        append(&mut output, b"]: ")?;
        append_escaped(&mut output, diagnostic.message())?;
        if let Some(path) = diagnostic.graph_path() {
            if path.segments().len() > 256
                || path
                    .segments()
                    .iter()
                    .any(|segment| !bounded_member(segment, 4096, 4096, true))
            {
                return None;
            }
            append(&mut output, b" (at ")?;
            for (index, segment) in path.segments().iter().enumerate() {
                if index != 0 {
                    append(&mut output, b".")?;
                }
                append_escaped(&mut output, segment)?;
            }
            append(&mut output, b")")?;
        }
        if let Some(patch) = diagnostic.suggestion() {
            if !bounded_member(&patch.summary, 4096, 4096, true) {
                return None;
            }
            append(&mut output, b"; help: ")?;
            append_escaped(&mut output, &patch.summary)?;
        }
        append(&mut output, b"\n")?;
    }
    Some(output)
}

#[cfg_attr(test, allow(dead_code))]
pub(crate) fn commit(outcome @ OracleOutcome(exit, destination, _): OracleOutcome) -> i32 {
    let Ok(mut stream) = output_stream(destination) else {
        return 74;
    };
    if stream
        .write_all(outcome.payload())
        .and_then(|()| stream.flush())
        .is_err()
    {
        74
    } else {
        exit
    }
}
