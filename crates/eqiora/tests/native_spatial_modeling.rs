use std::num::NonZeroUsize;

use eqiora::DimExponents;
use eqiora::api::{
    ModelDocument, ScalarEllipticExecutionEnvironment, ScalarEllipticIntent, ScalarEllipticMethod,
};
use eqiora::language::{
    DraftBoundarySide, DraftExpression, DraftField, DraftParameter, DraftRelation,
    DraftRepresentation, DraftSpatialDomain, ModelDraft,
};
use eqiora::realization::RealizationRevision;

const SPATIAL_SOURCE: &str =
    include_str!("../../../verify/interfaces/python-native-modeling/models/poisson.eqi");

#[test]
fn native_spatial_draft_reaches_the_existing_scalar_elliptic_path() {
    let interval = DraftSpatialDomain::cartesian_box("interval", [(0.0, 1.0)]);
    let lower = DraftSpatialDomain::boundary("lower_end", &interval, 0, DraftBoundarySide::Lower);
    let upper = DraftSpatialDomain::boundary("upper_end", &interval, 0, DraftBoundarySide::Upper);
    let space = DraftRepresentation::continuum("scalar_space");
    let potential = DraftField::spatial_scalar(
        "potential",
        &interval,
        &space,
        DimExponents::DIMENSIONLESS,
        0.0,
    );
    let source_scale = DraftParameter::new(
        "source_scale",
        DimExponents {
            length: -2,
            ..DimExponents::DIMENSIONLESS
        },
        1.0,
    );
    let balance = DraftRelation::continuous_on(
        "balance",
        &interval,
        [
            -DraftExpression::divergence(DraftExpression::gradient(potential.expression()))
                - source_scale.expression(),
        ],
    );
    let lower_value = DraftRelation::continuous_on(
        "lower_value",
        &lower,
        [DraftExpression::trace(potential.expression())],
    );
    let upper_value = DraftRelation::continuous_on(
        "upper_value",
        &upper,
        [DraftExpression::trace(potential.expression())],
    );
    let draft = ModelDraft::new(
        "native_poisson",
        [
            source_scale.into(),
            upper.into(),
            interval.clone().into(),
            space.clone().into(),
            potential.into(),
            lower.clone().into(),
            balance.into(),
            upper_value.into(),
            lower_value.into(),
        ],
    )
    .unwrap();

    assert_eq!(lower.parent(), Some(&interval));
    assert_ne!(
        DraftSpatialDomain::cartesian_box("interval", [(0.0, 1.0)]),
        interval
    );
    assert_ne!(DraftRepresentation::continuum("scalar_space"), space);

    let source = ModelDocument::compile("native-poisson.eqi", SPATIAL_SOURCE).unwrap();
    let native = ModelDocument::define(&draft).unwrap();
    assert_ne!(native.digest().unwrap(), source.digest().unwrap());
    assert_eq!(
        native.structural_fingerprint().unwrap(),
        source.structural_fingerprint().unwrap()
    );

    let environment = ScalarEllipticExecutionEnvironment::host_serial();
    let plan = native
        .preview_scalar_elliptic_run(
            ScalarEllipticIntent::new(
                RealizationRevision::new(1),
                ScalarEllipticMethod::FiniteElement,
                NonZeroUsize::new(4).unwrap(),
                NonZeroUsize::MIN,
            ),
            environment,
        )
        .unwrap();
    let result = native.run_scalar_elliptic_plan(plan, environment).unwrap();
    assert_eq!(result.field().value_count(), 5);
    assert_eq!(result.field().spatial_dimension(), 1);
    assert_eq!(result.field().logical_shape(), &[5]);
    assert_eq!(result.field_values(), &[0.0, 0.09375, 0.125, 0.09375, 0.0]);
}
