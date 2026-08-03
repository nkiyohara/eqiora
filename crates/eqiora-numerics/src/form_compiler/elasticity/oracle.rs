//! Independently derived oracle for the frozen Cartesian Q1 elasticity form.
//!
//! Every scientific literal and tolerance below is owned by public Issue 113.
//! This child module calls the production-native private seam directly; it
//! intentionally contains no second elasticity formula or test-only adapter.

use std::array;

use eqiora_compiler::compile;
use eqiora_graph::{GraphStore, InMemoryGraphStore};
use eqiora_meshing::{
    AffineGeometryMap, QuadratureRule, ReferenceCell, simplex_centroid_rule,
};
use eqiora_sem::KernelProgram;

use super::*;
use crate::form_compiler::{MatrixSlot, WeakSign, WeakTermSlot};

const SOURCE: &str = include_str!(
    "../../../../../verify/solid/isotropic-elasticity-2d/models/linear-load.eqi"
);

const PARAMETERS: [f64; 3] = [3.0, 2.0, 1.0];
const STATE: [f64; 8] = [1.0, -2.0, 2.0, 0.5, 2.5, 1.5, 3.5, 4.0];
const STATE_DIRECTION: [f64; 8] = [1.0, -2.0, 3.0, -4.0, 5.0, -6.0, 7.0, -8.0];
const PARAMETER_DIRECTION: [f64; 3] = [0.5, -1.0 / 3.0, 2.0];
const COTANGENT: [f64; 8] = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];

const MATRIX: [f64; 64] = [
    11.0 / 3.0,
    5.0 / 4.0,
    -13.0 / 6.0,
    -1.0 / 4.0,
    1.0 / 3.0,
    1.0 / 4.0,
    -11.0 / 6.0,
    -5.0 / 4.0,
    5.0 / 4.0,
    11.0 / 3.0,
    1.0 / 4.0,
    1.0 / 3.0,
    -1.0 / 4.0,
    -13.0 / 6.0,
    -5.0 / 4.0,
    -11.0 / 6.0,
    -13.0 / 6.0,
    1.0 / 4.0,
    11.0 / 3.0,
    -5.0 / 4.0,
    -11.0 / 6.0,
    5.0 / 4.0,
    1.0 / 3.0,
    -1.0 / 4.0,
    -1.0 / 4.0,
    1.0 / 3.0,
    -5.0 / 4.0,
    11.0 / 3.0,
    5.0 / 4.0,
    -11.0 / 6.0,
    1.0 / 4.0,
    -13.0 / 6.0,
    1.0 / 3.0,
    -1.0 / 4.0,
    -11.0 / 6.0,
    5.0 / 4.0,
    11.0 / 3.0,
    -5.0 / 4.0,
    -13.0 / 6.0,
    1.0 / 4.0,
    1.0 / 4.0,
    -13.0 / 6.0,
    5.0 / 4.0,
    -11.0 / 6.0,
    -5.0 / 4.0,
    11.0 / 3.0,
    -1.0 / 4.0,
    1.0 / 3.0,
    -11.0 / 6.0,
    -5.0 / 4.0,
    1.0 / 3.0,
    1.0 / 4.0,
    -13.0 / 6.0,
    -1.0 / 4.0,
    11.0 / 3.0,
    5.0 / 4.0,
    -5.0 / 4.0,
    -11.0 / 6.0,
    -1.0 / 4.0,
    -13.0 / 6.0,
    1.0 / 4.0,
    1.0 / 3.0,
    5.0 / 4.0,
    11.0 / 3.0,
];
const LOAD: [f64; 8] = [1.0 / 16.0, 0.0, 1.0 / 16.0, 0.0, 1.0 / 16.0, 0.0, 1.0 / 16.0, 0.0];
const RESIDUAL: [f64; 8] = [
    -217.0 / 16.0,
    -21.0,
    23.0 / 16.0,
    -9.0,
    -25.0 / 16.0,
    9.0,
    215.0 / 16.0,
    21.0,
];
const STATE_ACTION: [f64; 8] = [-7.0, 11.0, 1.0, 17.0, -1.0, -17.0, 7.0, -11.0];
const MU_ACTION: [f64; 8] = [-3.0, -5.5, -1.0, -1.5, 1.0, 1.5, 3.0, 5.5];
const LAMBDA_ACTION: [f64; 8] = [
    -9.0 / 4.0,
    -9.0 / 4.0,
    9.0 / 4.0,
    -9.0 / 4.0,
    -9.0 / 4.0,
    9.0 / 4.0,
    9.0 / 4.0,
    9.0 / 4.0,
];
const PRESSURE_GRADIENT_ACTION: [f64; 8] = [
    -1.0 / 16.0,
    0.0,
    -1.0 / 16.0,
    0.0,
    -1.0 / 16.0,
    0.0,
    -1.0 / 16.0,
    0.0,
];
const PARAMETER_ACTION: [f64; 8] = [
    -7.0 / 8.0,
    -2.0,
    -11.0 / 8.0,
    0.0,
    9.0 / 8.0,
    0.0,
    5.0 / 8.0,
    2.0,
];
const STATE_TRANSPOSE: [f64; 8] = [-21.0, -27.0, 3.0, -9.0, -3.0, 9.0, 21.0, 27.0];
const PARAMETER_TRANSPOSE: [f64; 3] = [56.0, 27.0, -1.0];

const STEP: f64 = 1.0 / 4096.0;
const ABSOLUTE_TOLERANCE: f64 = 1.0e-9;

#[test]
fn exact_local_contribution_and_differential_actions_match_the_dual_oracle() {
    let program = compile_program(SOURCE);
    let model = crate::canonical_elasticity::lower_isotropic_elasticity_cartesian_2d(&program)
        .unwrap();
    let quadrature = quadrature();
    let form = derive_cartesian_q1_elasticity_form_2d(&program).unwrap();
    let admitted = form.admit_quadrature(&quadrature).unwrap();
    let primal = admitted
        .evaluate(
            &geometry(),
            &quadrature,
            PARAMETERS[0],
            PARAMETERS[1],
            Some(model.load_potential_expression()),
        )
        .unwrap();
    let baseline = admitted
        .evaluate_with_actions(
            &geometry(),
            &quadrature,
            PARAMETERS,
            &STATE,
            &STATE_DIRECTION,
            PARAMETER_DIRECTION,
            &COTANGENT,
        )
        .unwrap();
    assert_absolute_slice(baseline.contribution().matrix(), &MATRIX);
    assert_absolute_slice(baseline.contribution().rhs(), &LOAD);
    assert_absolute_slice(primal.matrix(), baseline.contribution().matrix());
    assert_absolute_slice(primal.rhs(), baseline.contribution().rhs());
    assert_absolute_slice(baseline.residual(), &RESIDUAL);
    assert_absolute_slice(baseline.state_direction_action(), &STATE_ACTION);
    assert_absolute_slice(baseline.parameter_direction_action(), &PARAMETER_ACTION);
    assert_absolute_slice(baseline.state_transpose_action(), &STATE_TRANSPOSE);
    assert_absolute_slice(baseline.parameter_transpose_action(), &PARAMETER_TRANSPOSE);

    for (direction, expected) in [
        ([1.0, 0.0, 0.0], MU_ACTION),
        ([0.0, 1.0, 0.0], LAMBDA_ACTION),
        ([0.0, 0.0, 1.0], PRESSURE_GRADIENT_ACTION),
    ] {
        let coordinate = actions(PARAMETERS, STATE, STATE_DIRECTION, direction, COTANGENT);
        assert_absolute_slice(coordinate.parameter_direction_action(), &expected);
    }

    assert_absolute_scalar(dot(&COTANGENT, baseline.state_direction_action()), -60.0);
    assert_absolute_scalar(dot(&STATE_DIRECTION, baseline.state_transpose_action()), -60.0);
    assert_absolute_scalar(dot(&COTANGENT, baseline.parameter_direction_action()), 17.0);
    assert_absolute_scalar(
        dot(&PARAMETER_DIRECTION, baseline.parameter_transpose_action()),
        17.0,
    );
}

#[test]
fn centered_finite_differences_confirm_every_forward_and_transpose_action() {
    let baseline = actions(PARAMETERS, STATE, STATE_DIRECTION, PARAMETER_DIRECTION, COTANGENT);
    let plus_state = array::from_fn(|index| STATE[index] + STEP * STATE_DIRECTION[index]);
    let minus_state = array::from_fn(|index| STATE[index] - STEP * STATE_DIRECTION[index]);
    let state_fd = centered_vector(
        residual(PARAMETERS, plus_state),
        residual(PARAMETERS, minus_state),
    );
    assert_absolute_slice(&state_fd, baseline.state_direction_action());

    let plus_parameters = array::from_fn(|index| {
        PARAMETERS[index] + STEP * PARAMETER_DIRECTION[index]
    });
    let minus_parameters = array::from_fn(|index| {
        PARAMETERS[index] - STEP * PARAMETER_DIRECTION[index]
    });
    let parameter_fd = centered_vector(
        residual(plus_parameters, STATE),
        residual(minus_parameters, STATE),
    );
    assert_absolute_slice(&parameter_fd, baseline.parameter_direction_action());

    let state_transpose_fd = array::from_fn(|coordinate| {
        let mut plus = STATE;
        let mut minus = STATE;
        plus[coordinate] += STEP;
        minus[coordinate] -= STEP;
        centered_scalar(
            dot(&COTANGENT, &residual(PARAMETERS, plus)),
            dot(&COTANGENT, &residual(PARAMETERS, minus)),
        )
    });
    assert_absolute_slice(&state_transpose_fd, baseline.state_transpose_action());

    let parameter_transpose_fd = array::from_fn(|coordinate| {
        let mut plus = PARAMETERS;
        let mut minus = PARAMETERS;
        plus[coordinate] += STEP;
        minus[coordinate] -= STEP;
        centered_scalar(
            dot(&COTANGENT, &residual(plus, STATE)),
            dot(&COTANGENT, &residual(minus, STATE)),
        )
    });
    assert_absolute_slice(
        &parameter_transpose_fd,
        baseline.parameter_transpose_action(),
    );
}

#[test]
fn exact_values_reject_constitutive_order_load_and_transpose_mutants() {
    let baseline = actions(PARAMETERS, STATE, STATE_DIRECTION, PARAMETER_DIRECTION, COTANGENT);

    // A full-gradient law cannot preserve the frozen matrix or the zero action
    // of the exact infinitesimal rotation in this DOF ordering.
    let rotation = [0.0, 0.0, 0.0, 0.5, -0.5, 0.0, -0.5, 0.5];
    let rotation_action = matrix_action(baseline.contribution().matrix(), &rotation);
    assert_absolute_slice(&rotation_action, &[0.0; 8]);

    for (name, parameters) in [
        ("swapped Lame contributions", [2.0, 3.0, 1.0]),
        ("halved shear contribution", [1.5, 2.0, 1.0]),
        ("dropped volumetric contribution", [3.0, 0.0, 1.0]),
        ("halved volumetric contribution", [3.0, 1.0, 1.0]),
    ] {
        let mutant = actions(parameters, STATE, STATE_DIRECTION, PARAMETER_DIRECTION, COTANGENT);
        assert_rejected_numeric_mutant(mutant.contribution().matrix(), &MATRIX, name);
    }
    let half_shear = actions(
        [1.5, 2.0, 1.0],
        STATE,
        STATE_DIRECTION,
        PARAMETER_DIRECTION,
        COTANGENT,
    );
    let dropped_shear = baseline
        .contribution()
        .matrix()
        .iter()
        .zip(half_shear.contribution().matrix())
        .map(|(baseline, half)| 2.0 * half - baseline)
        .collect::<Vec<_>>();
    assert_rejected_numeric_mutant(&dropped_shear, &MATRIX, "dropped shear contribution");

    let component_permutation = [1, 0, 3, 2, 5, 4, 7, 6];
    let node_permutation = [0, 1, 4, 5, 2, 3, 6, 7];
    for (name, permutation) in [
        ("swapped vector components", component_permutation),
        ("swapped scalar-node bit order", node_permutation),
    ] {
        let mutant = permute_matrix(baseline.contribution().matrix(), permutation);
        assert_rejected_numeric_mutant(&mutant, &MATRIX, name);
    }

    let reversed_load = baseline
        .contribution()
        .rhs()
        .iter()
        .map(|value| -*value)
        .collect::<Vec<_>>();
    assert_rejected_numeric_mutant(&reversed_load, &LOAD, "reversed load gradient");
    assert_rejected_numeric_mutant(
        baseline.state_direction_action(),
        &STATE_TRANSPOSE,
        "primal action returned as state transpose",
    );

    for (name, mutant) in [
        ("omitted mu coordinate", [0.0, 27.0, -1.0]),
        ("omitted lambda coordinate", [56.0, 0.0, -1.0]),
        ("omitted pressure-gradient coordinate", [56.0, 27.0, 0.0]),
        ("transposed mu/lambda coordinates", [27.0, 56.0, -1.0]),
    ] {
        assert_rejected_numeric_mutant(&mutant, &PARAMETER_TRANSPOSE, name);
    }
}

#[test]
fn witness_only_form_rejects_actions_and_realization_drift() {
    let quadrature = quadrature();
    let witness = compile_cartesian_q1_elasticity_form_2d(&quadrature).unwrap();
    assert!(
        witness
            .evaluate_with_actions(
                &geometry(),
                &quadrature,
                PARAMETERS,
                &STATE,
                &STATE_DIRECTION,
                PARAMETER_DIRECTION,
                &COTANGENT,
            )
            .is_err(),
        "witness-data form admitted derivative actions without a certificate"
    );

    let derived = derived_form();
    let one_point = QuadratureRule::tensor_product_gauss_legendre(2, 1).unwrap();
    let three_point = QuadratureRule::tensor_product_gauss_legendre(2, 3).unwrap();
    let simplex = simplex_centroid_rule(2).unwrap();
    assert!(derived.admit_quadrature(&one_point).is_err());
    assert!(derived.admit_quadrature(&three_point).is_err());
    assert!(compile_cartesian_q1_elasticity_form_2d(&simplex).is_err());

    let generic = compile_cartesian_q1_elasticity_form_2d(&three_point).unwrap();
    let model = crate::canonical_elasticity::lower_isotropic_elasticity_cartesian_2d(
        &compile_program(SOURCE),
    )
    .unwrap();
    assert!(
        generic
            .evaluate(&geometry(), &three_point, 3.0, 2.0, None)
            .is_ok(),
        "generic witness form lost the body-free local-action envelope"
    );
    assert!(
        generic
            .evaluate(
                &geometry(),
                &three_point,
                3.0,
                2.0,
                Some(model.load_potential_expression()),
            )
            .is_ok(),
        "generic witness form lost accepted ScalarSpatialExpression loads"
    );

    let admitted = derived.admit_quadrature(&quadrature).unwrap();
    let wrong_geometry = AffineGeometryMap::new(
        ReferenceCell::hypercube(2).unwrap(),
        3,
        vec![0.25, 0.25, 0.0],
        vec![0.25, 0.0, 0.0, 0.25, 0.0, 0.0],
    )
    .unwrap();
    assert!(
        admitted
            .evaluate_with_actions(
                &wrong_geometry,
                &quadrature,
                PARAMETERS,
                &STATE,
                &STATE_DIRECTION,
                PARAMETER_DIRECTION,
                &COTANGENT,
            )
            .is_err()
    );
}

#[test]
fn derived_primal_requires_its_certificate_owned_load_before_realization_work() {
    let program = compile_program(SOURCE);
    let exact_model =
        crate::canonical_elasticity::lower_isotropic_elasticity_cartesian_2d(&program).unwrap();
    let foreign_source = SOURCE.replace("pressure_gradient", "foreign_pressure_gradient");
    let foreign_model = crate::canonical_elasticity::lower_isotropic_elasticity_cartesian_2d(
        &compile_program(&foreign_source),
    )
    .unwrap();
    let quadrature = quadrature();
    let form = derive_cartesian_q1_elasticity_form_2d(&program).unwrap();
    let admitted = form.admit_quadrature(&quadrature).unwrap();
    let wrong_geometry = AffineGeometryMap::new(
        ReferenceCell::hypercube(2).unwrap(),
        3,
        vec![0.25, 0.25, 0.0],
        vec![0.25, 0.0, 0.0, 0.25, 0.0, 0.0],
    )
    .unwrap();
    let wrong_quadrature = QuadratureRule::tensor_product_gauss_legendre(2, 1).unwrap();

    for (name, potential) in [
        ("absent", None),
        (
            "foreign",
            Some(foreign_model.load_potential_expression()),
        ),
    ] {
        let error = admitted
            .evaluate(
                &wrong_geometry,
                &wrong_quadrature,
                3.0,
                2.0,
                potential,
            )
            .expect_err("derived primal must reject a non-certificate load");
        assert!(
            error.message().contains("load") || error.message().contains("certificate"),
            "{name} potential reached realization validation first: {error:?}"
        );
    }

    assert!(
        admitted
            .evaluate(
                &geometry(),
                &quadrature,
                3.0,
                2.0,
                Some(exact_model.load_potential_expression()),
            )
            .is_ok()
    );
}

#[test]
fn derivation_rejects_ambiguous_incomplete_foreign_and_mixed_roles() {
    let duplicate_unknown = SOURCE.replace(
        "field displacement on body as space: m shape spatial_vector;",
        concat!(
            "field displacement on body as space: m shape spatial_vector;\n",
            "  field foreign_displacement on body as space: m shape spatial_vector;",
        ),
    );
    assert_derivation_rejects(&duplicate_unknown, "duplicate displacement role");

    let foreign_parameter = SOURCE.replace(
        "parameter pressure_gradient: kg / (m ^ 2 * s ^ 2) = 1;",
        concat!(
            "parameter pressure_gradient: kg / (m ^ 2 * s ^ 2) = 1;\n",
            "  parameter foreign_parameter: 1 = 7;",
        ),
    );
    assert_derivation_rejects(&foreign_parameter, "foreign parameter role");

    let duplicate_volume = SOURCE.replace(
        "  relation x_lower_value continuous on x_lower",
        concat!(
            "  relation foreign_balance continuous on body {\n",
            "    -div(2 * mu * symmetric_part(grad(displacement))\n",
            "      + lambda * isotropic_lift(div(displacement)))\n",
            "      - grad(load_potential) = 0;\n",
            "  }\n\n",
            "  relation x_lower_value continuous on x_lower",
        ),
    );
    assert_derivation_rejects(&duplicate_volume, "ambiguous volume relation");

    let incomplete_boundary = SOURCE.replace(
        "  relation y_upper_value continuous on y_upper { trace(displacement) = 0; }\n",
        "",
    );
    assert_derivation_rejects(&incomplete_boundary, "incomplete boundary role");

    let duplicate_boundary = SOURCE.replace(
        "  relation y_upper_value continuous on y_upper { trace(displacement) = 0; }",
        concat!(
            "  relation y_upper_value continuous on y_upper { trace(displacement) = 0; }\n",
            "  relation y_upper_duplicate continuous on y_upper { trace(displacement) = 0; }",
        ),
    );
    assert_derivation_rejects(&duplicate_boundary, "ambiguous boundary role");

    let mixed_boundary = SOURCE.replace(
        "relation x_upper_value continuous on x_upper { trace(displacement) = 0; }",
        concat!(
            "relation x_upper_value continuous on x_upper {\n",
            "    normal(2 * mu * symmetric_part(grad(displacement))\n",
            "      + lambda * isotropic_lift(div(displacement))) = 0;\n",
            "  }",
        ),
    );
    assert_derivation_rejects(&mixed_boundary, "mixed boundary role");

    let mut oversized_load = String::from("pressure_gradient * coordinate(0)");
    oversized_load.push_str(&" + 0".repeat(4_097));
    let oversized = SOURCE.replace("pressure_gradient * coordinate(0)", &oversized_load);
    assert_derivation_rejects(&oversized, "bounded compiler resources");
}

#[test]
fn certificate_replay_rejects_stale_relation_node_and_test_trial_slot() {
    let quadrature = quadrature();

    let mut stale_relation = derived_form();
    let relation = stale_relation
        .certificate
        .entries
        .iter()
        .find(|entry| entry.relation != stale_relation.certificate.entries[0].relation)
        .expect("elasticity certificate owns volume and boundary Relations")
        .relation;
    stale_relation.certificate.entries[0].relation = relation;
    assert!(stale_relation.admit_quadrature(&quadrature).is_err());

    let mut stale_node = derived_form();
    let source_node = stale_node
        .certificate
        .entries
        .iter()
        .find(|entry| entry.source_node != stale_node.certificate.entries[0].source_node)
        .expect("elasticity certificate owns distinct derivation nodes")
        .source_node;
    stale_node.certificate.entries[0].source_node = source_node;
    assert!(stale_node.admit_quadrature(&quadrature).is_err());

    let mut slot_mutant = derived_form();
    let bilinear = slot_mutant
        .certificate
        .entries
        .iter()
        .position(|entry| matches!(entry.slot, WeakTermSlot::Bilinear { .. }))
        .expect("elasticity certificate owns a bilinear test/trial entry");
    slot_mutant.certificate.entries[bilinear].slot = WeakTermSlot::Bilinear {
        test: MatrixSlot::Trial,
        trial: MatrixSlot::Test,
    };
    assert!(slot_mutant.admit_quadrature(&quadrature).is_err());

    let mut sign_mutant = derived_form();
    let positive = sign_mutant
        .certificate
        .entries
        .iter()
        .position(|entry| entry.sign == WeakSign::Positive)
        .expect("elasticity certificate owns a positive weak term");
    sign_mutant.certificate.entries[positive].sign = WeakSign::Negative;
    assert!(sign_mutant.admit_quadrature(&quadrature).is_err());
}

fn actions(
    parameters: [f64; 3],
    state: [f64; 8],
    state_direction: [f64; 8],
    parameter_direction: [f64; 3],
    cotangent: [f64; 8],
) -> CartesianElasticityDifferentialActions2d {
    let form = derived_form();
    let quadrature = quadrature();
    form.admit_quadrature(&quadrature)
        .unwrap()
        .evaluate_with_actions(
            &geometry(),
            &quadrature,
            parameters,
            &state,
            &state_direction,
            parameter_direction,
            &cotangent,
        )
        .unwrap()
}

fn residual(parameters: [f64; 3], state: [f64; 8]) -> [f64; 8] {
    *actions(parameters, state, [0.0; 8], [0.0; 3], [0.0; 8]).residual()
}

fn derived_form() -> DerivedCartesianQ1ElasticityForm2d {
    derive_cartesian_q1_elasticity_form_2d(&compile_program(SOURCE)).unwrap()
}

fn compile_program(source: &str) -> KernelProgram {
    let mut compiled = compile("compiled-cartesian-elasticity.eqi", source).unwrap();
    let (transaction, model, _) = compiled.remove(0).into_parts();
    let mut store = InMemoryGraphStore::new();
    store.commit(transaction).unwrap();
    KernelProgram::from_snapshot(&store.snapshot(), model).unwrap()
}

fn assert_derivation_rejects(source: &str, mutation: &str) {
    let program = compile_program(source);
    let error = derive_cartesian_q1_elasticity_form_2d(&program)
        .expect_err("structural mutant must fail closed");
    assert!(
        error.message().contains("form compiler"),
        "{mutation} escaped the compiler gate with an unrelated diagnostic: {error:?}"
    );
}

fn geometry() -> AffineGeometryMap {
    AffineGeometryMap::new(
        ReferenceCell::hypercube(2).unwrap(),
        2,
        vec![0.25, 0.25],
        vec![0.25, 0.0, 0.0, 0.25],
    )
    .unwrap()
}

fn quadrature() -> QuadratureRule {
    QuadratureRule::tensor_product_gauss_legendre(2, 2).unwrap()
}

fn centered_vector<const N: usize>(plus: [f64; N], minus: [f64; N]) -> [f64; N] {
    array::from_fn(|index| centered_scalar(plus[index], minus[index]))
}

fn centered_scalar(plus: f64, minus: f64) -> f64 {
    (plus - minus) / (2.0 * STEP)
}

fn dot(left: &[f64], right: &[f64]) -> f64 {
    left.iter().zip(right).map(|(left, right)| left * right).sum()
}

fn matrix_action(matrix: &[f64], input: &[f64; 8]) -> [f64; 8] {
    array::from_fn(|row| {
        matrix[row * 8..(row + 1) * 8]
            .iter()
            .zip(input)
            .map(|(entry, input)| entry * input)
            .sum()
    })
}

fn permute_matrix(matrix: &[f64], permutation: [usize; 8]) -> [f64; 64] {
    array::from_fn(|index| {
        let row = index / 8;
        let column = index % 8;
        matrix[permutation[row] * 8 + permutation[column]]
    })
}

fn assert_absolute_scalar(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() <= ABSOLUTE_TOLERANCE,
        "actual={actual:e}, expected={expected:e}, error={:e}, tolerance={ABSOLUTE_TOLERANCE:e}",
        (actual - expected).abs(),
    );
}

fn assert_absolute_slice(actual: &[f64], expected: &[f64]) {
    assert_eq!(actual.len(), expected.len());
    for (actual, expected) in actual.iter().copied().zip(expected.iter().copied()) {
        assert_absolute_scalar(actual, expected);
    }
}

fn assert_rejected_numeric_mutant(actual: &[f64], expected: &[f64], mutation: &str) {
    assert_eq!(actual.len(), expected.len());
    let maximum_error = actual
        .iter()
        .zip(expected)
        .map(|(actual, expected)| (actual - expected).abs())
        .fold(0.0, f64::max);
    assert!(
        maximum_error > ABSOLUTE_TOLERANCE,
        "{mutation} survived the exact oracle: error={maximum_error:e}"
    );
}
