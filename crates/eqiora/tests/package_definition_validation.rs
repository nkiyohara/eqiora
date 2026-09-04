use std::fmt::Write as _;

use eqiora::package::{
    BundleEntryV1, BundleRoleV1, ExactVersion, NormalizedRelativePath, PackageManifestV1,
    PackagePreparationError, PackageReleaseV1, PackageSourcesV1, QualifiedName, SourceFileV1,
    prepare_package_release_v1,
};

const VALID_COMPONENTS: &str = include_str!(
    "../../../verify/packages/package-definition-validation/models/valid-components.eqi"
);
const VALID_MODEL: &str =
    include_str!("../../../verify/packages/package-definition-validation/models/valid-model.eqi");
const PERMUTED_COMPONENTS: &str = include_str!(
    "../../../verify/packages/package-definition-validation/models/valid-components-permuted.eqi"
);
const PERMUTED_MODEL: &str = include_str!(
    "../../../verify/packages/package-definition-validation/models/valid-model-permuted.eqi"
);
const EXPECTED: &[u8] =
    include_bytes!("../../../verify/packages/package-definition-validation/expected/contract.json");
const VALID_PHYSICAL_CLOSURE: &str = include_str!(
    "../../../verify/packages/package-definition-validation/models/valid/physical-closure.eqi"
);
const VALID_PHYSICAL_RECONNECTION: &str = include_str!(
    "../../../verify/packages/package-definition-validation/models/valid/closed-child-physical-reconnect.eqi"
);
const VALID_COMPONENTWISE_GRADIENT: &str = include_str!(
    "../../../verify/packages/package-definition-validation/models/valid/componentwise-gradient-residual.eqi"
);

fn sources(name: &str, files: &[(&str, &str)]) -> PackageSourcesV1 {
    let entry = files
        .iter()
        .find(|(path, _)| *path == "src/main.eqi")
        .or_else(|| (files.len() == 1).then(|| &files[0]))
        .expect("fixture entry module")
        .0
        .strip_prefix("src/")
        .unwrap()
        .strip_suffix(".eqi")
        .unwrap()
        .replace('/', ".");
    let entries = files
        .iter()
        .map(|(path, _)| {
            BundleEntryV1::new(
                NormalizedRelativePath::parse(*path).expect("fixture path"),
                BundleRoleV1::ModelSource,
            )
        })
        .collect();
    let manifest = PackageManifestV1::new(
        &entry,
        QualifiedName::parse(name).expect("fixture package name"),
        ExactVersion::parse("0.1.0").expect("fixture version"),
        vec![],
        entries,
    )
    .expect("fixture manifest");
    let files = files
        .iter()
        .map(|(path, source)| {
            SourceFileV1::new(
                NormalizedRelativePath::parse(*path).expect("fixture path"),
                BundleRoleV1::ModelSource,
                source.as_bytes().to_vec(),
            )
        })
        .collect();
    PackageSourcesV1::new(manifest, files).expect("closed fixture source inventory")
}

fn release(files: &[(&str, &str)]) -> PackageReleaseV1 {
    prepare_package_release_v1(
        sources("org.eqiora.verify.definition_validation", files),
        &[],
    )
    .expect("all definitions validate before release")
}

fn expected_case<'a>(expected: &'a serde_json::Value, name: &str) -> &'a serde_json::Value {
    expected["diagnostics"]
        .as_array()
        .expect("diagnostic oracle array")
        .iter()
        .find(|entry| entry["case"] == name)
        .unwrap_or_else(|| panic!("missing diagnostic oracle for {name}"))
}

fn assert_rejected_before_release(expected: &serde_json::Value, name: &str, source: &str) {
    let error = prepare_package_release_v1(
        sources(
            "org.eqiora.verify.invalid_definition",
            &[("src/main.eqi", source)],
        ),
        &[],
    )
    .unwrap_err();
    let PackagePreparationError::Diagnostics(diagnostics) = error else {
        panic!("{name}: expected compiler diagnostics before release, got {error}");
    };
    assert!(!diagnostics.is_empty(), "{name}: empty diagnostic set");
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic.source_span().is_some()),
        "{name}: every definition diagnostic must retain a real source span: {diagnostics:#?}"
    );
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic.graph_path().is_none()),
        "{name}: definition checking must not invent GraphPath identity: {diagnostics:#?}"
    );

    let oracle = expected_case(expected, name);
    let code = oracle["code"].as_str().expect("diagnostic code");
    let fragment = oracle["message_fragment"]
        .as_str()
        .expect("diagnostic fragment");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.code().to_string() == code && diagnostic.message().contains(fragment)
        }),
        "{name}: expected {code} containing {fragment:?}, got {diagnostics:#?}"
    );
}

fn over_depth_source() -> String {
    const COMPONENTS: usize = 64;
    let mut source = String::new();
    for index in 0..COMPONENTS {
        if index + 1 == COMPONENTS {
            writeln!(source, "component C{index:02} {{}}").expect("write fixture");
        } else {
            writeln!(
                source,
                "component C{index:02} {{ instance child: C{:02}; }}",
                index + 1
            )
            .expect("write fixture");
        }
    }
    source.push_str("model Empty {}\n");
    source
}

#[test]
fn complete_package_definition_graph_is_validated_without_occurrences() {
    let model = format!(
        "import org.eqiora.verify.definition_validation.components;\n{}",
        VALID_MODEL.replace("Wrapper(", "components.Wrapper(")
    );
    let permuted_model = format!(
        "import org.eqiora.verify.definition_validation.components;\n{}",
        PERMUTED_MODEL.replace("Wrapper(", "components.Wrapper(")
    );

    let canonical = release(&[
        ("src/components.eqi", VALID_COMPONENTS),
        ("src/main.eqi", &model),
    ]);
    let file_permuted = release(&[
        ("src/main.eqi", &model),
        ("src/components.eqi", VALID_COMPONENTS),
    ]);
    let declaration_permuted = release(&[
        ("src/components.eqi", PERMUTED_COMPONENTS),
        ("src/main.eqi", &permuted_model),
    ]);

    assert_eq!(
        canonical.canonical_json().expect("canonical release"),
        file_permuted
            .canonical_json()
            .expect("file-permuted release"),
        "source-file input order is not semantic"
    );
    assert_eq!(
        canonical.package_identity().expect("canonical identity"),
        declaration_permuted
            .package_identity()
            .expect("declaration-permuted identity"),
        "declaration order is not semantic"
    );
    assert_ne!(
        canonical.source_digest().expect("canonical source digest"),
        declaration_permuted
            .source_digest()
            .expect("permuted source digest"),
        "author-source lineage retains changed bytes"
    );
}

#[test]
fn physical_endpoint_obligations_close_compositionally() {
    prepare_package_release_v1(
        sources(
            "org.eqiora.verify.physical_definition_closure",
            &[("src/main.eqi", VALID_PHYSICAL_CLOSURE)],
        ),
        &[],
    )
    .expect("nested child endpoint obligations close without flattening occurrences");

    prepare_package_release_v1(
        sources(
            "org.eqiora.verify.physical_definition_reconnection",
            &[("src/main.eqi", VALID_PHYSICAL_RECONNECTION)],
        ),
        &[],
    )
    .expect("equivalent child physical fragments are idempotent topology claims");
}

macro_rules! negative_case {
    ($test:ident, $name:literal, $fixture:literal) => {
        #[test]
        fn $test() {
            let expected: serde_json::Value =
                serde_json::from_slice(EXPECTED).expect("expected contract");
            assert_rejected_before_release(&expected, $name, include_str!($fixture));
        }
    };
}

negative_case!(
    rejects_unused_public_relation,
    "unused-public-relation",
    "../../../verify/packages/package-definition-validation/models/invalid/unused-public-relation.eqi"
);
negative_case!(
    rejects_unused_private_relation,
    "unused-private-relation",
    "../../../verify/packages/package-definition-validation/models/invalid/unused-private-relation.eqi"
);
negative_case!(
    rejects_missing_nested_binding,
    "missing-nested-binding",
    "../../../verify/packages/package-definition-validation/models/invalid/missing-nested-binding.eqi"
);
negative_case!(
    rejects_wrong_nested_binding,
    "wrong-nested-binding",
    "../../../verify/packages/package-definition-validation/models/invalid/wrong-nested-binding.eqi"
);
negative_case!(
    rejects_required_private_parameter,
    "required-private-parameter",
    "../../../verify/packages/package-definition-validation/models/invalid/required-private-parameter.eqi"
);
negative_case!(
    rejects_invalid_second_model,
    "invalid-second-model",
    "../../../verify/packages/package-definition-validation/models/invalid/invalid-second-model.eqi"
);
negative_case!(
    rejects_unused_connector_dimension,
    "unused-connector-dimension",
    "../../../verify/packages/package-definition-validation/models/invalid/unused-connector-dimension.eqi"
);
negative_case!(
    rejects_unused_recursive_cycle,
    "unused-recursive-cycle",
    "../../../verify/packages/package-definition-validation/models/invalid/unused-recursive-cycle.eqi"
);
negative_case!(
    rejects_gradient_without_spatial_support,
    "gradient-parameter-support",
    "../../../verify/packages/package-definition-validation/models/invalid/gradient-parameter-support.eqi"
);
negative_case!(
    rejects_coordinate_outside_domain,
    "coordinate-outside-domain",
    "../../../verify/packages/package-definition-validation/models/invalid/coordinate-outside-domain.eqi"
);
negative_case!(
    rejects_coordinate_without_domain,
    "coordinate-without-domain",
    "../../../verify/packages/package-definition-validation/models/invalid/coordinate-without-domain.eqi"
);
negative_case!(
    rejects_cross_domain_fields,
    "cross-domain-fields",
    "../../../verify/packages/package-definition-validation/models/invalid/cross-domain-fields.eqi"
);
#[test]
fn admits_componentwise_shaped_relation_residual() {
    release(&[(
        "src/componentwise_gradient_residual.eqi",
        VALID_COMPONENTWISE_GRADIENT,
    )]);
}
negative_case!(
    rejects_trace_in_volume_relation,
    "trace-volume-misuse",
    "../../../verify/packages/package-definition-validation/models/invalid/trace-volume-misuse.eqi"
);
negative_case!(
    rejects_normal_in_volume_relation,
    "normal-volume-misuse",
    "../../../verify/packages/package-definition-validation/models/invalid/normal-volume-misuse.eqi"
);
negative_case!(
    rejects_boundary_of_boundary,
    "boundary-of-boundary",
    "../../../verify/packages/package-definition-validation/models/invalid/boundary-of-boundary.eqi"
);
negative_case!(
    rejects_open_private_physical_port,
    "private-physical-port-open",
    "../../../verify/packages/package-definition-validation/models/invalid/private-physical-port-open.eqi"
);
negative_case!(
    rejects_public_physical_port_with_two_owners,
    "public-physical-port-double-owner",
    "../../../verify/packages/package-definition-validation/models/invalid/public-physical-port-double-owner.eqi"
);
negative_case!(
    rejects_open_model_child_physical_port,
    "model-child-physical-port-open",
    "../../../verify/packages/package-definition-validation/models/invalid/model-child-physical-port-open.eqi"
);
negative_case!(
    rejects_one_open_model_physical_membership,
    "model-one-physical-membership-open",
    "../../../verify/packages/package-definition-validation/models/invalid/model-one-physical-membership-open.eqi"
);
negative_case!(
    keeps_repeated_instance_physical_obligations_distinct,
    "model-two-instances-one-open",
    "../../../verify/packages/package-definition-validation/models/invalid/model-two-instances-one-open.eqi"
);
#[test]
fn rejects_over_depth_definition_graph() {
    let expected: serde_json::Value = serde_json::from_slice(EXPECTED).expect("expected contract");
    assert_rejected_before_release(&expected, "over-depth-definition", &over_depth_source());
}
