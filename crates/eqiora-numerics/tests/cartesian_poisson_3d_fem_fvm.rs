use std::f64::consts::PI;

mod support;

use support::cartesian_poisson::{ManufacturedCase, verify};

const SOURCE: &str =
    include_str!("../../../verify/numerics/cartesian-poisson-3d-fem-fvm/models/poisson.eqi");
const EXPECTED: &str =
    include_str!("../../../verify/numerics/cartesian-poisson-3d-fem-fvm/expected/convergence.csv");

#[test]
fn one_canonical_cube_revision_converges_through_q1_fem_and_tpfa_fvm() {
    verify(
        ManufacturedCase {
            file: "manufactured-poisson-cube.eqi",
            source: SOURCE,
            expected: EXPECTED,
            dimension: 3,
            source_at_center: 3.0 * PI.powi(2),
            maximum_relative_balance: 2.0e-11,
        },
        &|coordinate| {
            (PI * coordinate[0]).sin() * (PI * coordinate[1]).sin() * (PI * coordinate[2]).sin()
        },
    );
}
