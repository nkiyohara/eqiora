use std::f64::consts::PI;

use eqiora_compiler::compile;
use eqiora_graph::{GraphStore, InMemoryGraphStore};
use eqiora_numerics::scalar::compare_canonical_scalar_elliptic_1d;
use eqiora_sem::KernelProgram;

const SOURCE: &str = include_str!("../../../verify/numerics/poisson-fem-fvm/models/poisson.eqi");

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut compiled = compile("verify/numerics/poisson-fem-fvm/models/poisson.eqi", SOURCE)
        .expect("repository verification source compiles");
    let (transaction, model_id, _) = compiled.remove(0).into_parts();
    let mut store = InMemoryGraphStore::new();
    store
        .commit(transaction)
        .expect("repository verification transaction commits");
    let program = KernelProgram::from_snapshot(&store.snapshot(), model_id)
        .expect("repository verification model validates");
    let (_, report) =
        compare_canonical_scalar_elliptic_1d(&program, &[8, 16, 32, 64, 128], &|coordinate| {
            (PI * coordinate).sin()
        })?;
    println!(
        "cells,h_max,fem_l2,fem_order,fvm_l2,fvm_order,fem_relative_balance,fvm_relative_balance"
    );
    for row in report {
        let fem_order = row
            .fem_order
            .map_or_else(String::new, |value| format!("{value:.6}"));
        let fvm_order = row
            .fvm_order
            .map_or_else(String::new, |value| format!("{value:.6}"));
        println!(
            "{},{:.8e},{:.8e},{},{:.8e},{},{:.8e},{:.8e}",
            row.cells,
            row.max_cell_measure,
            row.fem_l2_error,
            fem_order,
            row.fvm_l2_error,
            fvm_order,
            row.fem_relative_balance_error,
            row.fvm_relative_balance_error,
        );
    }
    Ok(())
}
