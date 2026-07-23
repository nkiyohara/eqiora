//! The complete unsafe cuBLAS boundary.
//!
//! Only the level-1 reductions/vector updates and diagonal band action used by
//! Eqiora's CUDA Krylov implementation are loaded. The owning library outlives
//! every copied symbol and handle.

use std::ffi::c_void;
use std::fmt;
use std::mem::MaybeUninit;

use cudarc::driver;
use libloading::Library;

type Status = u32;
type Handle = *mut c_void;

const SUCCESS: Status = 0;

#[repr(u32)]
#[derive(Debug, Clone, Copy)]
enum Operation {
    NonTranspose = 0,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy)]
enum PointerMode {
    Host = 0,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy)]
enum AtomicsMode {
    NotAllowed = 0,
}

type Create = unsafe extern "C" fn(*mut Handle) -> Status;
type Destroy = unsafe extern "C" fn(Handle) -> Status;
type SetStream = unsafe extern "C" fn(Handle, *mut c_void) -> Status;
type GetVersion = unsafe extern "C" fn(Handle, *mut i32) -> Status;
type SetPointerMode = unsafe extern "C" fn(Handle, PointerMode) -> Status;
type SetAtomicsMode = unsafe extern "C" fn(Handle, AtomicsMode) -> Status;
type CopyVector = unsafe extern "C" fn(Handle, i32, *const f64, i32, *mut f64, i32) -> Status;
type Dot = unsafe extern "C" fn(Handle, i32, *const f64, i32, *const f64, i32, *mut f64) -> Status;
type Norm = unsafe extern "C" fn(Handle, i32, *const f64, i32, *mut f64) -> Status;
type Axpy = unsafe extern "C" fn(Handle, i32, *const f64, *const f64, i32, *mut f64, i32) -> Status;
type Scale = unsafe extern "C" fn(Handle, i32, *const f64, *mut f64, i32) -> Status;
type GeneralBandMatrixVector = unsafe extern "C" fn(
    Handle,
    Operation,
    i32,
    i32,
    i32,
    i32,
    *const f64,
    *const f64,
    i32,
    *const f64,
    i32,
    *const f64,
    *mut f64,
    i32,
) -> Status;

/// Private FFI failure converted to a stable Eqiora diagnostic at the adapter
/// boundary.
pub(crate) struct BlasError(String);

impl fmt::Debug for BlasError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_tuple("BlasError").field(&self.0).finish()
    }
}

struct Functions {
    create: Create,
    destroy: Destroy,
    set_stream: SetStream,
    get_version: GetVersion,
    set_pointer_mode: SetPointerMode,
    set_atomics_mode: SetAtomicsMode,
    copy: CopyVector,
    dot: Dot,
    norm: Norm,
    axpy: Axpy,
    scale: Scale,
    general_band_matrix_vector: GeneralBandMatrixVector,
}

struct BlasLibrary {
    functions: Functions,
    _library: Library,
}

impl fmt::Debug for BlasLibrary {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("BlasLibrary { libcublas.so.12 }")
    }
}

impl BlasLibrary {
    fn load() -> Result<Self, BlasError> {
        // SAFETY: `library` is stored beside every copied symbol and remains
        // live until after its handle is destroyed.
        let library = unsafe { Library::new("libcublas.so.12") }
            .map_err(|error| BlasError(format!("could not load libcublas.so.12: {error}")))?;
        let functions = Functions {
            create: load(&library, b"cublasCreate_v2\0")?,
            destroy: load(&library, b"cublasDestroy_v2\0")?,
            set_stream: load(&library, b"cublasSetStream_v2\0")?,
            get_version: load(&library, b"cublasGetVersion_v2\0")?,
            set_pointer_mode: load(&library, b"cublasSetPointerMode_v2\0")?,
            set_atomics_mode: load(&library, b"cublasSetAtomicsMode\0")?,
            copy: load(&library, b"cublasDcopy_v2\0")?,
            dot: load(&library, b"cublasDdot_v2\0")?,
            norm: load(&library, b"cublasDnrm2_v2\0")?,
            axpy: load(&library, b"cublasDaxpy_v2\0")?,
            scale: load(&library, b"cublasDscal_v2\0")?,
            general_band_matrix_vector: load(&library, b"cublasDgbmv_v2\0")?,
        };
        Ok(Self {
            functions,
            _library: library,
        })
    }
}

pub(crate) fn probe_cublas() -> Result<(), BlasError> {
    BlasLibrary::load().map(|_| ())
}

fn load<T: Copy>(library: &Library, symbol: &'static [u8]) -> Result<T, BlasError> {
    // SAFETY: `T` exactly matches the documented CUDA 12 cuBLAS C signature,
    // and the owning library outlives the copied function pointer.
    unsafe { library.get::<T>(symbol) }
        .map(|value| *value)
        .map_err(|error| {
            let name = std::str::from_utf8(symbol)
                .unwrap_or("<non-UTF8 symbol>")
                .trim_end_matches('\0');
            BlasError(format!("missing cuBLAS symbol {name}: {error}"))
        })
}

fn status(operation: &str, value: Status) -> Result<(), BlasError> {
    if value == SUCCESS {
        Ok(())
    } else {
        Err(BlasError(format!(
            "{operation} returned cuBLAS status {value}"
        )))
    }
}

/// One cuBLAS handle fixed to a stream, host scalar pointers, and disabled
/// atomic routines.
#[derive(Debug)]
pub(crate) struct BlasHandle {
    library: BlasLibrary,
    raw: Handle,
}

impl BlasHandle {
    pub(crate) fn new(stream: driver::sys::CUstream) -> Result<Self, BlasError> {
        let library = BlasLibrary::load()?;
        let mut raw = MaybeUninit::uninit();
        // SAFETY: `raw` is a valid out pointer and the library remains live.
        unsafe {
            status(
                "cublasCreate_v2",
                (library.functions.create)(raw.as_mut_ptr()),
            )?
        };
        // SAFETY: successful creation initialized one live handle.
        let raw = unsafe { raw.assume_init() };
        let configure = || {
            // SAFETY: the handle and same-context stream are live.
            unsafe {
                status(
                    "cublasSetStream_v2",
                    (library.functions.set_stream)(raw, stream.cast()),
                )?;
                status(
                    "cublasSetPointerMode_v2",
                    (library.functions.set_pointer_mode)(raw, PointerMode::Host),
                )?;
                status(
                    "cublasSetAtomicsMode",
                    (library.functions.set_atomics_mode)(raw, AtomicsMode::NotAllowed),
                )
            }
        };
        if let Err(error) = configure() {
            // SAFETY: creation succeeded and ownership has not escaped.
            let _ = unsafe { (library.functions.destroy)(raw) };
            return Err(error);
        }
        Ok(Self { library, raw })
    }

    pub(crate) fn version(&self) -> Result<i32, BlasError> {
        let mut version = MaybeUninit::uninit();
        // SAFETY: the live handle writes one `c_int` to the valid out pointer.
        unsafe {
            status(
                "cublasGetVersion_v2",
                (self.library.functions.get_version)(self.raw, version.as_mut_ptr()),
            )?;
            Ok(version.assume_init())
        }
    }

    pub(crate) fn copy(
        &self,
        elements: i32,
        input: driver::sys::CUdeviceptr,
        output: driver::sys::CUdeviceptr,
    ) -> Result<(), BlasError> {
        // SAFETY: both pointers name live, non-overlapping `elements`-long
        // `f64` device vectors on this handle's context.
        unsafe {
            status(
                "cublasDcopy_v2",
                (self.library.functions.copy)(
                    self.raw,
                    elements,
                    input as *const f64,
                    1,
                    output as *mut f64,
                    1,
                ),
            )
        }
    }

    pub(crate) fn dot(
        &self,
        elements: i32,
        left: driver::sys::CUdeviceptr,
        right: driver::sys::CUdeviceptr,
    ) -> Result<f64, BlasError> {
        let mut result = MaybeUninit::uninit();
        // SAFETY: both pointers name live `elements`-long `f64` device
        // vectors. Host pointer mode makes `result` an initialized host scalar
        // before this call returns.
        unsafe {
            status(
                "cublasDdot_v2",
                (self.library.functions.dot)(
                    self.raw,
                    elements,
                    left as *const f64,
                    1,
                    right as *const f64,
                    1,
                    result.as_mut_ptr(),
                ),
            )?;
            Ok(result.assume_init())
        }
    }

    pub(crate) fn norm(
        &self,
        elements: i32,
        values: driver::sys::CUdeviceptr,
    ) -> Result<f64, BlasError> {
        let mut result = MaybeUninit::uninit();
        // SAFETY: `values` names a live `elements`-long `f64` device vector;
        // host pointer mode initializes `result` before return.
        unsafe {
            status(
                "cublasDnrm2_v2",
                (self.library.functions.norm)(
                    self.raw,
                    elements,
                    values as *const f64,
                    1,
                    result.as_mut_ptr(),
                ),
            )?;
            Ok(result.assume_init())
        }
    }

    pub(crate) fn axpy(
        &self,
        elements: i32,
        alpha: f64,
        input: driver::sys::CUdeviceptr,
        output: driver::sys::CUdeviceptr,
    ) -> Result<(), BlasError> {
        // SAFETY: the pointers name live `elements`-long `f64` vectors and
        // host pointer mode reads `alpha` during the call.
        unsafe {
            status(
                "cublasDaxpy_v2",
                (self.library.functions.axpy)(
                    self.raw,
                    elements,
                    &alpha,
                    input as *const f64,
                    1,
                    output as *mut f64,
                    1,
                ),
            )
        }
    }

    pub(crate) fn scale(
        &self,
        elements: i32,
        alpha: f64,
        values: driver::sys::CUdeviceptr,
    ) -> Result<(), BlasError> {
        // SAFETY: `values` names a live `elements`-long `f64` device vector
        // and host pointer mode reads `alpha` during the call.
        unsafe {
            status(
                "cublasDscal_v2",
                (self.library.functions.scale)(self.raw, elements, &alpha, values as *mut f64, 1),
            )
        }
    }

    pub(crate) fn diagonal_multiply(
        &self,
        elements: i32,
        diagonal: driver::sys::CUdeviceptr,
        input: driver::sys::CUdeviceptr,
        output: driver::sys::CUdeviceptr,
    ) -> Result<(), BlasError> {
        let alpha = 1.0;
        let beta = 0.0;
        // SAFETY: the zero-band matrix is an `elements`-long `f64` diagonal
        // with `lda = 1`; input/output are equally sized live device vectors.
        unsafe {
            status(
                "cublasDgbmv_v2",
                (self.library.functions.general_band_matrix_vector)(
                    self.raw,
                    Operation::NonTranspose,
                    elements,
                    elements,
                    0,
                    0,
                    &alpha,
                    diagonal as *const f64,
                    1,
                    input as *const f64,
                    1,
                    &beta,
                    output as *mut f64,
                    1,
                ),
            )
        }
    }
}

impl Drop for BlasHandle {
    fn drop(&mut self) {
        let raw = std::mem::replace(&mut self.raw, std::ptr::null_mut());
        if !raw.is_null() {
            // SAFETY: ownership is unique and `raw` is replaced first.
            let _ = unsafe { (self.library.functions.destroy)(raw) };
        }
    }
}
