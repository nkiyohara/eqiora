use std::cell::Cell;
use std::collections::BTreeMap;
use std::num::NonZeroUsize;
use std::sync::Arc;

use eqiora_artifact::{
    CartesianMeshCellsV2, GeometryMeshCorrespondenceEnvelopeV1, MeshProductionLineageEnvelopeV1,
    ModelEnvelope,
};
use eqiora_core::Diagnostic;
use eqiora_core::diagnostic::codes;
use eqiora_geometry::{GeometryGraph, PlanarTopologyHandle};
use eqiora_numerics::{
    AuthenticatedCommonMesh, CommonScalarPlan, CommonSolvePolicy, CommonSpatialPolicy,
    resolve_common_plan,
};
use eqiora_realization::RealizationRevision;
use eqiora_solver::REFERENCE_LINEAR_SOLVER;

use super::{CompleteEvaluationMap, EvaluationMapOccurrence, EvaluationMapPlan};
use crate::{DifferentiableProgram, ModelDocument};

const SOURCE: &str = r#"public component DifferentiatedPoisson(support square: volume(ambient_dimension = 2), support x_lower: boundary(parent = square), support x_upper: boundary(parent = square), support y_lower: boundary(parent = square), support y_upper: boundary(parent = square)) {
  variable potential: 1 on square;
  public parameter diffusion: 1;
  public parameter wave_number: 1 / m;
  public parameter source_scale: 1 / m ^ 2;
  public parameter boundary_offset: 1;
  relation balance on square {
    -div(diffusion * grad(potential))
      - source_scale
        * math.sin(wave_number * coordinate(0))
        * math.sin(wave_number * coordinate(1)) = 0;
  }
  relation x_lower_value on x_lower { trace(potential) - boundary_offset = 0; }
  relation x_upper_value on x_upper { trace(potential) - boundary_offset = 0; }
  relation y_lower_value on y_lower { trace(potential) - boundary_offset = 0; }
  relation y_upper_value on y_upper { trace(potential) - boundary_offset = 0; }
}
"#;

const P1: [f64; 3] = [2.0, 0.75, 0.5];
const P2: [f64; 3] = [3.0, 1.25, -0.25];
const BYTE_LIMIT: usize = 1 << 30;

#[test]
fn registered_composition_oracle_executes_all_private_falsifiers() {
    complete_construction_binds_positions_and_rejects_foreign_members();
    execution_records_each_ordered_occurrence_and_stops_on_original_failure();
    foreign_evaluator_cannot_accept_an_unrequested_member();
    cancellation_marks_exact_occurrences_and_completion_wins();
    structural_admission_and_resource_limits_precede_evaluation();
}

#[test]
fn complete_construction_binds_positions_and_rejects_foreign_members() {
    let (document, program) = fixture();
    let program = Arc::new(program);
    let plan = EvaluationMapPlan::new(program.clone(), &[&P2, &P1, &P2], BYTE_LIMIT).unwrap();
    let members = [&P2, &P1, &P2]
        .map(|point| program.evaluate(point).unwrap())
        .to_vec();
    assert!(CompleteEvaluationMap::from_members(&plan, members.clone()).is_ok());
    let mut missing = members.clone();
    missing.pop();
    assert!(CompleteEvaluationMap::from_members(&plan, missing).is_err());
    let mut inserted = members.clone();
    inserted.push(members[0].clone());
    assert!(CompleteEvaluationMap::from_members(&plan, inserted).is_err());
    let mut reordered = members.clone();
    reordered.swap(0, 1);
    assert!(CompleteEvaluationMap::from_members(&plan, reordered).is_err());
    let foreign_document = document_from_source(
        &SOURCE.replace("component DifferentiatedPoisson", "component ForeignModel"),
    );
    let inputs = ["source_scale", "diffusion", "boundary_offset"];
    let foreign_programs = [
        program_for(
            &foreign_document,
            CommonSpatialPolicy::Q1,
            RealizationRevision::new(21),
            &inputs,
        ),
        program_for(
            &document,
            CommonSpatialPolicy::Q1,
            RealizationRevision::new(22),
            &inputs,
        ),
        program_for(
            &document,
            CommonSpatialPolicy::CellCenteredTpfa,
            RealizationRevision::new(21),
            &inputs,
        ),
        program_for(
            &document,
            CommonSpatialPolicy::Q1,
            RealizationRevision::new(21),
            &["diffusion", "source_scale", "boundary_offset"],
        ),
    ];
    for foreign in foreign_programs {
        let mut mutated = members.clone();
        mutated[1] = foreign.evaluate(&P1).unwrap();
        assert!(CompleteEvaluationMap::from_members(&plan, mutated).is_err());
    }
    let mut substituted = members.clone();
    substituted[1] = members[0].clone();
    assert!(CompleteEvaluationMap::from_members(&plan, substituted).is_err());
    // Declared repeated points are two valid positions, not a deduplication error.
    let complete = CompleteEvaluationMap::from_members(&plan, members).unwrap();
    assert_eq!(complete.members().len(), 3);
    assert!(complete.evaluation(3).is_none());
}

#[test]
fn execution_records_each_ordered_occurrence_and_stops_on_original_failure() {
    let (_, program) = fixture();
    let program = Arc::new(program);
    let plan = EvaluationMapPlan::new(program, &[&P2, &P1, &P2], BYTE_LIMIT).unwrap();
    let mut calls = Vec::new();
    let depth = Cell::new(0);
    let result = plan
        .execute_with_evaluator(
            &mut |program, point| {
                assert_eq!(depth.replace(1), 0);
                calls.push(point.to_vec());
                let result = program.evaluate(point);
                depth.set(0);
                result
            },
            &mut || false,
        )
        .unwrap();
    assert_eq!(calls, [P2.to_vec(), P1.to_vec(), P2.to_vec()]);
    assert_eq!(result.members().len(), 3);

    let original = vec![Diagnostic::error(
        codes::INVALID_LINEARIZATION,
        "original member failure",
    )];
    let mut calls = 0;
    let report = plan
        .execute_with_evaluator(
            &mut |program, point| {
                calls += 1;
                if calls == 2 {
                    Err(original.clone())
                } else {
                    program.evaluate(point)
                }
            },
            &mut || false,
        )
        .unwrap_err();
    assert_eq!(calls, 2);
    assert_eq!(report.stopped_index(), 1);
    assert_eq!(report.diagnostics(), original);
    assert_eq!(report.accepted_members()[0].point().values(), P2);
    assert!(matches!(
        report.occurrence(0),
        Some(EvaluationMapOccurrence::Accepted(_))
    ));
    assert!(
        matches!(report.occurrence(1), Some(EvaluationMapOccurrence::Failed(d)) if d == original)
    );
    assert!(matches!(
        report.occurrence(2),
        Some(EvaluationMapOccurrence::NotStarted)
    ));
    assert!(report.occurrence(3).is_none());
    assert!(
        CompleteEvaluationMap::from_members(&plan, report.accepted_members().to_vec()).is_err()
    );
}

#[test]
fn foreign_evaluator_cannot_accept_an_unrequested_member() {
    let (document, program) = fixture();
    let program = Arc::new(program);
    let plan = EvaluationMapPlan::new(program.clone(), &[&P2, &P1, &P2], BYTE_LIMIT).unwrap();
    let foreign = program_for(
        &document,
        CommonSpatialPolicy::CellCenteredTpfa,
        RealizationRevision::new(21),
        &["source_scale", "diffusion", "boundary_offset"],
    );
    for substitute in [
        program.evaluate(&P1).unwrap(),
        foreign.evaluate(&P2).unwrap(),
    ] {
        let mut calls = 0;
        let report = plan
            .execute_with_evaluator(
                &mut |_, _| {
                    calls += 1;
                    Ok(substitute.clone())
                },
                &mut || false,
            )
            .unwrap_err();
        assert_eq!(calls, 1);
        assert_eq!(report.stopped_index(), 0);
        assert_eq!(report.diagnostics()[0].code(), codes::INVALID_LINEARIZATION);
        assert!(report.accepted_members().is_empty());
    }
}

#[test]
fn cancellation_marks_exact_occurrences_and_completion_wins() {
    let (_, program) = fixture();
    let program = Arc::new(program);
    let plan = EvaluationMapPlan::new(program.clone(), &[&P2, &P1, &P2], BYTE_LIMIT).unwrap();
    for stop_after in [0, 1, 2, 3] {
        let calls = Cell::new(0);
        let polls = Cell::new(0);
        let in_evaluation = Cell::new(false);
        let result = plan.execute_with_evaluator(
            &mut |program, point| {
                in_evaluation.set(true);
                calls.set(calls.get() + 1);
                let result = program.evaluate(point);
                in_evaluation.set(false);
                result
            },
            &mut || {
                assert!(!in_evaluation.get());
                polls.set(polls.get() + 1);
                calls.get() >= stop_after
            },
        );
        assert_eq!(calls.get(), stop_after);
        if stop_after == 3 {
            assert_eq!(result.unwrap().members().len(), 3);
            assert_eq!(polls.get(), 3);
        } else {
            let report = result.unwrap_err();
            assert!(report.is_cancelled());
            assert_eq!(report.stopped_index(), stop_after);
            assert_eq!(report.accepted_members().len(), stop_after);
            assert_eq!(report.diagnostics()[0].code(), codes::EXECUTION_CANCELLED);
            assert!(matches!(
                report.occurrence(stop_after),
                Some(EvaluationMapOccurrence::Cancelled)
            ));
            if stop_after < 2 {
                assert!(matches!(
                    report.occurrence(stop_after + 1),
                    Some(EvaluationMapOccurrence::NotStarted)
                ));
            }
        }
    }
    let empty = EvaluationMapPlan::new(program, &[], 0).unwrap();
    assert!(
        empty
            .execute_with_cancellation(|| panic!("empty map does not poll"))
            .is_ok()
    );
    let cancel = Cell::new(false);
    let failed = plan
        .execute_with_evaluator(
            &mut |_, _| {
                cancel.set(true);
                Err(vec![Diagnostic::error(
                    codes::INVALID_LINEARIZATION,
                    "failed during evaluation",
                )])
            },
            &mut || cancel.get(),
        )
        .unwrap_err();
    assert!(
        !failed.is_cancelled(),
        "a failed action wins over a cancellation raised inside it"
    );
}

#[test]
fn structural_admission_and_resource_limits_precede_evaluation() {
    let (_, program) = fixture();
    let program = Arc::new(program);
    for invalid in [
        &[1.0, 2.0][..],
        &[1.0, f64::NAN, 0.0],
        &[1.0, f64::INFINITY, 0.0],
    ] {
        assert!(EvaluationMapPlan::new(program.clone(), &[&P2, invalid], BYTE_LIMIT).is_err());
    }
    let plan = EvaluationMapPlan::new(program.clone(), &[&P2, &P1], BYTE_LIMIT).unwrap();
    let required = plan.estimated_retained_bytes();
    assert!(required > 2 * program.identity().output_dimension() * size_of::<f64>());
    assert!(EvaluationMapPlan::new(program.clone(), &[&P2, &P1], required).is_ok());
    assert!(EvaluationMapPlan::new(program.clone(), &[&P2, &P1], required - 1).is_err());
    assert!(super::retained_bytes(2, usize::MAX).is_err());
    let more_than_historical_bound = vec![P2.as_slice(); 65];
    assert!(
        EvaluationMapPlan::new(program.clone(), &more_than_historical_bound, BYTE_LIMIT).is_ok()
    );
    let zeros = EvaluationMapPlan::new(program, &[&[1.0, 1.0, -0.0], &[1.0, 1.0, 0.0]], BYTE_LIMIT)
        .unwrap();
    assert_ne!(
        zeros.points()[0].values()[2].to_bits(),
        zeros.points()[1].values()[2].to_bits()
    );
}

pub(super) fn fixture() -> (ModelDocument, DifferentiableProgram) {
    let document = document_from_source(SOURCE);
    let program = program_for(
        &document,
        CommonSpatialPolicy::Q1,
        RealizationRevision::new(21),
        &["source_scale", "diffusion", "boundary_offset"],
    );
    (document, program)
}

pub(super) fn program_for(
    document: &ModelDocument,
    spatial: CommonSpatialPolicy,
    revision: RealizationRevision,
    input_names: &[&str],
) -> DifferentiableProgram {
    let plan = plan_for(document, spatial, revision);
    let inputs = input_names
        .iter()
        .map(|name| document.parameter_ref(name).unwrap())
        .collect::<Vec<_>>();
    let output = document
        .field_ref(&plan.fields().next().unwrap().0.ulid().to_string())
        .unwrap();
    DifferentiableProgram::compile(plan, &inputs, &output).unwrap()
}

fn document_from_source(source: &str) -> ModelDocument {
    let graph = GeometryGraph::new();
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
            (
                "diffusion",
                eqiora_lang::DraftExpression::constant(1.0).source_ast(),
            ),
            (
                "wave_number",
                eqiora_lang::DraftExpression::constant(std::f64::consts::PI).source_ast(),
            ),
            (
                "source_scale",
                eqiora_lang::DraftExpression::constant(2.0 * std::f64::consts::PI.powi(2))
                    .source_ast(),
            ),
            (
                "boundary_offset",
                eqiora_lang::DraftExpression::constant(0.0).source_ast(),
            ),
        ],
    )
    .unwrap()
}

fn plan_for(
    document: &ModelDocument,
    spatial: CommonSpatialPolicy,
    revision: RealizationRevision,
) -> CommonScalarPlan {
    let geometry = document
        .geometry_authority
        .first()
        .expect("external fixture retains its exact Geometry")
        .clone();
    let cells = CartesianMeshCellsV2::new([12, 12]).unwrap();
    let (mesh, correspondence) =
        GeometryMeshCorrespondenceEnvelopeV1::from_planar_rectangle_v2_cartesian(
            &geometry,
            cells.cells().try_into().unwrap(),
        )
        .unwrap();
    let production = MeshProductionLineageEnvelopeV1::from_structured_cartesian_v2_resources(
        &cells,
        &geometry,
        &mesh,
        &correspondence,
    )
    .unwrap();
    let owner =
        AuthenticatedCommonMesh::structured_cartesian(geometry, mesh, correspondence, production)
            .unwrap();
    let relative_tolerance = if revision.get() == 21 {
        1.0e-10
    } else {
        2.0e-10
    };
    let solver = CommonSolvePolicy::linear(
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
        solver,
        None,
        None,
        &REFERENCE_LINEAR_SOLVER,
        None,
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
