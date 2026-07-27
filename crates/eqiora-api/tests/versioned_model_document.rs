use std::num::NonZeroUsize;

use eqiora_api::{
    ExactModelCodec, ModelDocument, ScalarEllipticExecutionEnvironment, ScalarEllipticIntent,
    ScalarEllipticMethod,
};
use eqiora_core::diagnostic::codes;
use eqiora_realization::RealizationRevision;

const DECAY_SOURCE: &str = r#"
model decay {
  field x: 1 = 1;
  parameter rate: 1 / s = 1;
  relation flow continuous {
    derivative(x) + rate * x = 0;
  }
}
"#;

const PHYSICAL_SOURCE: &str =
    include_str!("../../../verify/electrical/parallel-dc-network/models/parallel-dc.eqi");
const POISSON_2D: &str =
    include_str!("../../../verify/numerics/cartesian-poisson-fem-fvm/models/poisson.eqi");

#[test]
fn explicit_v2_compiles_and_round_trips_scalar_physical_source() {
    let document = ExactModelCodec::V2
        .compile("parallel-dc.eqi", PHYSICAL_SOURCE)
        .unwrap();
    assert_eq!(document.exact_codec(), ExactModelCodec::V2);

    let bytes = document.canonical_json().unwrap();
    let digest = document.digest().unwrap();
    assert!(String::from_utf8_lossy(&bytes).contains("eqiora.model-envelope/v2"));
    let reconstructed = ExactModelCodec::V2.replay(&bytes).unwrap();
    assert_eq!(reconstructed.exact_codec(), ExactModelCodec::V2);
    assert_eq!(reconstructed.canonical_json().unwrap(), bytes);
    assert_eq!(reconstructed.digest().unwrap(), digest);
}

#[test]
fn current_v7_round_trips_without_weakening_explicit_v5_replay() {
    let document = ModelDocument::compile("decay.eqi", DECAY_SOURCE).unwrap();
    assert_eq!(document.exact_codec(), ExactModelCodec::V7);

    let bytes = document.canonical_json().unwrap();
    let digest = document.digest().unwrap();
    assert!(String::from_utf8_lossy(&bytes).contains("eqiora.model-envelope/v7"));
    assert_eq!(
        document
            .artifact_reference()
            .unwrap()
            .artifact()
            .to_string(),
        digest
    );

    let reconstructed = ExactModelCodec::V7.replay(&bytes).unwrap();
    assert_eq!(reconstructed.exact_codec(), ExactModelCodec::V7);
    assert_eq!(reconstructed.canonical_json().unwrap(), bytes);
    assert_eq!(reconstructed.digest().unwrap(), digest);
    for historical in [
        ExactModelCodec::V1,
        ExactModelCodec::V2,
        ExactModelCodec::V3,
        ExactModelCodec::V4,
        ExactModelCodec::V5,
    ] {
        assert_eq!(
            historical.replay(&bytes).unwrap_err()[0].code(),
            codes::INVALID_ARTIFACT
        );
    }
}

#[test]
fn explicit_wire_selection_never_falls_back_or_auto_detects() {
    let v1 = ExactModelCodec::V1
        .compile("decay.eqi", DECAY_SOURCE)
        .unwrap();
    let v1_bytes = v1.canonical_json().unwrap();
    let v2 = ExactModelCodec::V2
        .compile("parallel-dc.eqi", PHYSICAL_SOURCE)
        .unwrap();
    let v2_bytes = v2.canonical_json().unwrap();

    let wrong_v1 = ExactModelCodec::V1.replay(&v2_bytes).unwrap_err();
    let wrong_v2 = ExactModelCodec::V2.replay(&v1_bytes).unwrap_err();
    assert_eq!(wrong_v1[0].code(), codes::INVALID_ARTIFACT);
    assert_eq!(wrong_v2[0].code(), codes::INVALID_ARTIFACT);

    let unsupported_transaction = ExactModelCodec::V1
        .compile("parallel-dc.eqi", PHYSICAL_SOURCE)
        .unwrap_err();
    assert_eq!(unsupported_transaction[0].code(), codes::INVALID_ARTIFACT);
}

#[test]
fn workflows_retain_the_exact_selected_model_artifact_codec() {
    let physical = ExactModelCodec::V2
        .compile("parallel-dc.eqi", PHYSICAL_SOURCE)
        .unwrap();
    let target = physical.aliases()["supply_voltage"];
    let edit = physical.preview_value_edit(target, 10.0).unwrap();
    assert_eq!(edit.exact_codec(), ExactModelCodec::V2);
    assert_eq!(
        physical
            .commit_value_edit(edit)
            .unwrap()
            .document()
            .exact_codec(),
        ExactModelCodec::V2
    );

    let spatial = ExactModelCodec::V2
        .compile("poisson.eqi", POISSON_2D)
        .unwrap();
    let intent = ScalarEllipticIntent::new(
        RealizationRevision::new(1),
        ScalarEllipticMethod::FiniteElement,
        NonZeroUsize::new(4).unwrap(),
        NonZeroUsize::MIN,
    );
    let plan = spatial
        .preview_scalar_elliptic_run(intent, ScalarEllipticExecutionEnvironment::host_serial())
        .expect("Realization linkage is independent of Model wire generation");
    plan.artifact()
        .validate_model_artifact(&spatial.artifact_reference().unwrap())
        .expect("exact v2 Model artifact reference");
}
