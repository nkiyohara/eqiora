use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict, PyDictMethods, PyModule, PyModuleMethods};
use std::process::Command;

const SOURCE: &str = r#"
model decay {
  field x: 1 = 1;
  parameter rate: 1 / s = 1;
  relation flow continuous {
    derivative(x) + rate * x = 0;
  }
}
"#;

const PANIC_PAYLOAD: &str = "private panic payload must not cross Python boundary";
const PANIC_PROBE_CHILD: &str = "EQIORA_PYTHON_PANIC_PROBE_CHILD";

#[pyfunction]
fn boundary_panic(py: Python<'_>) -> PyResult<()> {
    _eqiora::panic_boundary(py, || -> PyResult<()> { panic!("{PANIC_PAYLOAD}") })
}

#[test]
fn python_control_plane_preserves_identity_and_fails_closed() -> PyResult<()> {
    Python::initialize();
    Python::attach(|py| {
        let loaded = py
            .import("sys")?
            .getattr("modules")?
            .cast_into::<PyDict>()?;
        for optional in ["numpy", "torch", "jax"] {
            assert!(
                !loaded.contains(optional)?,
                "{optional} was loaded before the native control module initialized"
            );
        }

        let native_module = pyo3::wrap_pymodule!(_eqiora::_eqiora)(py);
        let module = native_module.bind(py);
        assert_eq!(
            module.getattr("__version__")?.extract::<String>()?,
            "0.1.0a1"
        );
        for optional in ["numpy", "torch", "jax"] {
            assert!(
                !loaded.contains(optional)?,
                "initializing the control module imported optional module {optional}"
            );
        }
        let exception_base = module.getattr("EqioraError")?;
        let is_subclass = py.import("builtins")?.getattr("issubclass")?;
        for family in [
            "ValidationError",
            "CompatibilityError",
            "CapabilityError",
            "ExecutionError",
            "CancellationError",
            "InternalError",
        ] {
            assert!(
                is_subclass
                    .call1((module.getattr(family)?, &exception_base))?
                    .extract::<bool>()?,
                "{family} must remain a distinct EqioraError family"
            );
        }
        let manually_constructed = module.getattr("ValidationError")?.call1(("manual",))?;
        assert_eq!(
            manually_constructed
                .getattr("category")?
                .extract::<String>()?,
            "validation"
        );
        assert_eq!(manually_constructed.getattr("diagnostics")?.len()?, 0);

        let compile_kwargs = PyDict::new(py);
        compile_kwargs.set_item("filename", "python-control-plane.eqi")?;
        let base = module
            .getattr("compile")?
            .call((SOURCE,), Some(&compile_kwargs))?;
        let base_bytes = model_bytes(&base)?;
        let base_digest = base.getattr("digest")?.extract::<String>()?;
        let base_model_id = base.getattr("model_id")?.extract::<String>()?;
        let base_revision = revision_number(&base)?;

        let replayed = eqiora::api::ModelDocument::replay(&base_bytes)
            .expect("Python-produced current Model must replay through the Rust facade");
        let replayed_reference = replayed
            .artifact_reference()
            .expect("the validated replay must retain a typed Model reference");
        assert_eq!(
            replayed.canonical_json().unwrap(),
            base_bytes,
            "Python must expose the exact canonical artifact bytes"
        );
        assert_eq!(replayed.digest().unwrap(), base_digest);
        assert_eq!(replayed_reference.model().ulid().to_string(), base_model_id);
        assert_eq!(replayed_reference.semantic_revision().get(), base_revision);

        let edit = base.call_method1("preview_value_edit", ("rate", 2.0))?;
        assert_eq!(
            edit.getattr("base_digest")?.extract::<String>()?,
            base_digest
        );
        assert_eq!(
            edit.getattr("base_revision")?.extract::<u64>()?,
            base_revision
        );
        let child = base.call_method1("commit", (&edit,))?;
        assert_eq!(
            model_bytes(&base)?,
            base_bytes,
            "the base Model was mutated"
        );
        assert_eq!(base.getattr("digest")?.extract::<String>()?, base_digest);
        assert_eq!(
            base.getattr("model_id")?.extract::<String>()?,
            base_model_id
        );
        assert_eq!(revision_number(&base)?, base_revision);

        assert_eq!(
            child.getattr("model_id")?.extract::<String>()?,
            base_model_id,
            "an edit must advance one Model rather than minting another"
        );
        assert_eq!(revision_number(&child)?, base_revision + 1);
        assert_ne!(child.getattr("digest")?.extract::<String>()?, base_digest);
        let child_bytes = model_bytes(&child)?;
        let child_replay = eqiora::api::ModelDocument::replay(&child_bytes).unwrap();
        assert_eq!(child_replay.canonical_json().unwrap(), child_bytes);
        assert_eq!(
            child_replay.digest().unwrap(),
            child.getattr("digest")?.extract::<String>()?
        );

        let replayed_child = module
            .getattr("replay")?
            .call1((PyBytes::new(py, &child_bytes),))?;
        let parameter_id = replayed_child
            .getattr("parameter_ids")?
            .get_item(0)?
            .extract::<String>()?;
        let grandchild_edit =
            replayed_child.call_method1("preview_value_edit", (parameter_id, 3.0))?;
        let grandchild = replayed_child.call_method1("commit", (&grandchild_edit,))?;
        assert_eq!(revision_number(&grandchild)?, base_revision + 2);

        let sibling_edit = base.call_method1("preview_value_edit", ("rate", 3.0))?;
        let sibling = base.call_method1("commit", (&sibling_edit,))?;
        let child_state_edit = child.call_method1("preview_value_edit", ("x", 2.0))?;
        let sibling_state_edit = sibling.call_method1("preview_value_edit", ("x", 2.0))?;
        assert_ne!(
            child_state_edit.getattr("key")?.extract::<String>()?,
            sibling_state_edit.getattr("key")?.extract::<String>()?
        );
        assert!(
            !child_state_edit.eq(&sibling_state_edit)?,
            "edits over divergent base artifacts must retain distinct identity"
        );

        let stale = child
            .call_method1("commit", (&edit,))
            .expect_err("an edit prepared for the base must be stale on its child");
        assert_exception(
            module,
            py,
            stale,
            "ValidationError",
            "validation",
            Some("EQ0106"),
        )?;

        let malformed = module
            .getattr("replay")?
            .call1((PyBytes::new(py, b"{}"),))
            .expect_err("malformed current Model wire must fail closed");
        assert_exception(
            module,
            py,
            malformed,
            "CompatibilityError",
            "compatibility",
            Some("EQ0901"),
        )?;

        let invalid_source = module
            .getattr("compile")?
            .call1(("model broken { field ; }",))
            .expect_err("invalid source must be rejected by the shared compiler");
        assert_exception(
            module,
            py,
            invalid_source,
            "ValidationError",
            "validation",
            None,
        )?;

        let run_kwargs = PyDict::new(py);
        run_kwargs.set_item("end_time", 1.0)?;
        run_kwargs.set_item("max_step", 0.0)?;
        let invalid_execution = module
            .getattr("submit")?
            .call((&base,), Some(&run_kwargs))
            .expect_err("zero maximum step must be rejected by execution policy");
        assert_exception(
            module,
            py,
            invalid_execution,
            "ExecutionError",
            "execution",
            Some("EQ0501"),
        )?;

        let probe = PyModule::new(py, "eqiora_boundary_probe")?;
        probe.add_function(wrap_pyfunction!(boundary_panic, &probe)?)?;
        let contained = probe
            .getattr("boundary_panic")?
            .call0()
            .expect_err("the boundary probe must panic before conversion");
        assert!(
            !contained.to_string().contains(PANIC_PAYLOAD),
            "the Rust panic payload crossed into the Python exception"
        );
        assert_exception(
            module,
            py,
            contained,
            "InternalError",
            "internal",
            Some("EQ0002"),
        )?;

        assert_panic_hook_is_sanitized();

        Ok(())
    })
}

#[test]
fn panic_probe_child() {
    if std::env::var_os(PANIC_PROBE_CHILD).is_none() {
        return;
    }
    Python::initialize();
    Python::attach(|py| {
        let error = _eqiora::panic_boundary(py, || -> PyResult<()> { panic!("{PANIC_PAYLOAD}") })
            .expect_err("probe panic must become one Python error");
        assert!(!error.to_string().contains(PANIC_PAYLOAD));
    });
}

fn assert_panic_hook_is_sanitized() {
    let output = Command::new(std::env::current_exe().expect("current integration-test binary"))
        .args(["--exact", "panic_probe_child", "--nocapture"])
        .env(PANIC_PROBE_CHILD, "1")
        .env("RUST_BACKTRACE", "1")
        .output()
        .expect("panic containment subprocess must start");
    assert!(
        output.status.success(),
        "panic containment subprocess failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains(PANIC_PAYLOAD) && !stderr.contains("python_control_plane.rs"),
        "panic hook disclosed its payload or Rust location: {stderr}"
    );
}

fn model_bytes(model: &Bound<'_, PyAny>) -> PyResult<Vec<u8>> {
    model.call_method0("to_json")?.extract()
}

fn revision_number(model: &Bound<'_, PyAny>) -> PyResult<u64> {
    model.getattr("revision")?.getattr("number")?.extract()
}

fn assert_exception(
    module: &Bound<'_, PyModule>,
    py: Python<'_>,
    error: PyErr,
    class: &str,
    category: &str,
    code: Option<&str>,
) -> PyResult<()> {
    assert!(
        error.is_instance(py, &module.getattr(class)?),
        "expected {class}, received {error}"
    );
    let value = error.value(py);
    assert_eq!(value.getattr("category")?.extract::<String>()?, category);
    let diagnostics = value.getattr("diagnostics")?;
    assert!(diagnostics.len()? > 0, "structured diagnostics were lost");
    if let Some(code) = code {
        assert_eq!(
            diagnostics
                .get_item(0)?
                .getattr("code")?
                .extract::<String>()?,
            code
        );
    }
    Ok(())
}
