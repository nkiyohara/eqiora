//! Solve the `org.example.poisson` package end to end on the host CPU and
//! report the L2 error against its exact solution.
//!
//! The model says `-div(grad(u)) = 2 pi^2 sin(pi x) sin(pi y)` on the unit
//! square with `u = 0` on every side, so the exact solution is known:
//! `u(x, y) = sin(pi x) sin(pi y)`.
//!
//! Four stages appear below in order, and each one is a separate decision:
//!
//! 1. **Model** — what must be true. Compiled from the package source; it
//!    names no mesh, no element, no solver, and no machine.
//! 2. **Realization** — how the accepted model is made executable. Written
//!    out literally here because Eqiora never picks one for you.
//! 3. **Run** — execution of that one accepted Realization.
//! 4. **Evidence** — what the run proves, and what it does not.
//!
//! Run it from the repository root:
//!
//! ```text
//! cargo run --locked -p eqiora --example poisson
//! ```

use std::f64::consts::PI;
use std::num::NonZeroUsize;
use std::process::ExitCode;

use eqiora::Diagnostic;
use eqiora::api::{
    ModelDocument, ScalarEllipticExecutionEnvironment, ScalarEllipticIntent, ScalarEllipticMethod,
    ScalarEllipticRunResult,
};
use eqiora::realization::RealizationRevision;

const SOURCE: &str = include_str!("../../../packages/org.example.poisson/src/main.eqi");
const SOURCE_PATH: &str = "packages/org.example.poisson/src/main.eqi";

/// Uniform cells on each axis of the generated Cartesian mesh.
const CELLS_PER_AXIS: usize = 16;

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
    // 1. Model: canonical meaning, with no numerical commitment yet.
    let model = ModelDocument::compile(SOURCE_PATH, SOURCE)?;

    // 2. Realization: every numerical and placement choice, stated once.
    //    Nothing below is a default; swapping any line is a different
    //    Realization of the same Model.
    let environment = ScalarEllipticExecutionEnvironment::host_serial();
    let intent = ScalarEllipticIntent::new(
        RealizationRevision::new(1),
        ScalarEllipticMethod::FiniteElement,
        NonZeroUsize::new(CELLS_PER_AXIS).expect("the fixed cell count is nonzero"),
        NonZeroUsize::MIN,
    );

    // Resolving the intent against the environment allocates no mesh, matrix,
    // or worker pool. It either yields one content-addressed plan or fails;
    // an unsupported request never quietly becomes a supported one.
    let plan = model.preview_scalar_elliptic_run(intent, environment)?;

    // 3. Run: execute that exact accepted plan and nothing else.
    let result = model.run_scalar_elliptic_plan(plan, environment)?;

    // 4. Evidence: the answer, and the independent checks that stand behind it.
    let error = result
        .l2_error(|[x, y]| (PI * x).sin() * (PI * y).sin())
        .map_err(|diagnostic| vec![diagnostic])?;
    report(&model, &result, error)
}

fn report(
    model: &ModelDocument,
    result: &ScalarEllipticRunResult,
    error: f64,
) -> Result<(), Vec<Diagnostic>> {
    // `model.digest()` is the Model artifact identity, and it embeds fresh
    // entity ULIDs minted by this compile, so it differs between processes.
    // The structural fingerprint is the identity of the *meaning*, so two
    // compiles of the same source agree on it.
    let fingerprint = model
        .structural_fingerprint()
        .map_err(|diagnostic| vec![diagnostic])?;
    let solve = result.solve();
    let field = result.field();
    let shape = field.logical_shape();

    println!("model        {}", fingerprint.digest());
    println!(
        "realization  revision {}",
        result.plan().intent().realization_revision().get()
    );
    println!("  method     continuous Q1 finite elements");
    println!("  mesh       generated Cartesian, {CELLS_PER_AXIS}x{CELLS_PER_AXIS} cells");
    println!(
        "  placement  {} on {} worker(s)",
        result.plan().adapter(),
        result.plan().intent().workers(),
    );
    println!(
        "solve        {} iteration(s), true residual {:.3e} <= {:.3e} target",
        solve.completed_iterations(),
        solve.true_residual_norm(),
        solve.residual_target(),
    );
    println!(
        "balance      relative imbalance {:.3e}",
        result.balance().relative_imbalance(),
    );
    println!(
        "field        {}x{} vertices, values in [{:.6}, {:.6}]",
        shape[0],
        shape[1],
        field.minimum(),
        field.maximum(),
    );
    println!("L2 error     {error:.6e}  against sin(pi x) sin(pi y)");
    Ok(())
}
