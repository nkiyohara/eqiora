use std::env;
use std::error::Error;
use std::fs;
use std::path::PathBuf;

const EXACT_CYLINDER_MODEL: &str = "../../examples/steady-flow-past-cylinder.model.json";
const MIXED_BOUNDARY_ELASTICITY_MODEL: &str =
    "../../verify/solid/mixed-boundary-elasticity-2d/models/direct.eqi";
const FIXED_REFERENCE_FSI_MODEL: &str =
    "../../verify/fsi/fixed-reference-monolithic-step-2d/models/direct.eqi";

fn main() -> Result<(), Box<dyn Error>> {
    println!("cargo:rerun-if-changed={EXACT_CYLINDER_MODEL}");
    println!("cargo:rerun-if-changed={MIXED_BOUNDARY_ELASTICITY_MODEL}");
    println!("cargo:rerun-if-changed={FIXED_REFERENCE_FSI_MODEL}");

    let manifest_dir = PathBuf::from(
        env::var_os("CARGO_MANIFEST_DIR")
            .ok_or("Cargo did not provide CARGO_MANIFEST_DIR to the Python package build")?,
    );
    let output_dir = PathBuf::from(
        env::var_os("OUT_DIR")
            .ok_or("Cargo did not provide OUT_DIR to the Python package build")?,
    );
    fs::copy(
        manifest_dir.join(EXACT_CYLINDER_MODEL),
        output_dir.join("steady-flow-past-cylinder.model.json"),
    )?;
    fs::copy(
        manifest_dir.join(MIXED_BOUNDARY_ELASTICITY_MODEL),
        output_dir.join("mixed-boundary-elasticity.eqi"),
    )?;
    fs::copy(
        manifest_dir.join(FIXED_REFERENCE_FSI_MODEL),
        output_dir.join("fixed-reference-fsi.eqi"),
    )?;
    Ok(())
}
