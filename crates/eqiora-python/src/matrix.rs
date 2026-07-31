//! Private immutable native-to-NumPy matrix transfer.

use std::mem;
use std::sync::Mutex;

use numpy::ndarray::Array2;
use numpy::{Element, IntoPyArray, PyArray2, PyArrayMethods};
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;

enum MatrixStorage<T: Element> {
    Native(Vec<T>),
    Materializing,
    Numpy(Py<PyArray2<T>>),
}

/// One native matrix whose first NumPy projection becomes its immutable owner.
pub(crate) struct ReadOnlyMatrix<T: Element> {
    rows: usize,
    columns: usize,
    storage: Mutex<MatrixStorage<T>>,
}

impl<T: Element> ReadOnlyMatrix<T> {
    pub(crate) fn new(rows: usize, columns: usize, values: Vec<T>) -> Self {
        debug_assert_eq!(values.len(), rows * columns);
        Self {
            rows,
            columns,
            storage: Mutex::new(MatrixStorage::Native(values)),
        }
    }

    pub(crate) fn numpy(&self, py: Python<'_>) -> PyResult<Py<PyArray2<T>>> {
        let values = {
            let mut storage = self
                .storage
                .lock()
                .map_err(|_| PyRuntimeError::new_err("matrix storage lock is poisoned"))?;
            match &*storage {
                MatrixStorage::Numpy(array) => return Ok(array.clone_ref(py)),
                MatrixStorage::Materializing => {
                    return Err(PyRuntimeError::new_err(
                        "matrix NumPy materialization is already in progress",
                    ));
                }
                MatrixStorage::Native(_) => {}
            }
            let MatrixStorage::Native(values) =
                mem::replace(&mut *storage, MatrixStorage::Materializing)
            else {
                return Err(PyRuntimeError::new_err(
                    "matrix storage changed before NumPy materialization",
                ));
            };
            values
        };

        // NumPy import hooks can execute Python. Never hold the storage lock
        // across allocation, and make the transferred native allocation
        // immutable before publishing it.
        let native = Array2::from_shape_vec((self.rows, self.columns), values)
            .expect("validated native matrix shape");
        let matrix = native.into_pyarray(py);
        drop(matrix.readwrite().make_nonwriteable());
        let owned = matrix.unbind();

        let mut storage = self
            .storage
            .lock()
            .map_err(|_| PyRuntimeError::new_err("matrix storage lock is poisoned"))?;
        if !matches!(*storage, MatrixStorage::Materializing) {
            return Err(PyRuntimeError::new_err(
                "matrix storage changed during NumPy materialization",
            ));
        }
        *storage = MatrixStorage::Numpy(owned.clone_ref(py));
        Ok(owned)
    }
}
