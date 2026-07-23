//! Bounded Python data-plane projection for immutable CPU result buffers.

use std::mem;
use std::sync::Mutex;

use numpy::{IntoPyArray, PyArray1, PyArrayDescrMethods, PyArrayMethods, PyUntypedArrayMethods};
use pyo3::exceptions::{PyBufferError, PyRuntimeError};
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyDict, PyModule};

const DLPACK_CPU_DEVICE_TYPE: i32 = 1;
const DLPACK_CPU_DEVICE_ID: i32 = 0;
const DLPACK_MAJOR_VERSION: u32 = 1;
const DLPACK_MINOR_VERSION: u32 = 0;

/// One immutable, dense, one-dimensional CPU `float64` buffer.
///
/// The native result allocation is transferred into an opaque Python owner.
/// NumPy can therefore borrow it without copying while retaining its lifetime
/// independently of the originating Result. DLPack exports use a snapshot:
/// its read-only bit is advisory for consumers and cannot protect canonical
/// Result evidence by itself.
#[pyclass(name = "Array", module = "eqiora._eqiora", frozen)]
pub(crate) struct PyArrayBuffer {
    len: usize,
    storage: Mutex<ArrayStorage>,
}

enum ArrayStorage {
    Native(Vec<f64>),
    Materializing,
    Numpy(Py<PyArray1<f64>>),
}

impl PyArrayBuffer {
    pub(crate) fn from_owned_result(py: Python<'_>, values: Vec<f64>) -> PyResult<Py<Self>> {
        let len = values.len();
        Py::new(
            py,
            Self {
                len,
                storage: Mutex::new(ArrayStorage::Native(values)),
            },
        )
    }

    fn numpy_array(&self, py: Python<'_>) -> PyResult<Py<PyArray1<f64>>> {
        let values = {
            let mut storage = self
                .storage
                .lock()
                .map_err(|_| PyRuntimeError::new_err("Array storage lock is poisoned"))?;
            match &*storage {
                ArrayStorage::Numpy(array) => return Ok(array.clone_ref(py)),
                ArrayStorage::Materializing => {
                    return Err(PyRuntimeError::new_err(
                        "Array NumPy materialization is already in progress",
                    ));
                }
                ArrayStorage::Native(_) => {}
            }
            let ArrayStorage::Native(values) =
                mem::replace(&mut *storage, ArrayStorage::Materializing)
            else {
                return Err(PyRuntimeError::new_err(
                    "Array storage changed before NumPy materialization",
                ));
            };
            values
        };

        // NumPy initialization may execute Python import hooks. Do not hold
        // the storage lock across it: re-entry must fail, never self-deadlock.
        let array = values.into_pyarray(py);
        let owned = array.clone().unbind();
        drop(array.readwrite().make_nonwriteable());

        let mut storage = self
            .storage
            .lock()
            .map_err(|_| PyRuntimeError::new_err("Array storage lock is poisoned"))?;
        if !matches!(*storage, ArrayStorage::Materializing) {
            return Err(PyRuntimeError::new_err(
                "Array storage changed during NumPy materialization",
            ));
        }
        *storage = ArrayStorage::Numpy(owned.clone_ref(py));
        Ok(owned)
    }

    pub(crate) const fn len(&self) -> usize {
        self.len
    }

    pub(crate) fn snapshot(&self, py: Python<'_>) -> PyResult<Vec<f64>> {
        let numpy = {
            let storage = self
                .storage
                .lock()
                .map_err(|_| PyRuntimeError::new_err("Array storage lock is poisoned"))?;
            match &*storage {
                ArrayStorage::Native(values) => return Ok(values.clone()),
                ArrayStorage::Materializing => {
                    return Err(PyRuntimeError::new_err(
                        "Array NumPy materialization is already in progress",
                    ));
                }
                ArrayStorage::Numpy(array) => array.clone_ref(py),
            }
        };
        Ok(numpy.bind(py).readonly().as_slice()?.to_vec())
    }
}

/// Admit one exact CPU/native-`f64` rank-one input and copy it into
/// Eqiora-owned staging before native execution detaches from Python.
pub(crate) fn stage_f64_input(
    py: Python<'_>,
    value: &Bound<'_, PyAny>,
    expected_len: usize,
    label: &str,
) -> PyResult<Vec<f64>> {
    let values = if let Ok(buffer) = value.extract::<PyRef<'_, PyArrayBuffer>>() {
        if buffer.len() != expected_len {
            return Err(PyBufferError::new_err(format!(
                "{label} must contain exactly {expected_len} values, received {}",
                buffer.len(),
            )));
        }
        buffer.snapshot(py)?
    } else if let Ok(array) = value.cast::<PyArray1<f64>>() {
        copy_exact_f64_array(array, expected_len, label)?
    } else {
        stage_f64_dlpack_input(py, value, expected_len, label)?
    };
    if values.iter().any(|value| !value.is_finite()) {
        return Err(PyBufferError::new_err(format!(
            "{label} must contain finite float64 values",
        )));
    }
    Ok(values)
}

fn stage_f64_dlpack_input(
    py: Python<'_>,
    value: &Bound<'_, PyAny>,
    expected_len: usize,
    label: &str,
) -> PyResult<Vec<f64>> {
    let has_capsule = value.hasattr("__dlpack__")?;
    let has_device = value.hasattr("__dlpack_device__")?;
    if !has_capsule || !has_device {
        return Err(PyBufferError::new_err(format!(
            "{label} must be an Eqiora Array, an exact rank-one NumPy float64 array, or a complete DLPack producer",
        )));
    }

    let reported_device = value.call_method0("__dlpack_device__")?;
    let device = reported_device.extract::<(i32, i32)>().map_err(|_| {
        PyBufferError::new_err(format!(
            "{label} DLPack producer must report one integer device pair",
        ))
    })?;
    if device != (DLPACK_CPU_DEVICE_TYPE, DLPACK_CPU_DEVICE_ID) {
        return Err(PyBufferError::new_err(format!(
            "{label} DLPack input must already reside on CPU device 0",
        )));
    }

    // NumPy owns capsule version negotiation, exactly-once consumption, and
    // the producer deleter. copy=False forbids a hidden producer transfer;
    // Eqiora performs the one documented staging copy only after validation.
    let kwargs = PyDict::new(py);
    kwargs.set_item("device", "cpu")?;
    kwargs.set_item("copy", false)?;
    let imported = py
        .import("numpy")?
        .getattr("from_dlpack")?
        .call((value,), Some(&kwargs))?;
    let array = imported.cast::<PyArray1<f64>>().map_err(|_| {
        PyBufferError::new_err(format!(
            "{label} DLPack input must be an exact rank-one float64 array",
        ))
    })?;
    copy_exact_f64_array(array, expected_len, label)
}

fn copy_exact_f64_array(
    array: &Bound<'_, PyArray1<f64>>,
    expected_len: usize,
    label: &str,
) -> PyResult<Vec<f64>> {
    if !array.is_c_contiguous()
        || !array.is_aligned()
        || array.dtype().is_native_byteorder() != Some(true)
    {
        return Err(PyBufferError::new_err(format!(
            "{label} must be C-contiguous, aligned, and native-endian",
        )));
    }
    if array.len() != expected_len {
        return Err(PyBufferError::new_err(format!(
            "{label} must contain exactly {expected_len} values, received {}",
            array.len(),
        )));
    }
    let readonly = array.readonly();
    let borrowed = readonly
        .as_slice()
        .map_err(|_| PyBufferError::new_err(format!("{label} cannot be borrowed safely")))?;
    Ok(borrowed.to_vec())
}

#[pymethods]
impl PyArrayBuffer {
    /// Project this immutable host buffer into NumPy.
    ///
    /// `False` and `None` return the exact read-only zero-copy array. `True`
    /// creates an independent writable NumPy copy.
    #[pyo3(signature = (*, copy=None))]
    fn numpy(&self, py: Python<'_>, copy: Option<bool>) -> PyResult<Py<PyArray1<f64>>> {
        if copy == Some(true) {
            Ok(self
                .numpy_array(py)?
                .bind(py)
                .call_method0("copy")?
                .extract::<Py<PyArray1<f64>>>()?)
        } else {
            self.numpy_array(py)
        }
    }

    /// DLPack device pair `(kDLCPU, 0)`.
    fn __dlpack_device__(&self) -> (i32, i32) {
        (DLPACK_CPU_DEVICE_TYPE, DLPACK_CPU_DEVICE_ID)
    }

    /// Export one fresh, versioned DLPack snapshot.
    ///
    /// DLPack read-only flags are advisory, so the immutable Result allocation
    /// is never shared through this protocol. `copy=False`, legacy capsules,
    /// streams, and non-CPU device requests fail rather than weakening that
    /// boundary. NumPy owns capsule consumption and its exactly-once deleter.
    #[pyo3(signature = (*, stream=None, max_version=None, dl_device=None, copy=None))]
    fn __dlpack__(
        &self,
        py: Python<'_>,
        stream: Option<Py<PyAny>>,
        max_version: Option<(u32, u32)>,
        dl_device: Option<(i32, i32)>,
        copy: Option<bool>,
    ) -> PyResult<Py<PyAny>> {
        if stream.is_some() {
            return Err(PyBufferError::new_err(
                "CPU Array DLPack export accepts stream=None only",
            ));
        }
        let Some((major, _minor)) = max_version else {
            return Err(PyBufferError::new_err(
                "Array exports versioned DLPack 1.x capsules only",
            ));
        };
        if major != DLPACK_MAJOR_VERSION {
            return Err(PyBufferError::new_err(format!(
                "Array supports DLPack major version {DLPACK_MAJOR_VERSION}, not {major}",
            )));
        }
        if let Some(device) = dl_device
            && device != (DLPACK_CPU_DEVICE_TYPE, DLPACK_CPU_DEVICE_ID)
        {
            return Err(PyBufferError::new_err(
                "Array DLPack export cannot transfer away from CPU device 0",
            ));
        }
        if copy == Some(false) {
            return Err(PyBufferError::new_err(
                "immutable Result evidence requires a DLPack snapshot; copy=False is unsupported",
            ));
        }

        let kwargs = PyDict::new(py);
        kwargs.set_item("max_version", (DLPACK_MAJOR_VERSION, DLPACK_MINOR_VERSION))?;
        kwargs.set_item("copy", true)?;
        Ok(self
            .numpy_array(py)?
            .bind(py)
            .call_method("__dlpack__", (), Some(&kwargs))?
            .unbind())
    }

    /// Device on which this bounded array resides.
    #[getter]
    fn device(&self) -> &'static str {
        "cpu"
    }

    /// Exact CPU device ordinal.
    #[getter]
    const fn device_id(&self) -> u32 {
        DLPACK_CPU_DEVICE_ID as u32
    }

    /// Exact scalar dtype.
    #[getter]
    fn dtype(&self) -> &'static str {
        "float64"
    }

    /// Native byte order of the stored `float64` values.
    #[getter]
    const fn byte_order(&self) -> &'static str {
        if cfg!(target_endian = "little") {
            "little"
        } else {
            "big"
        }
    }

    /// Logical array shape.
    #[getter]
    const fn shape(&self) -> (usize,) {
        (self.len,)
    }

    /// Byte strides in logical-axis order.
    #[getter]
    const fn strides(&self) -> (isize,) {
        (mem::size_of::<f64>() as isize,)
    }

    /// This producer slice admits C-contiguous storage only.
    #[getter]
    const fn c_contiguous(&self) -> bool {
        true
    }

    /// The native allocation satisfies `float64` alignment.
    #[getter]
    const fn aligned(&self) -> bool {
        true
    }

    /// Canonical Result storage is immutable.
    #[getter]
    const fn readonly(&self) -> bool {
        true
    }

    /// The Array owns its lifetime anchor rather than borrowing a Result.
    #[getter]
    fn ownership(&self) -> &'static str {
        "owned"
    }

    /// Whether construction from the native Result copied its values.
    #[getter]
    const fn origin_copy_occurred(&self) -> bool {
        false
    }

    const fn __len__(&self) -> usize {
        self.len()
    }

    fn __repr__(&self) -> String {
        format!(
            "Array(shape=({},), dtype='float64', device='cpu:0', readonly=True)",
            self.len
        )
    }
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyArrayBuffer>()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descriptor_constants_match_the_native_allocation_contract() {
        assert_eq!(mem::size_of::<f64>(), 8);
        assert_eq!(mem::align_of::<f64>(), 8);
        assert_eq!(DLPACK_CPU_DEVICE_TYPE, 1);
        assert_eq!(DLPACK_CPU_DEVICE_ID, 0);
    }
}
