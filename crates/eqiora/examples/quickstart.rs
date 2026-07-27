use std::process::ExitCode;

use eqiora::Diagnostic;
use eqiora::api::ModelDocument;

const SOURCE: &str = include_str!("../../../examples/decay.eqi");

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(diagnostics) => {
            for diagnostic in diagnostics {
                eprintln!("{}: {}", diagnostic.code(), diagnostic.message());
            }
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), Vec<Diagnostic>> {
    let model = ModelDocument::compile("examples/decay.eqi", SOURCE)?;
    // `model.digest()` identifies the Model *artifact*, and every compile mints
    // fresh entity ULIDs, so it differs between processes by design. What a
    // reader of this line wants is the identity of the *meaning*, which two
    // compiles of the same source agree on. Printing the artifact digest here
    // made the shortest public path look non-reproducible.
    let fingerprint = model
        .structural_fingerprint()
        .map_err(|diagnostic| vec![diagnostic])?;
    let result = model.run_reference(1.0, 0.01)?;
    let series = result
        .series()
        .iter()
        .find(|series| series.name() == Some("x"))
        .ok_or_else(|| Vec::from([missing_result("the run returned no `x` series")]))?;
    let (&time, &value) = series
        .time()
        .last()
        .zip(series.values().last())
        .ok_or_else(|| Vec::from([missing_result("the `x` series is empty")]))?;

    println!("model {}", fingerprint.digest());
    println!("x({time:.2} s) = {value:.8}");
    Ok(())
}

fn missing_result(message: &str) -> Diagnostic {
    Diagnostic::error(eqiora::diagnostic::codes::NUMERICAL_SOLVE_FAILED, message)
}
