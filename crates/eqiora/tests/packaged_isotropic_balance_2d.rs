use std::collections::{BTreeMap, BTreeSet};
use std::f64::consts::PI;
use std::num::{NonZeroU16, NonZeroUsize};

use eqiora::api::ModelDocument;
use eqiora::artifact::{
    ExecutionProvenanceV1, ExecutionTopologyV1, LayoutArtifacts, ModelEnvelopeV4,
    RealizationEnvelopeV1, RunManifestV2,
};
use eqiora::compatibility::ExactModelCodec;
use eqiora::language::{ComponentItem, DomainSyntax, Item};
use eqiora::meshing::QuadratureRule;
use eqiora::numerics::{
    lower_isotropic_elasticity_cartesian_2d, solve_resolved_isotropic_elasticity_cartesian_2d,
};
use eqiora::package::{
    AuthorManifestV1, AuthorPackageSourcesV1, BundleEntryV1, BundleRoleV1, DependencyRequirementV1,
    ExactVersion, InMemoryPackageStore, NormalizedRelativePath, PackageCompilationRecordV1,
    PackageExecutionBindingV1, PackageReleaseV1, PackagedModelDocument, QualifiedName,
    ResolutionRecordV1, SourceFileV1, prepare_package_release_v1,
};
use eqiora::realization::{
    Discretization, DiscretizationMethod, ExecutionSchedule, MeshPolicy, QuadraturePolicy,
    RealizationCapabilities, RealizationPlan, RealizationRequest, RealizationRequirements,
    RealizationRevision, SemanticRevision, Space, Target, VectorLayoutKind, resolve,
};
use eqiora::sem::KernelProgram;
use eqiora::solver::{
    LinearSolver, REFERENCE_LINEAR_SOLVER, ReductionPolicy, ScalarType, SolverPlan,
};

#[path = "support/embedded_package.rs"]
mod embedded_package;

const VERIFIED_COMPONENT_V0_1: &str = include_str!(
    "../../../verify/solid/packaged-isotropic-balance-2d/package-v0.1.0/src/linear_elasticity.eqi"
);
const VERIFIED_MANIFEST_V0_1: &[u8] = include_bytes!(
    "../../../verify/solid/packaged-isotropic-balance-2d/package-v0.1.0/package.json"
);
const VERIFIED_README_V0_1: &[u8] =
    include_bytes!("../../../verify/solid/packaged-isotropic-balance-2d/package-v0.1.0/README.md");
const SYNTHETIC_COMPONENT: &str =
    include_str!("../../../verify/solid/packaged-isotropic-balance-2d/models/components.eqi");
const SYNTHETIC_COMPONENT_PERMUTED: &str = include_str!(
    "../../../verify/solid/packaged-isotropic-balance-2d/models/components-permuted.eqi"
);
const PACKAGED_MANUFACTURED: &str =
    include_str!("../../../verify/solid/packaged-isotropic-balance-2d/models/manufactured.eqi");
const PACKAGED_MANUFACTURED_PERMUTED: &str = include_str!(
    "../../../verify/solid/packaged-isotropic-balance-2d/models/manufactured-permuted.eqi"
);
const PACKAGED_LINEAR_LOAD: &str =
    include_str!("../../../verify/solid/packaged-isotropic-balance-2d/models/linear-load.eqi");
const EXPLICIT_MANUFACTURED: &str =
    include_str!("../../../verify/solid/isotropic-elasticity-2d/models/manufactured.eqi");
const EXPLICIT_LINEAR_LOAD: &str =
    include_str!("../../../verify/solid/isotropic-elasticity-2d/models/linear-load.eqi");
const PUBLIC_PACKAGE: &str = "Eqiora.Solid.LinearElasticity";
const ROOT_PACKAGE: &str = "org.eqiora.verify.packaged_isotropic_balance_2d";
const VERSION: &str = "0.1.0";
const COMPONENTS: usize = 2;

const PACKAGED_TO_EXPLICIT: [(&str, &str); 18] = [
    ("body", "body"),
    ("x_lower", "x_lower"),
    ("x_upper", "x_upper"),
    ("y_lower", "y_lower"),
    ("y_upper", "y_upper"),
    ("space", "space"),
    ("displacement", "displacement"),
    ("load_potential", "load_potential"),
    ("mu", "mu"),
    ("lambda", "lambda"),
    ("wave_number", "wave_number"),
    ("load_scale", "load_scale"),
    ("load", "load"),
    ("balance_law.balance", "balance"),
    ("x_lower_value", "x_lower_value"),
    ("x_upper_value", "x_upper_value"),
    ("y_lower_value", "y_lower_value"),
    ("y_upper_value", "y_upper_value"),
];

fn verified_component_v0_1_sources() -> AuthorPackageSourcesV1 {
    embedded_package::sources(
        VERIFIED_MANIFEST_V0_1,
        &[
            (
                "README.md",
                BundleRoleV1::Documentation,
                VERIFIED_README_V0_1,
            ),
            (
                "src/linear_elasticity.eqi",
                BundleRoleV1::ModelSource,
                VERIFIED_COMPONENT_V0_1.as_bytes(),
            ),
        ],
    )
}

fn verified_component_v0_1_release() -> PackageReleaseV1 {
    prepare_package_release_v1(verified_component_v0_1_sources(), &[])
        .expect("prepare the checked-in reusable solid package")
}

fn synthetic_component_sources(
    name: &str,
    source: &str,
    reverse_files: bool,
) -> AuthorPackageSourcesV1 {
    let model_path =
        NormalizedRelativePath::parse("src/linear_elasticity.eqi").expect("component source path");
    let readme_path = NormalizedRelativePath::parse("README.md").expect("README path");
    let manifest = AuthorManifestV1::new(
        QualifiedName::parse(name).expect("component package name"),
        ExactVersion::parse(VERSION).expect("component package version"),
        Vec::new(),
        vec![
            BundleEntryV1::new(readme_path.clone(), BundleRoleV1::Documentation),
            BundleEntryV1::new(model_path.clone(), BundleRoleV1::ModelSource),
        ],
    )
    .expect("component manifest");
    let mut files = vec![
        SourceFileV1::new(
            readme_path,
            BundleRoleV1::Documentation,
            VERIFIED_README_V0_1.to_vec(),
        ),
        SourceFileV1::new(
            model_path,
            BundleRoleV1::ModelSource,
            source.as_bytes().to_vec(),
        ),
    ];
    if reverse_files {
        files.reverse();
    }
    AuthorPackageSourcesV1::new(manifest, files).expect("closed component author sources")
}

fn synthetic_component_release(name: &str, source: &str, reverse_files: bool) -> PackageReleaseV1 {
    prepare_package_release_v1(
        synthetic_component_sources(name, source, reverse_files),
        &[],
    )
    .expect("prepare synthetic provider falsifier")
}

fn root_sources(
    component: &PackageReleaseV1,
    alias: &str,
    source: &str,
    reverse_files: bool,
) -> AuthorPackageSourcesV1 {
    let model_path = NormalizedRelativePath::parse("src/main.eqi").expect("root source path");
    let readme_path = NormalizedRelativePath::parse("README.md").expect("root README path");
    let requirement = DependencyRequirementV1::new(
        QualifiedName::parse(alias).expect("dependency alias"),
        component
            .package_identity()
            .expect("component package identity"),
    )
    .expect("exact component dependency");
    let manifest = AuthorManifestV1::new(
        QualifiedName::parse(ROOT_PACKAGE).expect("root package name"),
        ExactVersion::parse(VERSION).expect("root package version"),
        vec![requirement],
        vec![
            BundleEntryV1::new(readme_path.clone(), BundleRoleV1::Documentation),
            BundleEntryV1::new(model_path.clone(), BundleRoleV1::ModelSource),
        ],
    )
    .expect("root manifest");
    let source = source.replace("solid.", &format!("{alias}."));
    let mut files = vec![
        SourceFileV1::new(
            readme_path,
            BundleRoleV1::Documentation,
            b"Packaged isotropic-balance verification root.\n".to_vec(),
        ),
        SourceFileV1::new(model_path, BundleRoleV1::ModelSource, source.into_bytes()),
    ];
    if reverse_files {
        files.reverse();
    }
    AuthorPackageSourcesV1::new(manifest, files).expect("closed root author sources")
}

fn root_release(
    component: &PackageReleaseV1,
    alias: &str,
    source: &str,
    reverse_files: bool,
) -> PackageReleaseV1 {
    prepare_package_release_v1(
        root_sources(component, alias, source, reverse_files),
        std::slice::from_ref(component),
    )
    .expect("prepare exact packaged elasticity root")
}

fn compile_locked(
    component: &PackageReleaseV1,
    root: &PackageReleaseV1,
) -> (PackagedModelDocument, ResolutionRecordV1) {
    let resolution = ResolutionRecordV1::from_exact_releases(root, std::slice::from_ref(component))
        .expect("exact two-package resolution");
    let mut store = InMemoryPackageStore::default();
    store.insert(component).expect("insert component release");
    store.insert(root).expect("insert root release");
    let packaged =
        PackagedModelDocument::compile_locked(&store, &resolution, "Main", ExactModelCodec::V4)
            .expect("compile exact packaged elasticity Model");
    packaged
        .compilation()
        .validate_against(&resolution)
        .expect("package compilation matches exact resolution");
    (packaged, resolution)
}

fn baseline_package() -> (PackageReleaseV1, PackageReleaseV1) {
    let component = verified_component_v0_1_release();
    let root = root_release(&component, "solid", PACKAGED_MANUFACTURED, false);
    (component, root)
}

fn assert_verified_component_v0_1_boundary() {
    let document = eqiora::language::parse("linear_elasticity.eqi", VERIFIED_COMPONENT_V0_1)
        .into_document()
        .expect("checked-in component source parses");
    assert!(document.connectors().is_empty());
    assert!(document.models().is_empty());
    assert_eq!(document.components().len(), 1);
    let component = &document.components()[0];
    assert_eq!(component.name(), "IsotropicBalanceWithPotential2d");
    assert_eq!(component.items().len(), 6);
    assert_eq!(
        component
            .items()
            .iter()
            .filter(|item| matches!(item, ComponentItem::Support(_)))
            .count(),
        1
    );
    assert_eq!(
        component
            .items()
            .iter()
            .filter(|item| matches!(item, ComponentItem::FieldSlot(_)))
            .count(),
        2
    );
    assert_eq!(
        component
            .items()
            .iter()
            .filter(|item| matches!(item, ComponentItem::Parameter(_)))
            .count(),
        2
    );
    assert_eq!(
        component
            .items()
            .iter()
            .filter(|item| matches!(item, ComponentItem::Relation(_)))
            .count(),
        1
    );

    let synthetic = eqiora::language::parse("synthetic.eqi", SYNTHETIC_COMPONENT)
        .into_document()
        .expect("synthetic provider source parses");
    assert_eq!(
        eqiora::language::format(&document),
        eqiora::language::format(&synthetic),
        "the renamed provider falsifier repeats the public Component exactly"
    );
}

fn assert_root_boundary() {
    let document = eqiora::language::parse("manufactured.eqi", PACKAGED_MANUFACTURED)
        .into_document()
        .expect("root package source parses");
    assert!(document.connectors().is_empty());
    assert!(document.components().is_empty());
    assert_eq!(document.models().len(), 1);
    let items = document.models()[0].items();
    assert_eq!(
        items
            .iter()
            .filter(|item| matches!(item, Item::Domain(_)))
            .count(),
        5
    );
    assert_eq!(
        items
            .iter()
            .filter(|item| {
                matches!(item, Item::Domain(domain) if matches!(domain.syntax(), DomainSyntax::Boundary { .. }))
            })
            .count(),
        4
    );
    assert_eq!(
        items
            .iter()
            .filter(|item| matches!(item, Item::Representation(_)))
            .count(),
        1
    );
    assert_eq!(
        items
            .iter()
            .filter(|item| matches!(item, Item::Field(_)))
            .count(),
        2
    );
    assert_eq!(
        items
            .iter()
            .filter(|item| matches!(item, Item::Parameter(_)))
            .count(),
        4,
        "the root owns every tunable Parameter identity"
    );
    assert_eq!(
        items
            .iter()
            .filter(|item| matches!(item, Item::Relation(_)))
            .count(),
        5,
        "the root owns one load definition and four boundary Relations"
    );
    assert_eq!(
        items
            .iter()
            .filter(|item| matches!(item, Item::Instance(_)))
            .count(),
        1
    );
}

fn explicit_document() -> ModelDocument {
    ExactModelCodec::V4
        .compile("explicit-manufactured.eqi", EXPLICIT_MANUFACTURED)
        .expect("existing explicit-flat elasticity Model")
}

fn explicit_linear_load_document() -> ModelDocument {
    ExactModelCodec::V4
        .compile("explicit-linear-load.eqi", EXPLICIT_LINEAR_LOAD)
        .expect("existing explicit-flat nonzero-load Model")
}

fn activations_by_relation(model: &serde_json::Value) -> BTreeMap<String, String> {
    let mut activations = BTreeMap::new();
    for edge in model["edges"].as_array().expect("edge array") {
        if edge["kind"].as_str() != Some("activates") {
            continue;
        }
        let activation = id_ulid(&edge["from"]);
        let relation = id_ulid(&edge["to"]);
        assert!(activations.insert(relation, activation).is_none());
    }
    activations
}

fn collect_model_ulids(model: &serde_json::Value) -> BTreeSet<String> {
    fn collect(value: &serde_json::Value, identities: &mut BTreeSet<String>) {
        match value {
            serde_json::Value::Object(object) => {
                if let Some(ulid) = object.get("ulid") {
                    identities.insert(ulid.as_str().expect("typed ID ULID").to_owned());
                }
                for child in object.values() {
                    collect(child, identities);
                }
            }
            serde_json::Value::Array(values) => {
                for child in values {
                    collect(child, identities);
                }
            }
            _ => {}
        }
    }

    let mut identities =
        BTreeSet::from([model["model_ulid"].as_str().expect("Model ULID").to_owned()]);
    collect(model, &mut identities);
    identities
}

fn rewrite_model_ulids(value: &mut serde_json::Value, identities: &BTreeMap<String, String>) {
    match value {
        serde_json::Value::Object(object) => {
            for key in ["model_ulid", "ulid"] {
                if let Some(value) = object.get_mut(key) {
                    let source = value.as_str().expect("ULID string");
                    *value = identities
                        .get(source)
                        .unwrap_or_else(|| panic!("unmapped semantic identity `{source}`"))
                        .clone()
                        .into();
                }
            }
            for child in object.values_mut() {
                rewrite_model_ulids(child, identities);
            }
        }
        serde_json::Value::Array(values) => {
            for child in values {
                rewrite_model_ulids(child, identities);
            }
        }
        _ => {}
    }
}

fn id_ulid(value: &serde_json::Value) -> String {
    value["ulid"].as_str().expect("typed ID ULID").to_owned()
}

fn erase_relation_implementation(model: &mut serde_json::Value) {
    for node in model["nodes"].as_array_mut().expect("Model node array") {
        let definition = node["definition"]
            .as_object_mut()
            .expect("typed node definition");
        if definition.get("kind").and_then(serde_json::Value::as_str) == Some("relation") {
            assert!(definition.remove("residuals").is_some());
        }
    }
}

fn assert_identity_normalized_flat_structure(packaged: &ModelDocument, explicit: &ModelDocument) {
    assert_eq!(packaged.aliases().len(), PACKAGED_TO_EXPLICIT.len() + 2);
    assert_eq!(explicit.aliases().len(), PACKAGED_TO_EXPLICIT.len());
    for eliminated in [
        "balance_law.body",
        "balance_law.displacement",
        "balance_law.load_potential",
    ] {
        assert_eq!(packaged.aliases().get(eliminated), None);
    }
    assert_eq!(
        packaged.aliases().get("balance_law.mu"),
        packaged.aliases().get("mu")
    );
    assert_eq!(
        packaged.aliases().get("balance_law.lambda"),
        packaged.aliases().get("lambda")
    );

    let packaged_envelope =
        ModelEnvelopeV4::from_program(packaged.program()).expect("packaged Model v4 envelope");
    let explicit_envelope =
        ModelEnvelopeV4::from_program(explicit.program()).expect("explicit Model v4 envelope");
    let mut packaged_value: serde_json::Value = serde_json::from_slice(
        &packaged_envelope
            .canonical_json()
            .expect("packaged canonical Model"),
    )
    .expect("packaged Model JSON");
    let explicit_value: serde_json::Value = serde_json::from_slice(
        &explicit_envelope
            .canonical_json()
            .expect("explicit canonical Model"),
    )
    .expect("explicit Model JSON");

    let mut identities = BTreeMap::from([(
        packaged.program().model().ulid().to_string(),
        explicit.program().model().ulid().to_string(),
    )]);
    for (packaged_name, explicit_name) in PACKAGED_TO_EXPLICIT {
        let packaged_id = packaged
            .aliases()
            .get(packaged_name)
            .unwrap_or_else(|| panic!("packaged symbol `{packaged_name}`"));
        let explicit_id = explicit
            .aliases()
            .get(explicit_name)
            .unwrap_or_else(|| panic!("explicit symbol `{explicit_name}`"));
        assert!(
            identities
                .insert(
                    packaged_id.ulid().to_string(),
                    explicit_id.ulid().to_string()
                )
                .is_none()
        );
    }

    let packaged_activations = activations_by_relation(&packaged_value);
    let explicit_activations = activations_by_relation(&explicit_value);
    assert_eq!(packaged_activations.len(), explicit_activations.len());
    for (relation, activation) in packaged_activations {
        let explicit_relation = identities.get(&relation).expect("mapped Relation");
        let explicit_activation = explicit_activations
            .get(explicit_relation)
            .expect("corresponding Activation");
        assert!(
            identities
                .insert(activation, explicit_activation.clone())
                .is_none()
        );
    }

    assert_eq!(
        identities.keys().cloned().collect::<BTreeSet<_>>(),
        collect_model_ulids(&packaged_value),
        "normalization covers every packaged semantic identity"
    );
    assert_eq!(
        identities.values().cloned().collect::<BTreeSet<_>>(),
        collect_model_ulids(&explicit_value),
        "normalization is a complete explicit-Model bijection"
    );

    rewrite_model_ulids(&mut packaged_value, &identities);
    let normalized = ModelEnvelopeV4::from_json(
        &serde_json::to_vec(&packaged_value).expect("normalized Model JSON"),
        Default::default(),
    )
    .expect("normalized Model v4");
    let mut packaged_value: serde_json::Value = serde_json::from_slice(
        &normalized
            .canonical_json()
            .expect("normalized canonical Model"),
    )
    .expect("normalized canonical Model JSON");
    erase_relation_implementation(&mut packaged_value);
    let mut explicit_value = explicit_value;
    erase_relation_implementation(&mut explicit_value);
    assert_eq!(
        packaged_value, explicit_value,
        "identity-normalized semantic structure agrees; expression-DAG canonicalization is owned by RFC 0073"
    );
}

fn resolved(
    program: &KernelProgram,
    cells: usize,
    revision: u64,
) -> eqiora::realization::ResolvedRealization {
    let plan = RealizationPlan::new(
        Space::continuous_lagrange(NonZeroU16::MIN),
        Discretization::new(
            DiscretizationMethod::ContinuousGalerkin,
            MeshPolicy::GeneratedUniform {
                cells_per_axis: NonZeroUsize::new(cells).expect("positive refinement"),
            },
            QuadraturePolicy::GaussLegendre {
                points_per_axis: NonZeroUsize::new(2).expect("two-point quadrature"),
            },
        ),
        SolverPlan::new(
            LinearSolver::ConjugateGradient,
            1.0e-12,
            1.0e-14,
            NonZeroUsize::new(10_000).expect("finite iteration limit"),
        )
        .expect("coercive solver plan"),
        Target::HostCpu {
            threads: NonZeroUsize::MIN,
        },
        ExecutionSchedule::Offline,
    )
    .expect("Q1 elasticity plan");
    let request = RealizationRequest::explicit(
        program.model(),
        SemanticRevision::new(program.revision().0),
        RealizationRevision::new(revision),
        plan,
    );
    resolve(
        &request,
        RealizationRequirements::new(
            NonZeroUsize::new(2).expect("two dimensions"),
            ScalarType::F64,
            VectorLayoutKind::Replicated,
        ),
        &RealizationCapabilities::isotropic_elasticity_2d_reference(),
    )
    .expect("reference elasticity capability admits the plan")
}

fn exact_displacement_and_gradient(point: &[f64]) -> ([f64; 2], [[f64; 2]; 2]) {
    let x = PI * point[0];
    let y = PI * point[1];
    let constitutive = 8.0;
    let amplitude = 1.0 / (2.0 * PI * constitutive);
    let sx = x.sin();
    let sy = y.sin();
    let sin_2x = (2.0 * x).sin();
    let sin_2y = (2.0 * y).sin();
    let mixed = -sin_2x * sin_2y / (2.0 * constitutive);
    (
        [
            -amplitude * sin_2x * sy.powi(2),
            -amplitude * sx.powi(2) * sin_2y,
        ],
        [
            [-((2.0 * x).cos()) * sy.powi(2) / constitutive, mixed],
            [mixed, -sx.powi(2) * (2.0 * y).cos() / constitutive],
        ],
    )
}

fn execution_provenance() -> ExecutionProvenanceV1 {
    ExecutionProvenanceV1::new(
        "eqiora.host.serial",
        env!("CARGO_PKG_VERSION"),
        "eqiora.reference.cg",
        env!("CARGO_PKG_VERSION"),
        ExecutionTopologyV1::Host {
            workers: NonZeroUsize::MIN,
        },
        ReductionPolicy::Reproducible,
    )
    .expect("host-serial reference execution provenance")
}

#[test]
fn exact_package_structure_order_and_flat_kernel_meaning_are_closed() {
    assert_verified_component_v0_1_boundary();
    assert_root_boundary();

    let component = verified_component_v0_1_release();
    let permuted_component =
        synthetic_component_release(PUBLIC_PACKAGE, SYNTHETIC_COMPONENT_PERMUTED, true);
    assert_eq!(
        component
            .package_identity()
            .expect("public component identity"),
        permuted_component
            .package_identity()
            .expect("permuted component identity")
    );
    assert_ne!(
        component.source_digest().expect("public source digest"),
        permuted_component
            .source_digest()
            .expect("permuted source digest")
    );

    let root = root_release(&component, "solid", PACKAGED_MANUFACTURED, false);
    let permuted_root = root_release(
        &permuted_component,
        "linear_elasticity_provider",
        PACKAGED_MANUFACTURED_PERMUTED,
        true,
    );
    assert_eq!(
        root.package_identity().expect("root identity"),
        permuted_root
            .package_identity()
            .expect("permuted root identity"),
        "dependency alias, declaration, binding, and file order are non-semantic"
    );

    let (packaged, _) = compile_locked(&component, &root);
    let (permuted, _) = compile_locked(&permuted_component, &permuted_root);
    assert_eq!(
        packaged.model().canonical_json().expect("packaged Model"),
        permuted.model().canonical_json().expect("permuted Model")
    );
    assert_eq!(packaged.model().digest(), permuted.model().digest());
    assert_identity_normalized_flat_structure(packaged.model(), &explicit_document());
}

#[test]
fn package_blind_lowering_solution_and_convergence_match_the_explicit_model() {
    let (component, root) = baseline_package();
    let (packaged, _) = compile_locked(&component, &root);
    let explicit = explicit_document();
    let packaged_lowered = lower_isotropic_elasticity_cartesian_2d(packaged.model().program())
        .expect("ordinary lowerer accepts the packaged flat Kernel");
    let explicit_lowered = lower_isotropic_elasticity_cartesian_2d(explicit.program())
        .expect("ordinary lowerer accepts the explicit flat Kernel");
    assert_eq!(packaged_lowered.bounds(), explicit_lowered.bounds());
    assert_eq!(
        packaged_lowered.shear_modulus(),
        explicit_lowered.shear_modulus()
    );
    assert_eq!(
        packaged_lowered.first_lame_parameter(),
        explicit_lowered.first_lame_parameter()
    );
    assert_eq!(
        packaged_lowered
            .shear_modulus_expression()
            .parameter_values(),
        explicit_lowered
            .shear_modulus_expression()
            .parameter_values()
    );
    assert_eq!(
        packaged_lowered
            .first_lame_parameter_expression()
            .parameter_values(),
        explicit_lowered
            .first_lame_parameter_expression()
            .parameter_values()
    );
    for point in [[0.25, 0.25], [0.37, 0.61], [0.8, 0.2]] {
        assert_eq!(
            packaged_lowered
                .load_potential_expression()
                .evaluate(&point)
                .expect("packaged load tape"),
            explicit_lowered
                .load_potential_expression()
                .evaluate(&point)
                .expect("explicit load tape")
        );
        assert_eq!(
            packaged_lowered
                .conservative_body_force(&point)
                .expect("packaged conservative load"),
            explicit_lowered
                .conservative_body_force(&point)
                .expect("explicit conservative load")
        );
    }

    let error_rule =
        QuadratureRule::tensor_product_gauss_legendre(2, 4).expect("independent error rule");
    let mut l2_errors = Vec::new();
    let mut h1_errors = Vec::new();
    for (revision, cells) in [4, 8, 16, 32].into_iter().enumerate() {
        let (_, packaged_solution) = solve_resolved_isotropic_elasticity_cartesian_2d(
            packaged.model().program(),
            &resolved(packaged.model().program(), cells, revision as u64 + 1),
            &REFERENCE_LINEAR_SOLVER,
        )
        .expect("packaged elasticity solve");
        let (_, explicit_solution) = solve_resolved_isotropic_elasticity_cartesian_2d(
            explicit.program(),
            &resolved(explicit.program(), cells, revision as u64 + 1),
            &REFERENCE_LINEAR_SOLVER,
        )
        .expect("explicit elasticity solve");
        assert_eq!(packaged_solution, explicit_solution);
        for component in 0..COMPONENTS {
            let scale = packaged_solution.boundary_reaction()[component]
                .abs()
                .max(packaged_solution.integrated_body_force()[component].abs())
                .max(1.0);
            assert!(
                (packaged_solution.boundary_reaction()[component]
                    + packaged_solution.integrated_body_force()[component])
                    .abs()
                    <= 2.0e-11 * scale
            );
        }
        let norms = packaged_solution
            .displacement()
            .error_norms(&exact_displacement_and_gradient, &error_rule)
            .expect("continuous packaged error evidence");
        l2_errors.push(norms.l2());
        h1_errors.push(norms.h1_seminorm());
    }
    assert!(
        l2_errors.windows(2).all(|errors| errors[1] < errors[0]),
        "L2 errors must decrease on every refinement: {l2_errors:?}"
    );
    assert!(
        h1_errors.windows(2).all(|errors| errors[1] < errors[0]),
        "H1 errors must decrease on every refinement: {h1_errors:?}"
    );
    for errors in l2_errors.windows(2).skip(1) {
        assert!((errors[0] / errors[1]).log2() >= 1.9, "{l2_errors:?}");
    }
    for errors in h1_errors.windows(2).skip(1) {
        assert!((errors[0] / errors[1]).log2() >= 0.9, "{h1_errors:?}");
    }

    let renamed_component = synthetic_component_release(
        "org.example.renamed_linear_elasticity",
        SYNTHETIC_COMPONENT,
        true,
    );
    assert_ne!(
        renamed_component
            .package_identity()
            .expect("renamed provider identity"),
        component
            .package_identity()
            .expect("public provider identity")
    );
    let renamed_root = root_release(
        &renamed_component,
        "renamed_provider",
        PACKAGED_MANUFACTURED,
        false,
    );
    let (renamed, _) = compile_locked(&renamed_component, &renamed_root);
    assert_identity_normalized_flat_structure(renamed.model(), &explicit);
    let renamed_lowered = lower_isotropic_elasticity_cartesian_2d(renamed.model().program())
        .expect("lowerer does not inspect the provider package name");
    assert_eq!(renamed_lowered.bounds(), packaged_lowered.bounds());
    assert_eq!(renamed_lowered.shear_modulus(), 3.0);
    assert_eq!(renamed_lowered.first_lame_parameter(), 2.0);
    let (_, renamed_solution) = solve_resolved_isotropic_elasticity_cartesian_2d(
        renamed.model().program(),
        &resolved(renamed.model().program(), 8, 1),
        &REFERENCE_LINEAR_SOLVER,
    )
    .expect("executor does not inspect the provider package name");
    let (_, packaged_solution) = solve_resolved_isotropic_elasticity_cartesian_2d(
        packaged.model().program(),
        &resolved(packaged.model().program(), 8, 1),
        &REFERENCE_LINEAR_SOLVER,
    )
    .expect("public provider reference solve");
    assert_eq!(renamed_solution, packaged_solution);
}

#[test]
fn nonzero_linear_potential_preserves_force_and_reaction_across_the_package_boundary() {
    let component = verified_component_v0_1_release();
    let root = root_release(&component, "solid", PACKAGED_LINEAR_LOAD, false);
    let (packaged, _) = compile_locked(&component, &root);
    let explicit = explicit_linear_load_document();

    let packaged_lowered = lower_isotropic_elasticity_cartesian_2d(packaged.model().program())
        .expect("ordinary lowerer accepts the packaged nonzero load");
    let explicit_lowered = lower_isotropic_elasticity_cartesian_2d(explicit.program())
        .expect("ordinary lowerer accepts the explicit nonzero load");
    for point in [[0.0, 0.0], [0.37, 0.61], [1.0, 1.0]] {
        assert_eq!(
            packaged_lowered
                .load_potential_expression()
                .evaluate(&point)
                .expect("packaged linear potential"),
            explicit_lowered
                .load_potential_expression()
                .evaluate(&point)
                .expect("explicit linear potential")
        );
        assert_eq!(
            packaged_lowered
                .conservative_body_force(&point)
                .expect("packaged constant force"),
            explicit_lowered
                .conservative_body_force(&point)
                .expect("explicit constant force")
        );
    }

    let (_, packaged_solution) = solve_resolved_isotropic_elasticity_cartesian_2d(
        packaged.model().program(),
        &resolved(packaged.model().program(), 8, 1),
        &REFERENCE_LINEAR_SOLVER,
    )
    .expect("packaged nonzero-load solve");
    let (_, explicit_solution) = solve_resolved_isotropic_elasticity_cartesian_2d(
        explicit.program(),
        &resolved(explicit.program(), 8, 1),
        &REFERENCE_LINEAR_SOLVER,
    )
    .expect("explicit nonzero-load solve");
    assert_eq!(packaged_solution, explicit_solution);
    for (actual, exact) in packaged_solution
        .integrated_body_force()
        .into_iter()
        .zip([1.0, 0.0])
    {
        assert!((actual - exact).abs() < 3.0e-14);
    }
    for component in 0..COMPONENTS {
        assert!(
            (packaged_solution.boundary_reaction()[component]
                + packaged_solution.integrated_body_force()[component])
                .abs()
                < 2.0e-11
        );
    }
}

#[test]
fn package_compilation_realization_and_run_v2_form_one_exact_lineage() {
    let (component, root) = baseline_package();
    let (packaged, resolution) = compile_locked(&component, &root);
    let realization = resolved(packaged.model().program(), 8, 1);
    let (_, solution) = solve_resolved_isotropic_elasticity_cartesian_2d(
        packaged.model().program(),
        &realization,
        &REFERENCE_LINEAR_SOLVER,
    )
    .expect("accepted packaged execution precedes Run evidence");
    assert!(
        solution.solve_report().true_residual_norm() <= solution.solve_report().residual_target()
    );

    let model_reference = packaged
        .model()
        .artifact_reference()
        .expect("version-neutral packaged Model reference");
    let realization = RealizationEnvelopeV1::from_resolved(
        &model_reference,
        &realization,
        LayoutArtifacts::Replicated,
    )
    .expect("Realization v1");
    let run = RunManifestV2::new(&realization, execution_provenance()).expect("output-less Run v2");
    let binding = packaged
        .bind_execution_v2(&realization, &run)
        .expect("exact package execution binding");
    packaged
        .validate_execution_v2_binding(&binding, &realization, &run, &resolution)
        .expect("complete in-memory lineage");

    let compilation_bytes = packaged
        .compilation()
        .canonical_json()
        .expect("package compilation JSON");
    let compilation = PackageCompilationRecordV1::from_json(&compilation_bytes)
        .expect("replayed package compilation");
    compilation
        .validate_against(&resolution)
        .expect("replayed exact resolution");

    let model_bytes = packaged.model().canonical_json().expect("Model v4 JSON");
    let model = ExactModelCodec::V4
        .replay(&model_bytes)
        .expect("replayed Model v4");
    assert_eq!(model.canonical_json().expect("replayed Model"), model_bytes);

    let realization_bytes = realization.canonical_json().expect("Realization v1 JSON");
    let realization = RealizationEnvelopeV1::from_json(&realization_bytes, Default::default())
        .expect("replayed Realization v1");
    realization
        .validate_model_artifact(
            &model
                .artifact_reference()
                .expect("replayed Model reference"),
        )
        .expect("Realization points to the replayed Model");

    let run_bytes = run.canonical_json().expect("Run v2 JSON");
    let run = RunManifestV2::from_json(&run_bytes, Default::default()).expect("replayed Run v2");
    run.validate_against(&realization)
        .expect("Run points to the replayed Realization");

    let binding_bytes = binding
        .canonical_json()
        .expect("package execution binding JSON");
    let binding = PackageExecutionBindingV1::from_json(&binding_bytes)
        .expect("replayed package execution binding");
    packaged
        .validate_execution_v2_binding(&binding, &realization, &run, &resolution)
        .expect("canonical package-to-Run lineage replay");
}
