use eqiora::api::{
    FixedReferenceFsiSnapshotSetV1, UnstructuredP1ScalarFieldProjection2d,
    snapshot_fixed_reference_fsi_solution_v1,
};
use eqiora::artifact::{
    FieldSnapshotEnvelopeV1, JsonDecoderLimits, RunManifestV2, ValidatedFixedSpatialContextV1,
};
use eqiora::meshing::DiscreteFieldAssociation;
use eqiora_numerics::fsi::lower_fixed_reference_fsi_cartesian_2d;
use support::fixed_reference_fsi::{
    ExecutionContext, SpatialContext, direct_document, execution_context, prestrained_state,
    solve_step, spatial_context,
};

mod support;

struct AcceptedField {
    spatial: SpatialContext,
    execution: ExecutionContext,
    snapshots: FixedReferenceFsiSnapshotSetV1,
    pressure: eqiora::Id<eqiora::kinds::Field>,
    displacement: eqiora::Id<eqiora::kinds::Field>,
}

impl AcceptedField {
    fn context(&self) -> ValidatedFixedSpatialContextV1<'_> {
        ValidatedFixedSpatialContextV1::new(
            &self.spatial.model,
            &self.execution.realization,
            &self.spatial.geometry,
            &self.spatial.correspondence,
            &self.spatial.mesh_artifact,
        )
        .expect("accepted fixed-spatial lineage")
    }

    fn snapshot(&self, field: eqiora::Id<eqiora::kinds::Field>) -> &FieldSnapshotEnvelopeV1 {
        self.snapshots
            .snapshots()
            .iter()
            .find(|snapshot| snapshot.field() == field)
            .expect("accepted snapshot")
    }
}

#[test]
fn accepted_fsi_pressure_projects_one_exact_bounded_triangle_view() {
    let accepted = accepted_field();
    let context = accepted.context();
    let snapshot = accepted.snapshot(accepted.pressure);
    let block = &accepted.snapshots.blocks(accepted.pressure).unwrap()[0];

    let projection = UnstructuredP1ScalarFieldProjection2d::from_fixed_snapshot(
        &context,
        &accepted.execution.run,
        snapshot,
        block,
    )
    .expect("accepted scalar P1 snapshot projects");

    assert_eq!(
        projection.model_artifact(),
        context.model_reference().artifact()
    );
    assert_eq!(
        projection.semantic_revision(),
        context.model_reference().semantic_revision().get()
    );
    assert_eq!(
        projection.realization_artifact(),
        &context.realization().digest().unwrap()
    );
    assert_eq!(
        projection.run_artifact(),
        &accepted.execution.run.digest().unwrap()
    );
    assert_eq!(projection.snapshot_artifact(), &snapshot.digest().unwrap());
    assert_eq!(
        projection.mesh_artifact(),
        &context.mesh().digest().unwrap()
    );
    assert_eq!(projection.field(), accepted.pressure);
    assert_eq!(projection.support_domain(), snapshot.support_domain());
    assert_eq!(projection.value_dimension(), snapshot.dimension());
    assert_eq!(
        projection.vertices_m().len(),
        context.mesh().mesh().vertices().len()
    );
    assert_eq!(
        projection.triangles().len(),
        context.mesh().mesh().cells().len()
    );
    assert_eq!(projection.values(), block.values());
    assert_eq!(
        projection.triangles(),
        context
            .mesh()
            .mesh()
            .cells()
            .iter()
            .map(|cell| [
                u32::try_from(cell[0]).unwrap(),
                u32::try_from(cell[1]).unwrap(),
                u32::try_from(cell[2]).unwrap(),
            ])
            .collect::<Vec<_>>()
    );
    assert_eq!(
        projection.minimum(),
        block.values().iter().copied().fold(f64::INFINITY, f64::min)
    );
    assert_eq!(
        projection.maximum(),
        block
            .values()
            .iter()
            .copied()
            .fold(f64::NEG_INFINITY, f64::max)
    );
}

#[test]
fn foreign_lineage_and_non_scalar_p1_data_fail_before_projection() {
    let accepted = accepted_field();
    let context = accepted.context();
    let pressure_snapshot = accepted.snapshot(accepted.pressure);
    let pressure_block = &accepted.snapshots.blocks(accepted.pressure).unwrap()[0];

    let mut foreign_run: serde_json::Value =
        serde_json::from_slice(&accepted.execution.run.canonical_json().unwrap()).unwrap();
    foreign_run["semantic_revision"] = (accepted.execution.run.semantic_revision() + 1).into();
    let foreign_run = RunManifestV2::from_json(
        &serde_json::to_vec(&foreign_run).unwrap(),
        JsonDecoderLimits::default(),
    )
    .expect("foreign lineage is locally valid before exact replay");
    assert!(
        UnstructuredP1ScalarFieldProjection2d::from_fixed_snapshot(
            &context,
            &foreign_run,
            pressure_snapshot,
            pressure_block,
        )
        .is_err(),
        "a foreign semantic revision must not materialize projection state"
    );

    let displacement_snapshot = accepted.snapshot(accepted.displacement);
    let displacement_block = accepted
        .snapshots
        .blocks(accepted.displacement)
        .unwrap()
        .iter()
        .find(|block| block.association() == DiscreteFieldAssociation::Vertex)
        .unwrap();
    assert!(
        UnstructuredP1ScalarFieldProjection2d::from_fixed_snapshot(
            &context,
            &accepted.execution.run,
            displacement_snapshot,
            displacement_block,
        )
        .is_err(),
        "a vector P1 snapshot must not enter the scalar projection"
    );
    assert!(
        UnstructuredP1ScalarFieldProjection2d::from_fixed_snapshot(
            &context,
            &accepted.execution.run,
            pressure_snapshot,
            displacement_block,
        )
        .is_err(),
        "a foreign coefficient block must not enter the pressure projection"
    );
}

fn accepted_field() -> AcceptedField {
    let document = direct_document();
    let canonical = lower_fixed_reference_fsi_cartesian_2d(document.program())
        .expect("fixed-reference FSI meaning");
    let spatial = spatial_context(document.program(), &canonical);
    let execution = execution_context(document.program(), &canonical, &spatial);
    let solution = solve_step(
        &canonical,
        &spatial,
        &execution,
        &prestrained_state(&spatial),
    )
    .solution;
    let context = ValidatedFixedSpatialContextV1::new(
        &spatial.model,
        &execution.realization,
        &spatial.geometry,
        &spatial.correspondence,
        &spatial.mesh_artifact,
    )
    .expect("fixed-spatial lineage");
    let snapshots = snapshot_fixed_reference_fsi_solution_v1(&context, &solution)
        .expect("accepted FSI snapshots");
    AcceptedField {
        spatial,
        execution,
        snapshots,
        pressure: solution.fields().fluid_pressure(),
        displacement: solution.fields().solid_displacement(),
    }
}
