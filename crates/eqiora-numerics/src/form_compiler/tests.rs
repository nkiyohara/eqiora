use std::f64::consts::PI;

use eqiora_compiler::compile;
use eqiora_graph::{GraphStore, InMemoryGraphStore};
use eqiora_meshing::{AffineGeometryMap, QuadratureRule, ReferenceCell};
use eqiora_schema::kernel::{DomainKind, KernelNode};
use eqiora_sem::KernelProgram;

use super::*;

const SOURCE: &str = include_str!("../../../../packages/org.example.poisson/src/main.eqi");
const MATRIX: [f64; 16] = [
    0.833_333_333_333_333_3,
    -0.583_333_333_333_333_3,
    0.166_666_666_666_666_66,
    -0.416_666_666_666_666_63,
    -0.583_333_333_333_333_3,
    0.833_333_333_333_333_3,
    -0.416_666_666_666_666_63,
    0.166_666_666_666_666_66,
    0.166_666_666_666_666_66,
    -0.416_666_666_666_666_63,
    0.833_333_333_333_333_3,
    -0.583_333_333_333_333_3,
    -0.416_666_666_666_666_63,
    0.166_666_666_666_666_66,
    -0.583_333_333_333_333_3,
    0.833_333_333_333_333_3,
];
const LOAD: [f64; 4] = [
    0.425_495_714_381_214_03,
    0.474_820_601_775_891_64,
    0.242_871_390_835_767_61,
    0.271_025_855_380_221_5,
];
const MATRIX_RELATIVE: f64 = 1.0e-14;
const LOAD_RELATIVE: f64 = 2.5e-2;
const ACTION_RELATIVE: f64 = 1.0e-7;
const STEP: f64 = 1.0e-6;

#[test]
fn e1_element_fixture_matches_frozen_matrix_loads_and_certificate() {
    let form = compiled_form(SOURCE);
    let quadrature = QuadratureRule::tensor_product_gauss_legendre(2, 2).unwrap();
    let admitted = form.admit_quadrature(&quadrature).unwrap();
    let geometry = fixture_geometry();
    let source = |point: &[f64]| 2.0 * PI.powi(2) * (PI * point[0]).sin() * (PI * point[1]).sin();
    let local = admitted
        .evaluate(&geometry, &quadrature, 1.0, &source)
        .unwrap();
    assert_relative_slice(local.matrix(), &MATRIX, MATRIX_RELATIVE);
    assert_relative_slice(local.rhs(), &LOAD, LOAD_RELATIVE);

    let constant = admitted
        .evaluate(&geometry, &quadrature, 1.0, &|_: &[f64]| 1.0)
        .unwrap();
    assert_relative_slice(constant.rhs(), &[0.031_25; 4], MATRIX_RELATIVE);
    assert_matrix_invariants(local.matrix());

    let rules = form
        .certificate
        .entries
        .iter()
        .map(|entry| entry.rule_id)
        .collect::<Vec<_>>();
    assert_eq!(
        rules,
        [
            TEST_PAIRING,
            DIVERGENCE_BY_PARTS,
            HOMOGENEOUS_ESSENTIAL_DISCHARGE,
            HOMOGENEOUS_ESSENTIAL_DISCHARGE,
            HOMOGENEOUS_ESSENTIAL_DISCHARGE,
            HOMOGENEOUS_ESSENTIAL_DISCHARGE,
            SOURCE_PAIRING,
        ]
    );
    assert_eq!(form.parameters.len(), 2);
    form.validate_certificate().unwrap();
}

#[test]
fn e1_rejects_certificate_sign_slot_quadrature_aspect_and_coefficient_mutants() {
    let form = compiled_form(SOURCE);
    let quadrature = QuadratureRule::tensor_product_gauss_legendre(2, 2).unwrap();
    let geometry = fixture_geometry();
    let source = |point: &[f64]| 2.0 * PI.powi(2) * (PI * point[0]).sin() * (PI * point[1]).sin();
    let admitted = form.admit_quadrature(&quadrature).unwrap();
    let local = admitted
        .evaluate(&geometry, &quadrature, 1.0, &source)
        .unwrap();

    let mut sign_mutant = form.clone();
    sign_mutant.certificate.entries[1].sign = WeakSign::Negative;
    assert_gate(sign_mutant.validate_certificate(), "derivation certificate");

    let mut slot_mutant = form.clone();
    slot_mutant.certificate.entries[1].slot = WeakTermSlot::Bilinear {
        test: MatrixSlot::Trial,
        trial: MatrixSlot::Test,
    };
    assert_gate(slot_mutant.validate_certificate(), "derivation certificate");

    let under_integrated = QuadratureRule::tensor_product_gauss_legendre(2, 1).unwrap();
    assert_gate(
        form.admit_quadrature(&under_integrated).map(|_| ()),
        "realization compatibility",
    );

    let aspect_mutant = permute_matrix(local.matrix(), [0, 2, 1, 3]);
    assert!(maximum_relative(&aspect_mutant, &MATRIX) > MATRIX_RELATIVE);

    let scaled = admitted
        .evaluate(&geometry, &quadrature, 2.0, &source)
        .unwrap();
    assert!(maximum_relative(scaled.matrix(), &MATRIX) > MATRIX_RELATIVE);
}

#[test]
fn e1_independent_actions_reject_corrupted_jvp_and_primal_as_vjp() {
    let form = compiled_form(SOURCE);
    let quadrature = QuadratureRule::tensor_product_gauss_legendre(2, 2).unwrap();
    let source = |point: &[f64]| 2.0 * PI.powi(2) * (PI * point[0]).sin() * (PI * point[1]).sin();
    let local = form
        .admit_quadrature(&quadrature)
        .unwrap()
        .evaluate(&fixture_geometry(), &quadrature, 1.0, &source)
        .unwrap();
    let direction = normalized_direction(4);
    let state = direction.clone();

    let finite_difference =
        centered_residual_action(local.matrix(), local.rhs(), &state, &direction);
    let exact_jvp = matrix_action(local.matrix(), &direction);
    assert_relative_slice(&exact_jvp, &finite_difference, ACTION_RELATIVE);

    let mut corrupted = local.matrix().to_vec();
    for row in 0..4 {
        corrupted[row * 4 + 1] = 0.0;
    }
    let mutant_jvp = matrix_action(&corrupted, &direction);
    assert!(maximum_relative(&mutant_jvp, &finite_difference) > ACTION_RELATIVE);

    let directional_vjp = centered_scalar_gradient(local.matrix(), local.rhs(), &state, &direction);
    let exact_vjp = transpose_action(local.matrix(), &direction);
    assert_relative_slice(&exact_vjp, &directional_vjp, ACTION_RELATIVE);
    let primal_as_vjp = residual(local.matrix(), local.rhs(), &state);
    assert!(maximum_relative(&primal_as_vjp, &directional_vjp) > ACTION_RELATIVE);
}

#[test]
fn ineligible_natural_boundary_retains_the_existing_path() {
    let source = SOURCE.replace(
        "relation x_lower_value continuous on x_lower { trace(potential) = 0; }",
        "relation x_lower_value continuous on x_lower { normal(grad(potential)) = 0; }",
    );
    let program = compile_program(&source);
    let domain = box_domain(&program);
    assert!(derive_candidate(&program, domain).unwrap().is_none());
    crate::canonical::lower_scalar_elliptic_cartesian(&program).unwrap();
}

fn compiled_form(source: &str) -> DerivedScalarGalerkinForm {
    let program = compile_program(source);
    let domain = box_domain(&program);
    derive_candidate(&program, domain)
        .unwrap()
        .expect("exact package is compiler-eligible")
}

fn box_domain(program: &KernelProgram) -> RawId {
    program
        .nodes()
        .find_map(|node| match node {
            KernelNode::Domain(domain)
                if matches!(domain.kind(), DomainKind::CartesianBox { .. }) =>
            {
                Some(domain.id().erase())
            }
            _ => None,
        })
        .unwrap()
}

fn compile_program(source: &str) -> KernelProgram {
    let mut compiled = compile("compiled-poisson.eqi", source).unwrap();
    let (transaction, model, _) = compiled.remove(0).into_parts();
    let mut store = InMemoryGraphStore::new();
    store.commit(transaction).unwrap();
    KernelProgram::from_snapshot(&store.snapshot(), model).unwrap()
}

fn fixture_geometry() -> AffineGeometryMap {
    AffineGeometryMap::new(
        ReferenceCell::hypercube(2).unwrap(),
        2,
        vec![0.375, 0.75],
        vec![0.125, 0.0, 0.0, 0.25],
    )
    .unwrap()
}

fn assert_matrix_invariants(matrix: &[f64]) {
    for row in 0..4 {
        assert!(matrix[row * 4 + row] > 0.0);
        assert!(
            matrix[row * 4..row * 4 + 4].iter().sum::<f64>().abs()
                <= MATRIX_RELATIVE * matrix[row * 4 + row].abs()
        );
        for column in 0..4 {
            let scale = matrix[row * 4 + column]
                .abs()
                .max(matrix[column * 4 + row].abs());
            assert!(
                (matrix[row * 4 + column] - matrix[column * 4 + row]).abs()
                    <= MATRIX_RELATIVE * scale
            );
        }
    }
}

fn assert_gate(result: Result<(), Diagnostic>, gate: &str) {
    let error = result.expect_err("mutant must fail closed");
    assert!(error.message().contains(gate), "{error:?}");
}

fn assert_relative_slice(actual: &[f64], expected: &[f64], tolerance: f64) {
    assert_eq!(actual.len(), expected.len());
    assert!(
        maximum_relative(actual, expected) <= tolerance,
        "actual={actual:?}, expected={expected:?}"
    );
}

fn maximum_relative(actual: &[f64], expected: &[f64]) -> f64 {
    actual
        .iter()
        .zip(expected)
        .map(|(actual, expected)| (actual - expected).abs() / expected.abs().max(f64::MIN_POSITIVE))
        .fold(0.0, f64::max)
}

fn permute_matrix(matrix: &[f64], permutation: [usize; 4]) -> Vec<f64> {
    let mut permuted = vec![0.0; 16];
    for row in 0..4 {
        for column in 0..4 {
            permuted[row * 4 + column] = matrix[permutation[row] * 4 + permutation[column]];
        }
    }
    permuted
}

fn normalized_direction(dimension: usize) -> Vec<f64> {
    let mut direction = (0..dimension)
        .map(|index| ((index + 1) as f64).sin())
        .collect::<Vec<_>>();
    let norm = direction
        .iter()
        .map(|value| value * value)
        .sum::<f64>()
        .sqrt();
    direction.iter_mut().for_each(|value| *value /= norm);
    direction
}

fn residual(matrix: &[f64], rhs: &[f64], state: &[f64]) -> Vec<f64> {
    matrix_action(matrix, state)
        .into_iter()
        .zip(rhs)
        .map(|(value, rhs)| value - rhs)
        .collect()
}

fn matrix_action(matrix: &[f64], input: &[f64]) -> Vec<f64> {
    (0..input.len())
        .map(|row| {
            matrix[row * input.len()..(row + 1) * input.len()]
                .iter()
                .zip(input)
                .map(|(entry, input)| entry * input)
                .sum()
        })
        .collect()
}

fn transpose_action(matrix: &[f64], input: &[f64]) -> Vec<f64> {
    let dimension = input.len();
    (0..dimension)
        .map(|column| {
            input
                .iter()
                .enumerate()
                .map(|(row, value)| matrix[row * dimension + column] * value)
                .sum()
        })
        .collect()
}

fn centered_residual_action(
    matrix: &[f64],
    rhs: &[f64],
    state: &[f64],
    direction: &[f64],
) -> Vec<f64> {
    let plus = state
        .iter()
        .zip(direction)
        .map(|(state, direction)| state + STEP * direction)
        .collect::<Vec<_>>();
    let minus = state
        .iter()
        .zip(direction)
        .map(|(state, direction)| state - STEP * direction)
        .collect::<Vec<_>>();
    residual(matrix, rhs, &plus)
        .into_iter()
        .zip(residual(matrix, rhs, &minus))
        .map(|(plus, minus)| (plus - minus) / (2.0 * STEP))
        .collect()
}

fn centered_scalar_gradient(
    matrix: &[f64],
    rhs: &[f64],
    state: &[f64],
    cotangent: &[f64],
) -> Vec<f64> {
    (0..state.len())
        .map(|coordinate| {
            let mut plus = state.to_vec();
            let mut minus = state.to_vec();
            plus[coordinate] += STEP;
            minus[coordinate] -= STEP;
            let scalar = |point: &[f64]| {
                residual(matrix, rhs, point)
                    .iter()
                    .zip(cotangent)
                    .map(|(residual, cotangent)| residual * cotangent)
                    .sum::<f64>()
            };
            (scalar(&plus) - scalar(&minus)) / (2.0 * STEP)
        })
        .collect()
}
