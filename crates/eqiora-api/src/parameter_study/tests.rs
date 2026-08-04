use std::cell::Cell;
use std::num::NonZeroUsize;

use eqiora_core::Diagnostic;
use eqiora_core::diagnostic::codes;
use eqiora_realization::RealizationRevision;

use super::{CompleteParameterStudy, ParameterStudyPlan};
use crate::{
    DifferentiableEvaluation, DifferentiableProgram, ModelDocument,
    ScalarEllipticExecutionEnvironment, ScalarEllipticIntent, ScalarEllipticMethod,
};

const SOURCE: &str =
    include_str!("../../../../verify/differentiation/spatial-poisson-fem-fvm/models/poisson.eqi");
const DEFAULT_POINT: [f64; 3] = [19.739208802178716, 1.0, 0.0];
const PERMUTED_DIFFUSION: [f64; 3] = [1.25, 0.75, 1.0];
const CANONICAL_DIFFUSION: [f64; 3] = [0.75, 1.0, 1.25];

mod public_integration_oracle {
    mod eqiora {
        pub use eqiora_core::Id;

        pub mod api {
            pub use crate::{
                CompleteParameterStudy, DifferentiableEvaluation, DifferentiableProgram,
                ModelDocument, ParameterStudyPlan, ParameterStudyPointKey,
                ParameterStudyTerminalReport, ScalarEllipticExecutionEnvironment,
                ScalarEllipticIntent, ScalarEllipticMethod,
            };
        }

        pub mod diagnostic {
            pub use eqiora_core::diagnostic::codes;
        }

        pub mod entity {
            pub use eqiora_core::entity::kinds;
        }

        pub mod realization {
            pub use eqiora_realization::RealizationRevision;
        }
    }

    include!("../../../eqiora/tests/bounded_parameter_study.rs");
}

#[test]
fn registered_composition_oracle_executes_all_private_falsifiers() {
    public_integration_oracle::execute_all_public_authority_tests();
    injected_evaluator_is_called_once_per_reached_point_in_canonical_serial_order();
    injected_failure_stops_at_its_key_and_preserves_the_completed_prefix();
    injected_evaluator_proves_cancellation_boundaries_and_final_completion_priority();
    from_members_rejects_missing_duplicate_inserted_and_reordered_members();
    from_members_rejects_foreign_model_realization_method_program_and_point();
    evaluator_returning_another_point_cannot_substitute_a_member();
    evaluator_returning_a_foreign_program_member_fails_before_the_next_point();
}

#[test]
fn injected_evaluator_is_called_once_per_reached_point_in_canonical_serial_order() {
    let (document, program) = fixture();
    let diffusion = document.parameter_ref("diffusion").unwrap().id();
    let plan = ParameterStudyPlan::new(&program, diffusion, &PERMUTED_DIFFUSION).unwrap();
    let active_depth = Cell::new(0usize);
    let maximum_depth = Cell::new(0usize);
    let mut calls = Vec::new();
    let mut evaluator = |retained: &DifferentiableProgram, point: &[f64]| {
        assert!(retained.identity() == plan.program_identity());
        let depth = active_depth.get() + 1;
        active_depth.set(depth);
        maximum_depth.set(maximum_depth.get().max(depth));
        calls.push(point[1].to_bits());
        let result = retained.evaluate(point);
        active_depth.set(depth - 1);
        result
    };
    let mut never_cancel = || false;

    let complete = plan
        .execute_with_evaluator(&mut evaluator, &mut never_cancel)
        .unwrap_or_else(|_| panic!("the recording evaluator must complete"));
    assert_eq!(active_depth.get(), 0);
    assert_eq!(maximum_depth.get(), 1);
    assert_eq!(
        calls,
        CANONICAL_DIFFUSION
            .iter()
            .map(|value| value.to_bits())
            .collect::<Vec<_>>()
    );
    assert_member_points(complete.members(), &CANONICAL_DIFFUSION);
}

#[test]
fn injected_failure_stops_at_its_key_and_preserves_the_completed_prefix() {
    let (document, program) = fixture();
    let diffusion = document.parameter_ref("diffusion").unwrap().id();
    let plan = ParameterStudyPlan::new(&program, diffusion, &PERMUTED_DIFFUSION).unwrap();
    let injected = Diagnostic::error(codes::INVALID_LINEARIZATION, "injected point failure");
    let mut calls = Vec::new();
    let mut evaluator = |retained: &DifferentiableProgram, point: &[f64]| {
        calls.push(point[1].to_bits());
        if point[1].to_bits() == 1.0f64.to_bits() {
            Err(vec![injected.clone()])
        } else {
            retained.evaluate(point)
        }
    };
    let mut never_cancel = || false;

    let report = match plan.execute_with_evaluator(&mut evaluator, &mut never_cancel) {
        Ok(_) => panic!("an injected point failure cannot produce a complete study"),
        Err(report) => report,
    };
    assert_eq!(
        calls,
        [0.75f64.to_bits(), 1.0f64.to_bits()],
        "no later point is reached"
    );
    assert_key_values(report.completed_point_keys(), &[0.75]);
    assert_eq!(
        report
            .failed_point_key()
            .expect("the injected point failure has one key")
            .value()
            .to_bits(),
        1.0f64.to_bits()
    );
    assert_eq!(report.diagnostics(), &[injected]);
    assert!(!report.is_cancelled());
}

#[test]
fn injected_evaluator_proves_cancellation_boundaries_and_final_completion_priority() {
    let (document, program) = fixture();
    let diffusion = document.parameter_ref("diffusion").unwrap().id();
    let plan = ParameterStudyPlan::new(&program, diffusion, &PERMUTED_DIFFUSION).unwrap();

    let before_calls = Cell::new(0usize);
    let mut before_evaluator = |retained: &DifferentiableProgram, point: &[f64]| {
        before_calls.set(before_calls.get() + 1);
        retained.evaluate(point)
    };
    let mut cancel_immediately = || true;
    let before = match plan.execute_with_evaluator(&mut before_evaluator, &mut cancel_immediately) {
        Ok(_) => panic!("cancellation before point zero cannot complete"),
        Err(report) => report,
    };
    assert_eq!(before_calls.get(), 0);
    assert!(before.completed_point_keys().is_empty());
    assert!(before.is_cancelled());

    let after_first_calls = Cell::new(0usize);
    let after_first_polls = Cell::new(0usize);
    let mut after_first_evaluator = |retained: &DifferentiableProgram, point: &[f64]| {
        after_first_calls.set(after_first_calls.get() + 1);
        retained.evaluate(point)
    };
    let mut cancel_after_first = || {
        after_first_polls.set(after_first_polls.get() + 1);
        after_first_polls.get() == 2
    };
    let after_first =
        match plan.execute_with_evaluator(&mut after_first_evaluator, &mut cancel_after_first) {
            Ok(_) => panic!("cancellation after point zero cannot complete"),
            Err(report) => report,
        };
    assert_eq!(after_first_calls.get(), 1);
    assert_eq!(after_first_polls.get(), 2);
    assert_key_values(after_first.completed_point_keys(), &[0.75]);
    assert!(after_first.is_cancelled());

    let complete_calls = Cell::new(0usize);
    let completion_polls = Cell::new(0usize);
    let mut complete_evaluator = |retained: &DifferentiableProgram, point: &[f64]| {
        complete_calls.set(complete_calls.get() + 1);
        retained.evaluate(point)
    };
    let mut cancel_only_after_final = || {
        completion_polls.set(completion_polls.get() + 1);
        complete_calls.get() == CANONICAL_DIFFUSION.len()
    };
    let complete = plan
        .execute_with_evaluator(&mut complete_evaluator, &mut cancel_only_after_final)
        .unwrap_or_else(|_| panic!("no cancellation poll occurs after final completion"));
    assert_eq!(complete_calls.get(), CANONICAL_DIFFUSION.len());
    assert_eq!(completion_polls.get(), CANONICAL_DIFFUSION.len());
    assert_member_points(complete.members(), &CANONICAL_DIFFUSION);
}

#[test]
fn from_members_rejects_missing_duplicate_inserted_and_reordered_members() {
    let (document, program) = fixture();
    let diffusion = document.parameter_ref("diffusion").unwrap().id();
    let plan = ParameterStudyPlan::new(&program, diffusion, &PERMUTED_DIFFUSION).unwrap();
    let members = accepted_members(&program, &CANONICAL_DIFFUSION);
    assert!(CompleteParameterStudy::from_members(&plan, members.clone()).is_ok());

    let mut missing = members.clone();
    missing.remove(1);
    assert!(CompleteParameterStudy::from_members(&plan, missing).is_err());

    let mut duplicate = members.clone();
    duplicate[1] = duplicate[0].clone();
    assert!(CompleteParameterStudy::from_members(&plan, duplicate).is_err());

    let mut inserted = members.clone();
    inserted.push(program.evaluate(&complete_point(1.5)).unwrap());
    assert!(CompleteParameterStudy::from_members(&plan, inserted).is_err());

    let mut reordered = members;
    reordered.swap(0, 2);
    assert!(CompleteParameterStudy::from_members(&plan, reordered).is_err());
}

#[test]
fn from_members_rejects_foreign_model_realization_method_program_and_point() {
    let (document, program) = fixture();
    let diffusion = document.parameter_ref("diffusion").unwrap().id();
    let plan = ParameterStudyPlan::new(&program, diffusion, &PERMUTED_DIFFUSION).unwrap();
    let members = accepted_members(&program, &CANONICAL_DIFFUSION);

    let foreign_source = SOURCE.replacen(
        "model differentiated_poisson_plane",
        "model foreign_differentiated_poisson_plane",
        1,
    );
    let foreign_document =
        ModelDocument::compile("foreign-bounded-parameter-study.eqi", &foreign_source).unwrap();
    let foreign_model_program = program_for(
        &foreign_document,
        ScalarEllipticMethod::FiniteElement,
        RealizationRevision::new(21),
        &["source_scale", "diffusion", "boundary_offset"],
    );
    assert_ne!(foreign_model_program.identity(), program.identity());
    assert_replacement_rejects(
        &plan,
        &members,
        foreign_model_program
            .evaluate(&complete_point(1.0))
            .unwrap(),
    );

    let foreign_realization = program_for(
        &document,
        ScalarEllipticMethod::FiniteElement,
        RealizationRevision::new(22),
        &["source_scale", "diffusion", "boundary_offset"],
    );
    assert_replacement_rejects(
        &plan,
        &members,
        foreign_realization.evaluate(&complete_point(1.0)).unwrap(),
    );

    let fvm = program_for(
        &document,
        ScalarEllipticMethod::FiniteVolume,
        RealizationRevision::new(21),
        &["source_scale", "diffusion", "boundary_offset"],
    );
    assert_replacement_rejects(&plan, &members, fvm.evaluate(&complete_point(1.0)).unwrap());

    let foreign_parameter_order = program_for(
        &document,
        ScalarEllipticMethod::FiniteElement,
        RealizationRevision::new(21),
        &["diffusion", "source_scale", "boundary_offset"],
    );
    assert_replacement_rejects(
        &plan,
        &members,
        foreign_parameter_order
            .evaluate(&[1.0, DEFAULT_POINT[0], DEFAULT_POINT[2]])
            .unwrap(),
    );

    assert_replacement_rejects(
        &plan,
        &members,
        program.evaluate(&complete_point(1.5)).unwrap(),
    );
}

#[test]
fn evaluator_returning_another_point_cannot_substitute_a_member() {
    let (document, program) = fixture();
    let diffusion = document.parameter_ref("diffusion").unwrap().id();
    let plan = ParameterStudyPlan::new(&program, diffusion, &PERMUTED_DIFFUSION).unwrap();
    let substituted = program.evaluate(&complete_point(1.25)).unwrap();
    let mut calls = 0;
    let mut evaluator = |_retained: &DifferentiableProgram, _point: &[f64]| {
        calls += 1;
        Ok(substituted.clone())
    };
    let mut never_cancel = || false;

    let report = match plan.execute_with_evaluator(&mut evaluator, &mut never_cancel) {
        Ok(_) => panic!("a substituted point cannot produce a complete study"),
        Err(report) => report,
    };
    assert_eq!(calls, 1);
    assert!(report.completed_point_keys().is_empty());
    assert_eq!(
        report
            .failed_point_key()
            .expect("substitution fails the requested point")
            .value()
            .to_bits(),
        0.75f64.to_bits()
    );
    assert!(!report.diagnostics().is_empty());
    assert!(!report.is_cancelled());
}

#[test]
fn evaluator_returning_a_foreign_program_member_fails_before_the_next_point() {
    let (document, program) = fixture();
    let diffusion = document.parameter_ref("diffusion").unwrap().id();
    let plan = ParameterStudyPlan::new(&program, diffusion, &PERMUTED_DIFFUSION).unwrap();
    let fvm = program_for(
        &document,
        ScalarEllipticMethod::FiniteVolume,
        RealizationRevision::new(21),
        &["source_scale", "diffusion", "boundary_offset"],
    );
    let foreign = fvm.evaluate(&complete_point(0.75)).unwrap();
    let mut calls = 0;
    let mut evaluator = |_retained: &DifferentiableProgram, _point: &[f64]| {
        calls += 1;
        Ok(foreign.clone())
    };
    let mut never_cancel = || false;

    let report = match plan.execute_with_evaluator(&mut evaluator, &mut never_cancel) {
        Ok(_) => panic!("a foreign program member cannot produce a complete study"),
        Err(report) => report,
    };
    assert_eq!(calls, 1);
    assert!(report.completed_point_keys().is_empty());
    assert_eq!(
        report
            .failed_point_key()
            .expect("the foreign result fails its requested point")
            .value()
            .to_bits(),
        0.75f64.to_bits()
    );
    assert!(!report.diagnostics().is_empty());
    assert!(!report.is_cancelled());
}

fn fixture() -> (ModelDocument, DifferentiableProgram) {
    let document = ModelDocument::compile("bounded-parameter-study.eqi", SOURCE).unwrap();
    let program = program_for(
        &document,
        ScalarEllipticMethod::FiniteElement,
        RealizationRevision::new(21),
        &["source_scale", "diffusion", "boundary_offset"],
    );
    (document, program)
}

fn program_for(
    document: &ModelDocument,
    method: ScalarEllipticMethod,
    revision: RealizationRevision,
    input_names: &[&str],
) -> DifferentiableProgram {
    let environment = ScalarEllipticExecutionEnvironment::host_serial();
    let intent = ScalarEllipticIntent::new(
        revision,
        method,
        NonZeroUsize::new(12).unwrap(),
        NonZeroUsize::MIN,
    );
    let plan = document
        .preview_scalar_elliptic_run(intent, environment)
        .unwrap();
    let inputs = input_names
        .iter()
        .map(|name| document.parameter_ref(name).unwrap())
        .collect::<Vec<_>>();
    let output = document.field_ref("potential").unwrap();
    DifferentiableProgram::compile(document, plan, &inputs, &output).unwrap()
}

fn accepted_members(
    program: &DifferentiableProgram,
    values: &[f64],
) -> Vec<DifferentiableEvaluation> {
    values
        .iter()
        .map(|value| program.evaluate(&complete_point(*value)).unwrap())
        .collect()
}

fn assert_replacement_rejects(
    plan: &ParameterStudyPlan,
    members: &[DifferentiableEvaluation],
    replacement: DifferentiableEvaluation,
) {
    let mut mutated = members.to_vec();
    mutated[1] = replacement;
    assert!(CompleteParameterStudy::from_members(plan, mutated).is_err());
}

fn complete_point(diffusion: f64) -> [f64; 3] {
    [DEFAULT_POINT[0], diffusion, DEFAULT_POINT[2]]
}

fn assert_member_points(members: &[DifferentiableEvaluation], values: &[f64]) {
    assert_eq!(members.len(), values.len());
    for (member, value) in members.iter().zip(values) {
        assert_eq!(member.point().values()[1].to_bits(), value.to_bits());
    }
}

fn assert_key_values(keys: &[super::ParameterStudyPointKey], values: &[f64]) {
    assert_eq!(keys.len(), values.len());
    for (key, value) in keys.iter().zip(values) {
        assert_eq!(key.value().to_bits(), value.to_bits());
    }
}
