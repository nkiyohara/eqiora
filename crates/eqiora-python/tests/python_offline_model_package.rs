use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use eqiora::api::package::{PackageCompilationError, PackagedModelDocument};
use eqiora::package::{
    AuthorManifestV1, BundleEntryV1, BundleRoleV1, CanonicalDeclaration, DeclarationKindV1,
    ExactVersion, InMemoryPackageStore, NormalizedRelativePath, PackageCompilationRecordV1,
    PackageReleaseV1, QualifiedName, ResolutionRecordV1, SemanticContentV1, SemanticDeclarationV1,
    SourceFileV1, VisibilityV1,
};
use eqiora::{Diagnostic, Severity};
use pyo3::exceptions::PyTypeError;
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyBytes, PyDict, PyModule};

const OFFLINE_RESOLUTION_FILE: &[u8] =
    include_bytes!("../../../verify/packages/offline-model-package/models/resolution.json");
const OFFLINE_LIBRARY: &[u8] = include_bytes!(
    "../../../verify/packages/offline-model-package/models/store/ce343238d92f202646d2dd2947d68c311eac90aa711aa9d0e3905fa170f6f3f1.json"
);
const OFFLINE_ROOT: &[u8] = include_bytes!(
    "../../../verify/packages/offline-model-package/models/store/cd7afe063d06007b97c108d3957e1bdc92e64fe47adfc7ac92975fee4f2c0d28.json"
);
const EXPECTED_OFFLINE_MODEL_FILE: &[u8] = include_bytes!(
    "../../../verify/artifacts/current-model-relational-identity-transition/expected/deterministic/offline-model-package/model.json"
);
const EXPECTED_OFFLINE_COMPILATION_FILE: &[u8] = include_bytes!(
    "../../../verify/artifacts/current-model-relational-identity-transition/expected/deterministic/offline-model-package/compilation.json"
);
const TYPED_RESOLUTION_FILE: &[u8] = include_bytes!(
    "../../../verify/interfaces/python-offline-model-package/models/typed-execution-lineage/resolution.json"
);
const TYPED_RELEASE: &[u8] = include_bytes!(
    "../../../verify/interfaces/python-offline-model-package/models/typed-execution-lineage/store/4f3aa811b814ac7fb959f777ff5d758804e2e68593a568ee8935b122c9565462.json"
);
const OFFLINE_STORE: &str = "../../../verify/packages/offline-model-package/models/store";
const TYPED_STORE: &str =
    "../../../verify/interfaces/python-offline-model-package/models/typed-execution-lineage/store";
const OFFLINE_MODEL_ID: &str = "3JNCJVGEYX9N2QSYVEXRXWXWF4";
const OFFLINE_MODEL_DIGEST: &str =
    "92837f0f85ff4a1310af0ca6e412d3ace81393df837d017caf5bfabeb8f6c1a1";
const OFFLINE_COMPILATION_DIGEST: &str =
    "a6e31415d973c5dc23a92a101ba3db7cef7b1b70b0dc51d2b73214f1fc00bf49";
const TYPED_MODEL_ID: &str = "7Q7ZYW89BV0RH2HSB3S5ZMTY0K";
const TYPED_SOURCE_DIGEST: &str =
    "4f3aa811b814ac7fb959f777ff5d758804e2e68593a568ee8935b122c9565462";
const TYPED_RESOLUTION_DIGEST: &str =
    "38b5bb0c7e1f8aa7baa5e690157014a974c446f8f38fcd19d6b73b981e9ca810";
const TYPED_MODEL_DIGEST: &str = "c2c35e6b58f6ee0d40b8aa2bd0c252e519eec6f6779e39366ae2e28cdbd5300a";
const TYPED_COMPILATION_DIGEST: &str =
    "6e72043a1d0569d7488717cd7ffdf54a01c7e5e65262cecc3a49fcdce645dec0";

fn canonical_fixture(bytes: &[u8]) -> &[u8] {
    let canonical = bytes
        .strip_suffix(b"\n")
        .expect("repository JSON fixture must have exactly one formatting LF");
    assert!(!canonical.ends_with(b"\n"));
    canonical
}

fn fixture_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(relative)
}

fn compile_package(
    module: &Bound<'_, PyModule>,
    py: Python<'_>,
    store: &Path,
    resolution: &[u8],
    entry_model: &str,
) -> PyResult<Py<PyAny>> {
    let kwargs = PyDict::new(py);
    kwargs.set_item("entry_model", entry_model)?;
    module
        .getattr("compile_package")?
        .call(
            (
                store.to_str().expect("fixture path must be Unicode"),
                PyBytes::new(py, resolution),
            ),
            Some(&kwargs),
        )
        .map(Bound::unbind)
}

fn assert_no_lineage(model: &Bound<'_, PyAny>) -> PyResult<()> {
    assert_eq!(
        model
            .getattr("package_compilation_digest")?
            .extract::<Option<String>>()?,
        None
    );
    Ok(())
}

fn assert_exception(
    module: &Bound<'_, PyModule>,
    py: Python<'_>,
    error: PyErr,
    class: &str,
    category: &str,
    code: &str,
) -> PyResult<()> {
    assert!(error.is_instance(py, &module.getattr(class)?));
    let value = error.value(py);
    assert_eq!(value.getattr("category")?.extract::<String>()?, category);
    let diagnostics = value.getattr("diagnostics")?;
    assert!(diagnostics.len()? > 0);
    assert_eq!(
        diagnostics
            .get_item(0)?
            .getattr("code")?
            .extract::<String>()?,
        code
    );
    Ok(())
}

fn offline_direct() -> (InMemoryPackageStore, ResolutionRecordV1) {
    let library = PackageReleaseV1::from_json(OFFLINE_LIBRARY).expect("accepted library release");
    let root = PackageReleaseV1::from_json(OFFLINE_ROOT).expect("accepted root release");
    let mut store = InMemoryPackageStore::default();
    store.insert(&library).expect("insert accepted library");
    store.insert(&root).expect("insert accepted root");
    let resolution = ResolutionRecordV1::from_json(canonical_fixture(OFFLINE_RESOLUTION_FILE))
        .expect("accepted canonical resolution");
    (store, resolution)
}

fn assert_typed_fixture_matches_accepted_identities() {
    let resolution = canonical_fixture(TYPED_RESOLUTION_FILE);
    let release =
        PackageReleaseV1::from_json(TYPED_RELEASE).expect("precommitted typed-lineage release");
    assert_eq!(
        release.source_digest().expect("source digest").to_hex(),
        TYPED_SOURCE_DIGEST
    );
    let direct_resolution =
        ResolutionRecordV1::from_json(resolution).expect("precommitted typed-lineage resolution");
    assert_eq!(
        direct_resolution
            .digest()
            .expect("resolution digest")
            .to_hex(),
        TYPED_RESOLUTION_DIGEST
    );
    let mut direct_store = InMemoryPackageStore::default();
    direct_store.insert(&release).expect("insert typed release");
    let direct = PackagedModelDocument::compile_locked(&direct_store, &direct_resolution, "Main")
        .expect("accepted typed-lineage package");
    let direct_reference = direct
        .model()
        .artifact_reference()
        .expect("typed Model reference");
    assert_eq!(direct_reference.model().ulid().to_string(), TYPED_MODEL_ID);
    assert_eq!(direct_reference.semantic_revision().get(), 1);
    assert_eq!(
        direct.model().digest().expect("Model digest"),
        TYPED_MODEL_DIGEST
    );
    assert_eq!(
        direct
            .compilation()
            .digest()
            .expect("compilation digest")
            .to_hex(),
        TYPED_COMPILATION_DIGEST
    );
}

#[test]
fn secondary_fixture_is_exactly_the_accepted_typed_lineage_package() {
    assert_typed_fixture_matches_accepted_identities();
}

fn assert_compiler_diagnostics_equal(
    module: &Bound<'_, PyModule>,
    py: Python<'_>,
    error: PyErr,
    expected: &[Diagnostic],
) -> PyResult<()> {
    assert!(error.is_instance(py, &module.getattr("ValidationError")?));
    let value = error.value(py);
    assert_eq!(
        value.getattr("category")?.extract::<String>()?,
        "validation"
    );
    let actual = value.getattr("diagnostics")?;
    assert_eq!(actual.len()?, expected.len());
    for (index, expected) in expected.iter().enumerate() {
        let actual = actual.get_item(index)?;
        let severity = match expected.severity() {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Note => "note",
        };
        assert_eq!(actual.getattr("source")?.extract::<String>()?, "kernel");
        assert_eq!(
            actual.getattr("code")?.extract::<String>()?,
            expected.code().to_string()
        );
        assert_eq!(actual.getattr("severity")?.extract::<String>()?, severity);
        assert_eq!(
            actual.getattr("message")?.extract::<String>()?,
            expected.message()
        );
        assert_eq!(
            actual
                .getattr("graph_path")?
                .extract::<Option<Vec<String>>>()?,
            expected.graph_path().map(|path| path.segments().to_vec())
        );
        assert_eq!(
            actual
                .getattr("source_span")?
                .extract::<Option<(String, u32, u32)>>()?,
            expected
                .source_span()
                .map(|span| (span.file.clone(), span.start, span.end))
        );
        assert_eq!(
            actual.getattr("suggestion")?.extract::<Option<String>>()?,
            expected.suggestion().map(|patch| patch.summary.clone())
        );
    }
    Ok(())
}

#[test]
fn exact_package_projection_matches_frozen_artifacts_and_direct_diagnostics() -> PyResult<()> {
    Python::initialize();
    Python::attach(|py| {
        let native = pyo3::wrap_pymodule!(_eqiora::_eqiora)(py);
        let module = native.bind(py);
        let resolution = canonical_fixture(OFFLINE_RESOLUTION_FILE);
        let model = compile_package(module, py, &fixture_path(OFFLINE_STORE), resolution, "Main")?;
        let model = model.bind(py);
        let expected_model = canonical_fixture(EXPECTED_OFFLINE_MODEL_FILE);
        assert_eq!(
            model.call_method0("to_json")?.extract::<Vec<u8>>()?,
            expected_model
        );
        assert_eq!(
            model.getattr("model_id")?.extract::<String>()?,
            OFFLINE_MODEL_ID
        );
        assert_eq!(
            model.getattr("digest")?.extract::<String>()?,
            OFFLINE_MODEL_DIGEST
        );
        assert_eq!(
            model
                .getattr("revision")?
                .getattr("number")?
                .extract::<u64>()?,
            1
        );
        assert_eq!(
            model
                .getattr("package_compilation_digest")?
                .extract::<String>()?,
            OFFLINE_COMPILATION_DIGEST
        );

        let expected_compilation = PackageCompilationRecordV1::from_json(canonical_fixture(
            EXPECTED_OFFLINE_COMPILATION_FILE,
        ))
        .expect("accepted compilation artifact");
        assert_eq!(
            expected_compilation
                .digest()
                .expect("compilation digest")
                .to_hex(),
            OFFLINE_COMPILATION_DIGEST
        );
        let replayed = module
            .getattr("replay")?
            .call1((PyBytes::new(py, expected_model),))?;
        assert!(model.eq(&replayed)?);
        assert_eq!(model.hash()?, replayed.hash()?);
        assert_eq!(
            model.repr()?.extract::<String>()?,
            replayed.repr()?.extract::<String>()?
        );
        assert_no_lineage(&replayed)?;
        assert!(
            model
                .call_method1("structurally_equivalent", (&replayed,))?
                .extract::<bool>()?
        );
        assert_eq!(
            model
                .getattr("package_compilation_digest")?
                .extract::<String>()?,
            OFFLINE_COMPILATION_DIGEST
        );

        let (store, direct_resolution) = offline_direct();
        let expected =
            match PackagedModelDocument::compile_locked(&store, &direct_resolution, "Unknown") {
                Err(PackageCompilationError::Diagnostics(diagnostics)) => diagnostics,
                other => panic!("expected direct compiler diagnostics, received {other:?}"),
            };
        let error = compile_package(
            module,
            py,
            &fixture_path(OFFLINE_STORE),
            resolution,
            "Unknown",
        )
        .expect_err("unknown root-local Model must reject");
        assert_compiler_diagnostics_equal(module, py, error, &expected)?;
        Ok(())
    })
}

#[test]
fn lineage_is_private_to_the_exact_packaged_origin() -> PyResult<()> {
    Python::initialize();
    Python::attach(|py| {
        let native = pyo3::wrap_pymodule!(_eqiora::_eqiora)(py);
        let module = native.bind(py);
        let source = r#"
model source_model {
  field x: 1 = 1;
  relation hold continuous { derivative(x) = 0; }
}
"#;
        let source_model = module.getattr("compile")?.call1((source,))?;
        assert_no_lineage(&source_model)?;

        let field_kwargs = PyDict::new(py);
        field_kwargs.set_item("initial", 1.0)?;
        let field = module.getattr("Field")?.call(("x",), Some(&field_kwargs))?;
        let residual = module.getattr("derivative")?.call1((&field,))?;
        let relation_kwargs = PyDict::new(py);
        relation_kwargs.set_item("residual", &residual)?;
        let relation = module
            .getattr("Relation")?
            .call(("hold",), Some(&relation_kwargs))?;
        let defined = module.getattr("Model")?.getattr("define")?.call1((
            "defined_model",
            &field,
            &relation,
        ))?;
        assert_no_lineage(&defined)?;

        let resolution = canonical_fixture(TYPED_RESOLUTION_FILE);
        let packaged = compile_package(module, py, &fixture_path(TYPED_STORE), resolution, "Main")?;
        let packaged = packaged.bind(py);
        assert_eq!(
            packaged.getattr("model_id")?.extract::<String>()?,
            TYPED_MODEL_ID
        );
        assert_eq!(
            packaged.getattr("digest")?.extract::<String>()?,
            TYPED_MODEL_DIGEST
        );
        assert_eq!(
            packaged
                .getattr("package_compilation_digest")?
                .extract::<String>()?,
            TYPED_COMPILATION_DIGEST
        );
        let edit = packaged.call_method1("preview_value_edit", ("wave_number", 4.0))?;
        let child = packaged.call_method1("commit", (&edit,))?;
        assert_no_lineage(&child)?;
        assert_eq!(
            packaged
                .getattr("package_compilation_digest")?
                .extract::<String>()?,
            TYPED_COMPILATION_DIGEST
        );
        assert_ne!(
            packaged.getattr("digest")?.extract::<String>()?,
            child.getattr("digest")?.extract::<String>()?
        );
        let replayed_child = module
            .getattr("replay")?
            .call1((child.call_method0("to_json")?,))?;
        assert_no_lineage(&replayed_child)?;
        Ok(())
    })
}

static NEXT_SCRATCH: AtomicU64 = AtomicU64::new(0);

struct Scratch(PathBuf);

impl Scratch {
    fn create(label: &str) -> Self {
        let home = PathBuf::from(std::env::var_os("HOME").expect("HOME is required by this test"));
        let root = home.join(".cache/eqiora/oracle-tests").join(format!(
            "python-package-{label}-{}-{}",
            std::process::id(),
            NEXT_SCRATCH.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).expect("create unique home-backed test store");
        Self(root)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).expect("remove unique home-backed test store");
    }
}

fn dishonest_store() -> (Scratch, Vec<u8>) {
    let path = NormalizedRelativePath::parse("src/main.eqi").expect("source path");
    let manifest = AuthorManifestV1::new(
        QualifiedName::parse("org.example.FalseClaim").expect("package name"),
        ExactVersion::parse("1.0.0").expect("version"),
        vec![],
        vec![BundleEntryV1::new(path.clone(), BundleRoleV1::ModelSource)],
    )
    .expect("manifest");
    let semantic = SemanticContentV1::new(vec![SemanticDeclarationV1::new(
        QualifiedName::parse("Main").expect("declaration"),
        DeclarationKindV1::Model,
        VisibilityV1::Private,
        CanonicalDeclaration::new("eqiora.source-declaration.v1:sha256:deadbeef")
            .expect("deliberately false claim"),
    )])
    .expect("semantic claim");
    let release = PackageReleaseV1::new(
        manifest,
        semantic,
        vec![SourceFileV1::new(
            path,
            BundleRoleV1::ModelSource,
            b"model Main {}\n".to_vec(),
        )],
    )
    .expect("locally valid dishonest release");
    let resolution =
        ResolutionRecordV1::from_exact_releases(&release, &[]).expect("exact dishonest resolution");
    let scratch = Scratch::create("semantic-mismatch");
    let digest = release.source_digest().expect("source digest");
    fs::write(
        scratch.0.join(format!("{digest}.json")),
        release.canonical_json().expect("release JSON"),
    )
    .expect("write exact dishonest release");
    (
        scratch,
        resolution.canonical_json().expect("resolution JSON"),
    )
}

#[test]
fn invalid_shapes_and_package_failures_are_mapped_without_a_partial_model() -> PyResult<()> {
    Python::initialize();
    Python::attach(|py| {
        let native = pyo3::wrap_pymodule!(_eqiora::_eqiora)(py);
        let module = native.bind(py);
        let callable = module.getattr("compile_package")?;
        let resolution = canonical_fixture(OFFLINE_RESOLUTION_FILE);
        let store = fixture_path(OFFLINE_STORE);
        let store = store.to_str().expect("Unicode fixture path");
        let bytes = PyBytes::new(py, resolution);

        let bytearray = py
            .import("builtins")?
            .getattr("bytearray")?
            .call1((bytes.clone(),))?;
        let memoryview = py
            .import("builtins")?
            .getattr("memoryview")?
            .call1((bytes.clone(),))?;
        for (label, args, kwargs) in [
            (
                "bytes path",
                (PyBytes::new(py, store.as_bytes()), bytes.clone())
                    .into_pyobject(py)?
                    .into_any(),
                Some({
                    let kwargs = PyDict::new(py);
                    kwargs.set_item("entry_model", "Main")?;
                    kwargs
                }),
            ),
            (
                "bytearray resolution",
                (store, &bytearray).into_pyobject(py)?.into_any(),
                Some({
                    let kwargs = PyDict::new(py);
                    kwargs.set_item("entry_model", "Main")?;
                    kwargs
                }),
            ),
            (
                "memoryview resolution",
                (store, &memoryview).into_pyobject(py)?.into_any(),
                Some({
                    let kwargs = PyDict::new(py);
                    kwargs.set_item("entry_model", "Main")?;
                    kwargs
                }),
            ),
        ] {
            let error = callable
                .call(args.cast()?, kwargs.as_ref())
                .expect_err(label);
            assert!(error.is_instance_of::<PyTypeError>(py), "{label}: {error}");
        }
        let positional = callable
            .call1((store, bytes.clone(), "Main"))
            .expect_err("entry_model must be keyword-only");
        assert!(positional.is_instance_of::<PyTypeError>(py));
        let missing = callable
            .call1((store, bytes.clone()))
            .expect_err("entry_model is required");
        assert!(missing.is_instance_of::<PyTypeError>(py));
        let wrong_entry = PyDict::new(py);
        wrong_entry.set_item("entry_model", 1)?;
        let wrong_entry = callable
            .call((store, bytes), Some(&wrong_entry))
            .expect_err("entry_model must be str");
        assert!(wrong_entry.is_instance_of::<PyTypeError>(py));

        let (dishonest, dishonest_resolution) = dishonest_store();
        let error = compile_package(module, py, &dishonest.0, &dishonest_resolution, "Main")
            .expect_err("semantic-content mismatch must reject without a Model");
        assert_exception(
            module,
            py,
            error,
            "CompatibilityError",
            "compatibility",
            "EQ0901",
        )?;
        Ok(())
    })
}
