use std::{collections::BTreeSet, num::NonZeroUsize};

use eqiora::api::ModelDocument;
use eqiora::artifact::{
    AcceptedModelArtifact, CanonicalModelArtifact, DecoderLimits, ExecutionProvenanceV1,
    ExecutionTopologyV1, LayoutArtifacts, ModelArtifactGeneration, RealizationEnvelopeV1,
    ReplayableCanonicalModelArtifact, RunManifestV2,
};
use eqiora::compatibility::ExactModelCodec;
use eqiora::realization::{
    DefaultPolicyVersion, RealizationCapabilities, RealizationRequest, RealizationRequirements,
    SemanticRevision, VectorLayoutKind, resolve,
};
use eqiora::solver::{ReductionPolicy, ScalarType};
use eqiora::{
    DimExponents,
    language::{DraftExpression, DraftField, DraftParameter, DraftRelation, ModelDraft},
};

const POISSON: &str = include_str!("../../../verify/numerics/poisson-fem-fvm/models/poisson.eqi");

const SCALAR_PHYSICAL: &str = r#"
model scalar_physical_with_spatial_field {
  domain interval = box(0, 1);
  domain lower_end = boundary(interval, axis = 0, side = lower);
  domain upper_end = boundary(interval, axis = 0, side = upper);
  domain electrical = scalar_physical(
    across = kg * m ^ 2 / (s ^ 3 * A),
    through = A
  );
  representation scalar_space = continuum;

  field potential on interval as scalar_space: 1 = 0;
  port terminal_a: conserving on electrical;
  port terminal_b: conserving on electrical;

  relation balance continuous on interval {
    -div(grad(potential)) = 0;
  }
  relation lower_value continuous on lower_end { trace(potential) = 0; }
  relation upper_value continuous on upper_end { trace(potential) = 0; }
  relation ideal_link continuous {
    across(terminal_a) - across(terminal_b) = 0;
    through(terminal_a) + through(terminal_b) = 0;
  }

  connect conserving terminal_a, terminal_b;
}
"#;

const FIELD_BOUNDARY: &str = r#"
public connector MechanicalBoundary = field_physical(
  trace = velocity: m / s,
  flux = traction: kg / (m * s ^ 2),
  shape = spatial_vector,
  frame = spatial,
  pairing = euclidean_boundary_duality
);

public component BoundarySide {
  public support body: volume(ambient_dimension = 2);
  public support interface: boundary(parent = body);
  public port mechanical: conserving MechanicalBoundary over interface;

  relation carrier continuous on interface {
    trace(mechanical) - trace(mechanical) = 0;
    flux(mechanical) - flux(mechanical) = 0;
  }
}

model field_boundary_with_spatial_field {
  domain area = box(0, 1, 0, 1);
  domain left = boundary(area, axis = 0, side = lower);
  domain right = boundary(area, axis = 0, side = upper);
  domain bottom = boundary(area, axis = 1, side = lower);
  domain top = boundary(area, axis = 1, side = upper);
  representation scalar_space = continuum;

  field potential on area as scalar_space: 1 = 0;
  instance side_a: BoundarySide(
    support body = area,
    support interface = left
  );
  instance side_b: BoundarySide(
    support body = area,
    support interface = left
  );

  connect conserving side_a.mechanical, side_b.mechanical;

  relation balance continuous on area { -div(grad(potential)) = 0; }
  relation left_value continuous on left { trace(potential) = 0; }
  relation right_value continuous on right { trace(potential) = 0; }
  relation bottom_value continuous on bottom { trace(potential) = 0; }
  relation top_value continuous on top { trace(potential) = 0; }
}
"#;

fn resolved_realization(
    model: &ModelDocument,
    dimension: usize,
) -> (RealizationEnvelopeV1, RunManifestV2) {
    let reference = model.artifact_reference().expect("typed Model reference");
    let requirements = RealizationRequirements::new(
        NonZeroUsize::new(dimension).expect("positive dimension"),
        ScalarType::F64,
        VectorLayoutKind::Replicated,
    );
    let resolved = resolve(
        &RealizationRequest::default(
            model.program().model(),
            SemanticRevision::new(model.program().revision().0),
            DefaultPolicyVersion::V0,
        ),
        requirements,
        &RealizationCapabilities::scalar_elliptic_reference(),
    )
    .expect("capability-admitted reference plan");
    let realization =
        RealizationEnvelopeV1::from_resolved(&reference, &resolved, LayoutArtifacts::Replicated)
            .expect("version-neutral typed Realization");
    realization
        .validate_model_artifact(&reference)
        .expect("exact selected Model artifact");
    let execution = ExecutionProvenanceV1::new(
        "eqiora.host.serial",
        env!("CARGO_PKG_VERSION"),
        "eqiora.reference",
        env!("CARGO_PKG_VERSION"),
        ExecutionTopologyV1::Host {
            workers: NonZeroUsize::MIN,
        },
        ReductionPolicy::Reproducible,
    )
    .expect("closed execution provenance");
    let run = RunManifestV2::new(&realization, execution).expect("typed Run v2");
    run.validate_against(&realization)
        .expect("complete Run lineage");
    (realization, run)
}

#[test]
fn realization_and_run_lineage_accept_the_three_selected_model_generations() {
    let v1 = ExactModelCodec::V1
        .compile("poisson.eqi", POISSON)
        .expect("v1 spatial Model");
    let v2 = ExactModelCodec::V2
        .compile("scalar-physical.eqi", SCALAR_PHYSICAL)
        .expect("v2 scalar-physical Model");
    let v3 = ExactModelCodec::V3
        .compile("field-boundary.eqi", FIELD_BOUNDARY)
        .expect("v3 field-boundary Model");

    let (v1_realization, v1_run) = resolved_realization(&v1, 1);
    let (v2_realization, v2_run) = resolved_realization(&v2, 1);
    let (v3_realization, v3_run) = resolved_realization(&v3, 2);

    for (model, realization, run) in [
        (&v1, &v1_realization, &v1_run),
        (&v2, &v2_realization, &v2_run),
        (&v3, &v3_realization, &v3_run),
    ] {
        let replay = model
            .exact_codec()
            .replay(&model.canonical_json().expect("canonical Model"))
            .expect("explicit selected-wire replay");
        realization
            .validate_model_artifact(&replay.artifact_reference().unwrap())
            .expect("replayed Model identity");
        run.validate_against(realization)
            .expect("replayed Run link");
    }
}

#[test]
fn artifact_owner_dispatches_every_exact_generation_and_rejects_cross_generation_bytes() {
    let state = DraftField::new("x", DimExponents::DIMENSIONLESS, 1.0);
    let rate = DraftParameter::new(
        "rate",
        DimExponents {
            time: -1,
            ..DimExponents::DIMENSIONLESS
        },
        1.0,
    );
    let flow = DraftRelation::continuous(
        "flow",
        [DraftExpression::derivative(&state) + rate.expression() * state.expression()],
    );
    let draft = ModelDraft::new("decay", [state.into(), rate.into(), flow.into()]).unwrap();
    let model = ExactModelCodec::V1.define(&draft).expect("v1 Model");
    let (realization, _) = resolved_realization(&model, 1);
    let mut artifacts = Vec::new();
    let mut digests = BTreeSet::new();

    for &generation in ModelArtifactGeneration::ALL {
        let artifact = AcceptedModelArtifact::from_program(generation, model.program())
            .expect("owner-registered generation encodes the Model");
        assert_eq!(artifact.generation(), generation);
        let bytes = artifact.canonical_json().expect("canonical Model bytes");
        let decoded =
            AcceptedModelArtifact::from_json(generation, &bytes, DecoderLimits::default())
                .expect("explicit owner-selected generation decodes its bytes");
        assert_eq!(decoded, artifact);
        assert_eq!(
            decoded.replay_model().unwrap().program(),
            model.program(),
            "every registered generation replays the same semantic program"
        );

        for &foreign_generation in ModelArtifactGeneration::ALL {
            if foreign_generation != generation {
                assert!(
                    AcceptedModelArtifact::from_json(
                        foreign_generation,
                        &bytes,
                        DecoderLimits::default(),
                    )
                    .is_err(),
                    "wrong-generation bytes must fail closed"
                );
            }
        }

        let reference = decoded.artifact_reference().unwrap();
        assert!(digests.insert(reference.artifact().clone()));
        artifacts.push((generation, reference));
    }

    assert_eq!(artifacts.len(), ModelArtifactGeneration::ALL.len());
    assert_eq!(digests.len(), ModelArtifactGeneration::ALL.len());
    let (_, v1_reference) = &artifacts[0];
    realization
        .validate_model_artifact(v1_reference)
        .expect("the realization retains the exact v1 artifact");
    for (_, reference) in &artifacts[1..] {
        assert_eq!(v1_reference.model(), reference.model());
        assert_eq!(
            v1_reference.semantic_revision(),
            reference.semantic_revision()
        );
        assert_ne!(v1_reference.artifact(), reference.artifact());
        assert!(realization.validate_model_artifact(reference).is_err());
        assert!(v1_reference.validate_artifact(reference).is_err());
    }
}
