use std::num::NonZeroUsize;

use eqiora_api::{
    CartesianFieldOrder, ExactModelCodec, ModelDocument, ScalarEllipticExecutionEnvironment,
    ScalarEllipticIntent, ScalarEllipticMethod, ScalarFieldLocation,
};
use eqiora_core::DimExponents;
use eqiora_realization::RealizationRevision;

const SOURCE: &str =
    include_str!("../../../verify/numerics/cartesian-poisson-fem-fvm/models/poisson.eqi");

fn preview(
    document: &ModelDocument,
    method: ScalarEllipticMethod,
) -> eqiora_api::ScalarEllipticRunPlan {
    document
        .preview_scalar_elliptic_run(
            ScalarEllipticIntent::new(
                RealizationRevision::new(7),
                method,
                NonZeroUsize::new(4).unwrap(),
                NonZeroUsize::MIN,
            ),
            ScalarEllipticExecutionEnvironment::host_serial(),
        )
        .unwrap()
}

#[test]
fn scalar_field_projection_is_fixed_at_preview_and_reaccepted_after_execution() {
    let document = ModelDocument::compile("poisson.eqi", SOURCE).unwrap();
    for (method, location, shape) in [
        (
            ScalarEllipticMethod::FiniteElement,
            ScalarFieldLocation::Vertex,
            [5, 5],
        ),
        (
            ScalarEllipticMethod::FiniteVolume,
            ScalarFieldLocation::CellCenter,
            [4, 4],
        ),
    ] {
        let plan = preview(&document, method);
        let projection = plan.field_projection().clone();
        assert_eq!(projection.preferred_alias(), Some("potential"));
        assert_eq!(projection.value_dimension(), DimExponents::DIMENSIONLESS);
        assert_eq!(projection.spatial_dimension(), 2);
        assert_eq!(projection.bounds(), [[0.0, 1.0], [0.0, 1.0]]);
        assert_eq!(projection.location(), location);
        assert_eq!(projection.logical_shape(), shape);
        assert_eq!(projection.value_count(), shape[0] * shape[1]);
        assert_eq!(projection.order(), CartesianFieldOrder::LastAxisFastest);
        assert_eq!(
            plan.portable_realization().fields()[0].field(),
            projection.field()
        );

        let result = document
            .run_scalar_elliptic_plan(plan, ScalarEllipticExecutionEnvironment::host_serial())
            .unwrap();
        assert_eq!(result.field().location(), projection.location());
        assert_eq!(result.field().logical_shape(), projection.logical_shape());
        assert_eq!(result.field().value_count(), projection.value_count());
        assert_eq!(result.field_values().len(), projection.value_count());
    }
}

#[test]
fn presentation_alias_is_optional_but_exact_semantic_identity_is_not() {
    let source_document = ModelDocument::compile("poisson.eqi", SOURCE).unwrap();
    let bytes = source_document.canonical_json().unwrap();
    let replayed = ExactModelCodec::V6.replay(&bytes).unwrap();
    assert!(replayed.aliases().is_empty());

    let plan = preview(&replayed, ScalarEllipticMethod::FiniteElement);
    assert_eq!(plan.field_projection().preferred_alias(), None);
    assert_eq!(plan.model_digest(), replayed.digest().unwrap());

    let changed = ModelDocument::compile(
        "changed-poisson.eqi",
        &SOURCE.replace("19.739208802178716", "19.0"),
    )
    .unwrap();
    assert!(
        changed
            .run_scalar_elliptic_plan(plan, ScalarEllipticExecutionEnvironment::host_serial(),)
            .is_err()
    );
}
