use eqiora_artifact::{DecoderLimits, ImplicitTimeRunManifestV1, TimeRunManifestV1};

const LEGACY_DIFFSOL_RUN_JSON: &[u8] = br#"{"schema":"eqiora.time-run-manifest/v1","encoding":"eqiora.canonical-json/v1","model_sha256":"0000000000000000000000000000000000000000000000000000000000000000","semantic_revision":1,"lowering_sha256":"1111111111111111111111111111111111111111111111111111111111111111","plan":{"method":"bdf","start_time":0.0,"initial_step":0.1,"relative_tolerance":0.01,"absolute_tolerances":[0.001],"output_times":[0.5,1.0]},"execution":{"backend":"eqiora.time.diffsol","backend_version":"0.16.0","method":"bdf","equation_class":{"kind":"explicit-ode"},"initial_condition":"provided"},"output_sha256":[]}"#;
const LEGACY_DIFFSOL_RUN_DIGEST: &str =
    "3db53e04d97d35dba3f98026ea8c8b4d876d1bb70879d307def3bdd582887dfe";

const LEGACY_REFERENCE_RUN_JSON: &[u8] = br#"{"schema":"eqiora.implicit-time-run-manifest/v1","encoding":"eqiora.canonical-json/v1","model_sha256":"0000000000000000000000000000000000000000000000000000000000000000","semantic_revision":1,"lowering_sha256":"1111111111111111111111111111111111111111111111111111111111111111","input_initial_data_sha256":"2222222222222222222222222222222222222222222222222222222222222222","accepted_initial_data_sha256":"3333333333333333333333333333333333333333333333333333333333333333","plan":{"method":"implicit-euler","start_time":0.0,"initial_step":0.1,"relative_tolerance":0.01,"absolute_tolerances":[0.001],"output_times":[0.5,1.0]},"execution":{"backend":"eqiora.time.reference-implicit-euler","backend_version":"0.1.0-reference","method":"implicit-euler","equation_class":"general-implicit-dae","initial_condition":"provided"},"output_sha256":[]}"#;
const LEGACY_REFERENCE_RUN_DIGEST: &str =
    "644eca7895a3a6d16a2561022e4f08596689aa440106ff27bdee8326dbb4b236";

#[test]
fn pre_identity_diffsol_run_bytes_and_release_remain_exact() {
    let decoded =
        TimeRunManifestV1::from_json(LEGACY_DIFFSOL_RUN_JSON, DecoderLimits::default()).unwrap();

    assert_eq!(decoded.backend_version(), "0.16.0");
    assert_eq!(decoded.canonical_json().unwrap(), LEGACY_DIFFSOL_RUN_JSON);
    assert_eq!(
        decoded.digest().unwrap().as_str(),
        LEGACY_DIFFSOL_RUN_DIGEST
    );
}

#[test]
fn pre_identity_reference_run_bytes_and_release_remain_exact() {
    let decoded =
        ImplicitTimeRunManifestV1::from_json(LEGACY_REFERENCE_RUN_JSON, DecoderLimits::default())
            .unwrap();

    assert_eq!(decoded.backend_version(), "0.1.0-reference");
    assert_eq!(decoded.canonical_json().unwrap(), LEGACY_REFERENCE_RUN_JSON);
    assert_eq!(
        decoded.digest().unwrap().as_str(),
        LEGACY_REFERENCE_RUN_DIGEST
    );
}
