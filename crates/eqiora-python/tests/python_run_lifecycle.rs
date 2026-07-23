use std::path::Path;

use pyo3::ffi::c_str;
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyDict, PyDictMethods, PyModule};

const DECAY: &str = r#"
model decay {
  field x: 1 = 1;
  parameter rate: 1 / s = 1;
  relation flow continuous {
    derivative(x) + rate * x = 0;
  }
}
"#;

const OVERDETERMINED: &str = r#"
model overdetermined {
  field x: 1 = 1;
  relation first continuous { x = 0; }
  relation second continuous { x = 0; }
}
"#;

#[test]
fn python_reference_run_lifecycle_is_typed_bounded_and_fail_closed() -> PyResult<()> {
    Python::initialize();
    Python::attach(|py| {
        let module = public_module(py)?;
        let model = module.getattr("compile")?.call1((DECAY,))?;

        let completed = submit(&module, &model, 0.2, 0.1)?;
        assert_eq!(
            completed.getattr("model_digest")?.extract::<String>()?,
            model.getattr("digest")?.extract::<String>()?
        );
        assert_eq!(
            completed.getattr("model_id")?.extract::<String>()?,
            model.getattr("model_id")?.extract::<String>()?
        );
        let first = completed.call_method0("result")?;
        let second = completed.call_method0("result")?;
        assert!(
            first.is(&second),
            "a completed Result must materialize once"
        );
        assert_eq!(
            completed.getattr("status")?.repr()?.to_str()?,
            "RunStatus.Completed"
        );
        assert!(completed.getattr("done")?.extract::<bool>()?);
        assert_eq!(completed.getattr("history")?.len()?, 5);
        assert_eq!(
            first.getattr("model_digest")?.extract::<String>()?,
            model.getattr("digest")?.extract::<String>()?
        );
        assert_eq!(
            first.getattr("plan_key")?.extract::<String>()?,
            completed.getattr("plan_key")?.extract::<String>()?
        );

        let zero = submit(&module, &model, 0.0, 0.1)?;
        zero.call_method0("result")?;
        assert!(zero.getattr("progress")?.is_none());

        let cancelled = submit(&module, &model, 1.0, 1.0e-6)?;
        assert!(cancelled.call_method0("cancel")?.extract::<bool>()?);
        assert!(!cancelled.call_method0("cancel")?.extract::<bool>()?);
        let error = cancelled
            .call_method0("result")
            .expect_err("a cancellation boundary must not publish a Result");
        assert!(error.is_instance(py, &module.getattr("CancellationError")?));
        let diagnostic = error.value(py).getattr("diagnostics")?.get_item(0)?;
        assert_eq!(diagnostic.getattr("code")?.extract::<String>()?, "EQ0506");
        assert_eq!(
            cancelled.getattr("status")?.repr()?.to_str()?,
            "RunStatus.Cancelled"
        );
        let cancellation = cancelled.getattr("cancellation")?;
        assert!(!cancellation.is_none());
        assert_eq!(
            cancellation.getattr("plan_key")?.extract::<String>()?,
            cancelled.getattr("plan_key")?.extract::<String>()?
        );
        assert_eq!(
            cancellation
                .getattr("progress")?
                .getattr("model_time")?
                .extract::<f64>()?,
            cancelled
                .getattr("progress")?
                .getattr("model_time")?
                .extract::<f64>()?
        );

        let invalid = module.getattr("compile")?.call1((OVERDETERMINED,))?;
        let failed = submit(&module, &invalid, 0.1, 0.1)?;
        let error = failed
            .call_method0("result")
            .expect_err("an admitted but nonsquare system must fail during execution");
        assert!(error.is_instance(py, &module.getattr("ExecutionError")?));
        assert_eq!(
            error
                .value(py)
                .getattr("diagnostics")?
                .get_item(0)?
                .getattr("code")?
                .extract::<String>()?,
            "EQ0503"
        );
        assert_eq!(
            failed.getattr("status")?.repr()?.to_str()?,
            "RunStatus.Failed"
        );
        assert!(!failed.call_method0("cancel")?.extract::<bool>()?);

        let locals = PyDict::new(py);
        locals.set_item("eqiora", &module)?;
        locals.set_item("decay_source", DECAY)?;
        py.run(
            c_str!(
                r#"
import asyncio
import threading
import time

# The observer must see at least two distinct, throttled publications while
# result() owns the calling thread. Without GIL release it cannot do so.
gil_model = eqiora.compile(decay_source)
gil_run = eqiora.submit(gil_model, end_time=0.2, max_step=2.0e-6)
gil_ready = threading.Event()
gil_publications = []

def observe_gil_run():
    last_steps = None
    gil_ready.set()
    while not gil_run.done:
        snapshot = gil_run.progress
        if snapshot is not None and snapshot.accepted_steps != last_steps:
            gil_publications.append((time.monotonic(), snapshot.accepted_steps))
            last_steps = snapshot.accepted_steps
        time.sleep(0.001)

gil_observer = threading.Thread(target=observe_gil_run)
gil_observer.start()
assert gil_ready.wait(timeout=1.0)
assert not gil_run.done
gil_run.result()
gil_observer.join(timeout=2.0)
assert not gil_observer.is_alive()
assert len(gil_publications) >= 2
assert all(
    right[0] - left[0] >= 0.08
    for left, right in zip(gil_publications, gil_publications[1:])
)

async def exercise_awaitable():
    completed = eqiora.submit(
        eqiora.compile(decay_source), end_time=0.2, max_step=0.1
    )
    result = await completed
    assert result is completed.result()

    live = eqiora.submit(
        eqiora.compile(decay_source), end_time=1.0, max_step=1.0e-6
    )
    task = asyncio.ensure_future(live)
    await asyncio.sleep(0)
    task.cancel()
    try:
        await task
    except asyncio.CancelledError:
        pass
    else:
        raise AssertionError("the asyncio Task must observe its own cancellation")
    assert live.status not in (
        eqiora.RunStatus.Cancelling,
        eqiora.RunStatus.Cancelled,
    )
    assert live.cancel()
    try:
        live.result()
    except eqiora.CancellationError:
        pass
    else:
        raise AssertionError("explicit native cancellation must remain required")

asyncio.run(exercise_awaitable())
"#
            ),
            Some(&locals),
            Some(&locals),
        )?;

        Ok(())
    })
}

fn public_module(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    let native = pyo3::wrap_pymodule!(_eqiora::_eqiora)(py);
    let package_directory = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../bindings/python/python/eqiora")
        .canonicalize()?;
    let locals = PyDict::new(py);
    locals.set_item("native", native.bind(py))?;
    locals.set_item("package_directory", package_directory.to_string_lossy())?;
    py.run(
        c_str!(
            r#"
import importlib.util
import pathlib
import sys

package_path = pathlib.Path(package_directory)
spec = importlib.util.spec_from_file_location(
    "eqiora",
    package_path / "__init__.py",
    submodule_search_locations=[str(package_path)],
)
assert spec is not None and spec.loader is not None
package = importlib.util.module_from_spec(spec)
sys.modules["eqiora"] = package
sys.modules["eqiora._eqiora"] = native
spec.loader.exec_module(package)
"#
        ),
        None,
        Some(&locals),
    )?;
    Ok(locals
        .get_item("package")?
        .expect("the package loader must bind eqiora")
        .cast_into::<PyModule>()?)
}

fn submit<'py>(
    module: &Bound<'py, PyModule>,
    model: &Bound<'py, PyAny>,
    end_time: f64,
    max_step: f64,
) -> PyResult<Bound<'py, PyAny>> {
    let kwargs = PyDict::new(module.py());
    kwargs.set_item("end_time", end_time)?;
    kwargs.set_item("max_step", max_step)?;
    module.getattr("submit")?.call((model,), Some(&kwargs))
}
