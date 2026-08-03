use std::collections::{BTreeSet, HashSet};
use std::num::NonZeroUsize;

use eqiora::Id;
use eqiora::api::{
    CompleteParameterStudy, DifferentiableEvaluation, DifferentiableProgram, ModelDocument,
    ParameterStudyPlan, ParameterStudyPointKey, ParameterStudyTerminalReport,
    ScalarEllipticExecutionEnvironment, ScalarEllipticIntent, ScalarEllipticMethod,
};
use eqiora::diagnostic::codes;
use eqiora::entity::kinds;
use eqiora::realization::RealizationRevision;

const SOURCE: &str =
    include_str!("../../../verify/differentiation/spatial-poisson-fem-fvm/models/poisson.eqi");
const DEFAULT_POINT: [f64; 3] = [19.739208802178716, 1.0, 0.0];
const PERMUTED_DIFFUSION: [f64; 3] = [1.25, 0.75, 1.0];
const CANONICAL_DIFFUSION: [f64; 3] = [0.75, 1.0, 1.25];

struct Fixture {
    document: ModelDocument,
    program: DifferentiableProgram,
    source_scale: Id<kinds::Parameter>,
    diffusion: Id<kinds::Parameter>,
    boundary_offset: Id<kinds::Parameter>,
    wave_number: Id<kinds::Parameter>,
}

#[test]
fn study_matches_separately_accepted_evaluations_in_canonical_order() {
    let fixture = build_fixture(ScalarEllipticMethod::FiniteElement);
    let expected = independently_evaluate(&fixture.program, &CANONICAL_DIFFUSION);
    let plan = ParameterStudyPlan::new(&fixture.program, fixture.diffusion, &PERMUTED_DIFFUSION)
        .expect("the frozen three-point inventory is admissible without evaluation");

    assert!(plan.program_identity() == fixture.program.identity());
    assert_eq!(plan.varying_parameter(), fixture.diffusion);
    assert_eq!(
        plan.program_identity().inputs(),
        &[
            fixture.source_scale,
            fixture.diffusion,
            fixture.boundary_offset,
        ]
    );
    assert_point_bits(plan.base_point().values(), &DEFAULT_POINT);
    assert_point_bits(
        plan.base_point().values(),
        fixture.program.default_point().values(),
    );
    assert_eq!(
        plan.base_point().inputs(),
        fixture.program.default_point().inputs()
    );
    assert_keys(plan.point_keys(), fixture.diffusion, &CANONICAL_DIFFUSION);

    let complete = plan
        .execute()
        .unwrap_or_else(|_| panic!("the accepted three-point study must complete"));
    assert_complete_matches(&complete, &plan, &expected);

    let replacement =
        ParameterStudyPlan::new(&fixture.program, fixture.diffusion, &[0.75, 1.0, 1.5])
            .expect("the replacement inventory remains structurally valid");
    assert!(replacement != plan);
    let replacement_key = replacement
        .point_keys()
        .iter()
        .find(|key| key.value().to_bits() == 1.5f64.to_bits())
        .expect("the replacement key is observable");
    assert!(complete.evaluation(replacement_key).is_none());
}

#[test]
fn every_caller_permutation_has_one_plan_and_one_member_order() {
    let fixture = build_fixture(ScalarEllipticMethod::FiniteElement);
    let expected = independently_evaluate(&fixture.program, &CANONICAL_DIFFUSION);
    let permutations = [
        [0.75, 1.0, 1.25],
        [0.75, 1.25, 1.0],
        [1.0, 0.75, 1.25],
        [1.0, 1.25, 0.75],
        [1.25, 0.75, 1.0],
        [1.25, 1.0, 0.75],
    ];
    let reference_plan =
        ParameterStudyPlan::new(&fixture.program, fixture.diffusion, &permutations[0])
            .expect("the reference permutation is valid");

    for values in permutations {
        let plan = ParameterStudyPlan::new(&fixture.program, fixture.diffusion, &values)
            .expect("every exact permutation is valid");
        assert!(plan == reference_plan);
        let complete = plan
            .execute()
            .unwrap_or_else(|_| panic!("permutation must not affect execution"));
        assert_complete_matches(&complete, &reference_plan, &expected);
    }
}

#[test]
fn planning_rejects_every_frozen_inventory_and_identity_mutant() {
    let fixture = build_fixture(ScalarEllipticMethod::FiniteElement);

    assert!(
        ParameterStudyPlan::new(&fixture.program, fixture.diffusion, &[0.75, 1.0, 1.0]).is_err(),
        "exact-bit duplicates reject instead of being silently removed into a valid inventory"
    );
    assert!(
        ParameterStudyPlan::new(&fixture.program, fixture.diffusion, &[0.75, 1.25]).is_err(),
        "the exact default anchor is mandatory"
    );
    assert!(
        ParameterStudyPlan::new(&fixture.program, fixture.diffusion, &[1.0]).is_err(),
        "one point is below the bound"
    );
    assert!(
        ParameterStudyPlan::new(&fixture.program, fixture.diffusion, &[0.75, 1.0, f64::NAN],)
            .is_err(),
        "NaN rejects instead of being filtered from an otherwise valid inventory"
    );
    assert!(
        ParameterStudyPlan::new(
            &fixture.program,
            fixture.diffusion,
            &[0.75, 1.0, f64::INFINITY],
        )
        .is_err(),
        "infinity rejects instead of being filtered from an otherwise valid inventory"
    );
    assert!(
        ParameterStudyPlan::new(&fixture.program, fixture.wave_number, &[1.0, 2.0]).is_err(),
        "a same-Model Parameter outside the program input coordinates rejects"
    );

    let foreign_source = SOURCE.replacen(
        "model differentiated_poisson_plane",
        "model foreign_differentiated_poisson_plane",
        1,
    );
    let foreign_document =
        ModelDocument::compile("foreign-bounded-parameter-study.eqi", &foreign_source).unwrap();
    let foreign_diffusion = foreign_document.parameter_ref("diffusion").unwrap().id();
    assert_ne!(foreign_diffusion, fixture.diffusion);
    assert!(
        ParameterStudyPlan::new(&fixture.program, foreign_diffusion, &[0.75, 1.0]).is_err(),
        "a foreign-Model Parameter rejects even when its source alias agrees"
    );

    let sixty_four = (0..64)
        .map(|index| 1.0 + index as f64 / 64.0)
        .collect::<Vec<_>>();
    let maximum =
        ParameterStudyPlan::new(&fixture.program, fixture.diffusion, &sixty_four).unwrap();
    assert_eq!(maximum.point_keys().len(), 64);

    let sixty_five = (0..65)
        .map(|index| 1.0 + index as f64 / 64.0)
        .collect::<Vec<_>>();
    assert!(
        ParameterStudyPlan::new(&fixture.program, fixture.diffusion, &sixty_five).is_err(),
        "65 unique finite anchored points exceed the bound"
    );
}

#[test]
fn point_keys_use_parameter_identity_exact_bits_and_total_order() {
    let fixture = build_fixture(ScalarEllipticMethod::FiniteElement);
    let signed_zero_plan =
        ParameterStudyPlan::new(&fixture.program, fixture.diffusion, &[-0.0, 0.0, 1.0])
            .expect("planning is structural and retains both signed zeros");
    assert_keys(
        signed_zero_plan.point_keys(),
        fixture.diffusion,
        &[-0.0, 0.0, 1.0],
    );
    assert!(signed_zero_plan.point_keys()[0] != signed_zero_plan.point_keys()[1]);
    assert!(signed_zero_plan.point_keys()[0] < signed_zero_plan.point_keys()[1]);
    assert_eq!(
        signed_zero_plan
            .point_keys()
            .iter()
            .collect::<HashSet<_>>()
            .len(),
        3
    );
    assert_eq!(
        signed_zero_plan
            .point_keys()
            .iter()
            .collect::<BTreeSet<_>>()
            .len(),
        3
    );

    let source_plan = ParameterStudyPlan::new(
        &fixture.program,
        fixture.source_scale,
        &[DEFAULT_POINT[0], 1.0],
    )
    .expect("another already selected coordinate may own a study");
    let source_one = key_with_bits(source_plan.point_keys(), 1.0f64.to_bits());
    let diffusion_one = key_with_bits(signed_zero_plan.point_keys(), 1.0f64.to_bits());
    assert!(source_one != diffusion_one);
    assert_eq!(
        source_one.cmp(diffusion_one),
        fixture.source_scale.ulid().cmp(&fixture.diffusion.ulid())
    );

    let fvm_program = program_for(&fixture.document, ScalarEllipticMethod::FiniteVolume);
    let fvm_plan = ParameterStudyPlan::new(&fvm_program, fixture.diffusion, &PERMUTED_DIFFUSION)
        .expect("the accepted FVM program admits the same coordinate keys");
    let fem_plan =
        ParameterStudyPlan::new(&fixture.program, fixture.diffusion, &PERMUTED_DIFFUSION)
            .expect("the accepted FEM program admits the reference study");
    assert!(fem_plan != fvm_plan);
    assert!(fem_plan.point_keys() == fvm_plan.point_keys());
    let complete = fem_plan
        .execute()
        .unwrap_or_else(|_| panic!("the accepted FEM study must complete"));
    let fvm_one = key_with_bits(fvm_plan.point_keys(), 1.0f64.to_bits());
    assert!(
        complete.evaluation(fvm_one).is_some(),
        "an equal program-agnostic coordinate key resolves without importing FVM identity"
    );
}

#[test]
fn point_failure_is_terminal_and_preserves_original_diagnostics() {
    let fixture = build_fixture(ScalarEllipticMethod::FiniteElement);
    let failed_point = complete_point(-1.0);
    let original = fixture
        .program
        .evaluate(&failed_point)
        .expect_err("negative diffusion is rejected by the accepted point evaluator");
    let plan = ParameterStudyPlan::new(&fixture.program, fixture.diffusion, &[-1.0, 1.0])
        .expect("planning does not perform physical evaluation");

    let report = match plan.execute() {
        Ok(_) => panic!("one rejected point cannot publish a complete study"),
        Err(report) => report,
    };
    assert!(report.plan() == &plan);
    assert!(report.completed_point_keys().is_empty());
    assert_key(
        report
            .failed_point_key()
            .expect("a point failure names its exact key"),
        fixture.diffusion,
        -1.0,
    );
    assert_eq!(report.diagnostics(), original.as_slice());
    assert!(!report.is_cancelled());
}

#[test]
fn cancellation_is_observed_only_before_or_between_point_evaluations() {
    let fixture = build_fixture(ScalarEllipticMethod::FiniteElement);
    let plan = ParameterStudyPlan::new(&fixture.program, fixture.diffusion, &PERMUTED_DIFFUSION)
        .expect("the reference plan is valid");

    let mut before_first_polls = 0;
    let before_first = match plan.execute_with_cancellation(|| {
        before_first_polls += 1;
        true
    }) {
        Ok(_) => panic!("cancellation before point zero cannot complete"),
        Err(report) => report,
    };
    assert_eq!(before_first_polls, 1);
    assert_cancelled(&before_first, &plan, &[]);

    let mut after_first_polls = 0;
    let after_first = match plan.execute_with_cancellation(|| {
        after_first_polls += 1;
        after_first_polls == 2
    }) {
        Ok(_) => panic!("cancellation after point zero cannot complete"),
        Err(report) => report,
    };
    assert_eq!(after_first_polls, 2);
    assert_cancelled(&after_first, &plan, &[0.75]);

    let mut completion_polls = 0;
    let complete = plan
        .execute_with_cancellation(|| {
            completion_polls += 1;
            completion_polls > CANONICAL_DIFFUSION.len()
        })
        .unwrap_or_else(|_| panic!("completion wins after the final accepted point"));
    assert_eq!(completion_polls, CANONICAL_DIFFUSION.len());
    assert_keys(
        complete.point_keys(),
        fixture.diffusion,
        &CANONICAL_DIFFUSION,
    );
}

#[test]
fn repeated_direct_point_evaluation_remains_isolated_around_an_alternate_point() {
    let fixture = build_fixture(ScalarEllipticMethod::FiniteElement);
    let first = fixture.program.evaluate(&complete_point(0.75)).unwrap();
    let alternate = fixture.program.evaluate(&complete_point(1.25)).unwrap();
    let repeated = fixture.program.evaluate(&complete_point(0.75)).unwrap();

    assert_evaluation_matches(&repeated, &first);
    assert_ne!(
        first.point().values()[1].to_bits(),
        alternate.point().values()[1].to_bits()
    );
}

fn build_fixture(method: ScalarEllipticMethod) -> Fixture {
    let document = ModelDocument::compile("bounded-parameter-study.eqi", SOURCE).unwrap();
    let source_scale = document.parameter_ref("source_scale").unwrap().id();
    let diffusion = document.parameter_ref("diffusion").unwrap().id();
    let boundary_offset = document.parameter_ref("boundary_offset").unwrap().id();
    let wave_number = document.parameter_ref("wave_number").unwrap().id();
    let program = program_for(&document, method);
    Fixture {
        document,
        program,
        source_scale,
        diffusion,
        boundary_offset,
        wave_number,
    }
}

fn program_for(document: &ModelDocument, method: ScalarEllipticMethod) -> DifferentiableProgram {
    let environment = ScalarEllipticExecutionEnvironment::host_serial();
    let intent = ScalarEllipticIntent::new(
        RealizationRevision::new(21),
        method,
        NonZeroUsize::new(12).unwrap(),
        NonZeroUsize::MIN,
    );
    let plan = document
        .preview_scalar_elliptic_run(intent, environment)
        .unwrap();
    let inputs = [
        document.parameter_ref("source_scale").unwrap(),
        document.parameter_ref("diffusion").unwrap(),
        document.parameter_ref("boundary_offset").unwrap(),
    ];
    let output = document.field_ref("potential").unwrap();
    DifferentiableProgram::compile(document, plan, &inputs, &output).unwrap()
}

fn complete_point(diffusion: f64) -> [f64; 3] {
    [DEFAULT_POINT[0], diffusion, DEFAULT_POINT[2]]
}

fn independently_evaluate(
    program: &DifferentiableProgram,
    diffusion_values: &[f64],
) -> Vec<DifferentiableEvaluation> {
    diffusion_values
        .iter()
        .map(|value| {
            program
                .evaluate(&complete_point(*value))
                .expect("the independent accepted-point execution succeeds")
        })
        .collect()
}

fn assert_complete_matches(
    complete: &CompleteParameterStudy,
    plan: &ParameterStudyPlan,
    expected: &[DifferentiableEvaluation],
) {
    assert!(complete.plan() == plan);
    assert!(complete.point_keys() == plan.point_keys());
    assert_eq!(complete.members().len(), expected.len());
    for ((key, member), expected) in complete
        .point_keys()
        .iter()
        .zip(complete.members())
        .zip(expected)
    {
        assert_evaluation_matches(member, expected);
        let looked_up = complete
            .evaluation(key)
            .expect("every canonical key resolves its one accepted member");
        assert_evaluation_matches(looked_up, expected);
    }
}

fn assert_evaluation_matches(
    actual: &DifferentiableEvaluation,
    expected: &DifferentiableEvaluation,
) {
    assert!(actual.identity() == expected.identity());
    assert_eq!(actual.point().inputs(), expected.point().inputs());
    assert_point_bits(actual.point().values(), expected.point().values());

    let actual_primal = actual.primal();
    let expected_primal = expected.primal();
    assert_point_bits(actual_primal.output(), expected_primal.output());
    assert_eq!(
        actual_primal.evidence().state_system(),
        expected_primal.evidence().state_system()
    );
    assert_eq!(
        actual_primal.evidence().receipt().output(),
        expected_primal.evidence().receipt().output()
    );
    assert_eq!(
        actual_primal.evidence().primal_solve(),
        expected_primal.evidence().primal_solve()
    );
    assert_eq!(actual_primal.evidence(), expected_primal.evidence());
}

fn assert_cancelled(
    report: &ParameterStudyTerminalReport,
    plan: &ParameterStudyPlan,
    completed_values: &[f64],
) {
    assert!(report.plan() == plan);
    assert_keys(
        report.completed_point_keys(),
        plan.varying_parameter(),
        completed_values,
    );
    assert!(report.failed_point_key().is_none());
    assert_eq!(report.diagnostics().len(), 1);
    assert_eq!(report.diagnostics()[0].code(), codes::EXECUTION_CANCELLED);
    assert!(report.is_cancelled());
}

fn assert_keys(keys: &[ParameterStudyPointKey], parameter: Id<kinds::Parameter>, values: &[f64]) {
    assert_eq!(keys.len(), values.len());
    for (key, value) in keys.iter().zip(values) {
        assert_key(key, parameter, *value);
    }
}

fn assert_key(key: &ParameterStudyPointKey, parameter: Id<kinds::Parameter>, value: f64) {
    assert_eq!(key.parameter(), parameter);
    assert_eq!(key.value().to_bits(), value.to_bits());
}

fn key_with_bits(keys: &[ParameterStudyPointKey], bits: u64) -> &ParameterStudyPointKey {
    keys.iter()
        .find(|key| key.value().to_bits() == bits)
        .expect("the exact key bits are present")
}

fn assert_point_bits(actual: &[f64], expected: &[f64]) {
    assert_eq!(actual.len(), expected.len());
    assert!(
        actual
            .iter()
            .zip(expected)
            .all(|(actual, expected)| actual.to_bits() == expected.to_bits())
    );
}
