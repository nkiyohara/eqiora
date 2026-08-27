use std::cell::Cell;
use std::collections::BTreeMap;
use std::num::NonZeroUsize;

use eqiora_artifact::{
    CartesianMeshCellsV1, GeometryMeshCorrespondenceEnvelopeV1, MeshProductionLineageEnvelopeV1,
    ModelEnvelope,
};
use eqiora_core::Diagnostic;
use eqiora_core::diagnostic::codes;
use eqiora_geometry::{PlanarOperationGraph, PlanarTopologyHandle};
use eqiora_numerics::{
    AuthenticatedCommonMesh, CommonScalarPlan, CommonSolvePolicy, CommonSpatialPolicy,
    resolve_common_plan,
};
use eqiora_realization::RealizationRevision;
use eqiora_solver::{LinearSolver, REFERENCE_LINEAR_SOLVER, SolverPlan};

use super::{CompleteParameterStudy, ParameterStudyPlan};
use crate::{DifferentiableEvaluation, DifferentiableProgram, ModelDocument, ScalarEllipticMethod};

const SOURCE: &str = r#"public component DifferentiatedPoisson {
  public support square: volume(ambient_dimension = 2);
  public support x_lower: boundary(parent = square);
  public support x_upper: boundary(parent = square);
  public support y_lower: boundary(parent = square);
  public support y_upper: boundary(parent = square);
  representation scalar_space = continuum;
  field potential on square as scalar_space: 1 = 0;
  public parameter diffusion: 1;
  public parameter wave_number: 1 / m;
  public parameter source_scale: 1 / m ^ 2;
  public parameter boundary_offset: 1;
  relation balance continuous on square {
    -div(diffusion * grad(potential))
      - source_scale
        * sin(wave_number * coordinate(0))
        * sin(wave_number * coordinate(1)) = 0;
  }
  relation x_lower_value continuous on x_lower { trace(potential) - boundary_offset = 0; }
  relation x_upper_value continuous on x_upper { trace(potential) - boundary_offset = 0; }
  relation y_lower_value continuous on y_lower { trace(potential) - boundary_offset = 0; }
  relation y_upper_value continuous on y_upper { trace(potential) - boundary_offset = 0; }
}
"#;
const DEFAULT_POINT: [f64; 3] = [19.739208802178716, 1.0, 0.0];
const PERMUTED_DIFFUSION: [f64; 3] = [1.25, 0.75, 1.0];
const CANONICAL_DIFFUSION: [f64; 3] = [0.75, 1.0, 1.25];

#[test]
fn registered_composition_oracle_executes_all_private_falsifiers() {
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
        "component DifferentiatedPoisson",
        "component ForeignDifferentiatedPoisson",
        1,
    );
    let foreign_document = document_from_source(&foreign_source);
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
    let document = document_from_source(SOURCE);
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
    let plan = plan_for(document, method, revision);
    let inputs = input_names
        .iter()
        .map(|name| document.parameter_ref(name).unwrap())
        .collect::<Vec<_>>();
    let output = document
        .field_ref(&plan.field().ulid().to_string())
        .unwrap();
    DifferentiableProgram::compile(plan, &inputs, &output).unwrap()
}

fn document_from_source(source: &str) -> ModelDocument {
    let graph = PlanarOperationGraph::new();
    let rectangle = graph.rectangle([0.0, 1.0], [0.0, 1.0]).unwrap();
    let edges = rectangle.boundaries();
    let geometry = graph
        .build(
            &rectangle,
            &BTreeMap::from([
                ("square".to_owned(), vec![rectangle.region().into()]),
                (
                    "x_lower".to_owned(),
                    vec![PlanarTopologyHandle::from(edges[0])],
                ),
                (
                    "x_upper".to_owned(),
                    vec![PlanarTopologyHandle::from(edges[1])],
                ),
                (
                    "y_lower".to_owned(),
                    vec![PlanarTopologyHandle::from(edges[2])],
                ),
                (
                    "y_upper".to_owned(),
                    vec![PlanarTopologyHandle::from(edges[3])],
                ),
            ]),
        )
        .unwrap();
    ModelDocument::compile_with_geometry(
        "bounded-parameter-study.eqi",
        source,
        &geometry,
        None,
        &[
            ("diffusion", 1.0),
            ("wave_number", std::f64::consts::PI),
            ("source_scale", 2.0 * std::f64::consts::PI.powi(2)),
            ("boundary_offset", 0.0),
        ],
    )
    .unwrap()
}

fn plan_for(
    document: &ModelDocument,
    method: ScalarEllipticMethod,
    revision: RealizationRevision,
) -> CommonScalarPlan {
    let geometry = document
        .geometry_authority
        .first()
        .expect("external fixture retains its exact Geometry")
        .clone();
    let cells = CartesianMeshCellsV1::new([12, 12]).unwrap();
    let (mesh, correspondence) =
        GeometryMeshCorrespondenceEnvelopeV1::from_planar_rectangle_v2_cartesian(
            &geometry,
            cells.cells(),
        )
        .unwrap();
    let production = MeshProductionLineageEnvelopeV1::from_structured_cartesian_v1_resources(
        cells,
        &geometry,
        &mesh,
        &correspondence,
    )
    .unwrap();
    let owner =
        AuthenticatedCommonMesh::structured_cartesian(geometry, mesh, correspondence, production)
            .unwrap();
    let spatial = match method {
        ScalarEllipticMethod::FiniteElement => CommonSpatialPolicy::Q1,
        ScalarEllipticMethod::FiniteVolume => CommonSpatialPolicy::CellCenteredTpfa,
    };
    let relative_tolerance = if revision.get() == 21 {
        1.0e-10
    } else {
        2.0e-10
    };
    let solver = SolverPlan::new(
        LinearSolver::ConjugateGradient,
        relative_tolerance,
        1.0e-12,
        NonZeroUsize::new(10_000).unwrap(),
    )
    .unwrap();
    let model = ModelEnvelope::from_program(document.program()).unwrap();
    resolve_common_plan(
        &model,
        owner,
        spatial,
        CommonSolvePolicy::Linear(solver),
        None,
        None,
        &REFERENCE_LINEAR_SOLVER,
    )
    .unwrap()
    .project(
        |_| panic!("scalar fixture resolved as ODE"),
        |plan| plan,
        |_| panic!("scalar fixture resolved as elasticity"),
        |_| panic!("scalar fixture resolved as Stokes"),
        |_| panic!("scalar fixture resolved as transient flow"),
        |_| panic!("scalar fixture resolved as FSI"),
    )
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
