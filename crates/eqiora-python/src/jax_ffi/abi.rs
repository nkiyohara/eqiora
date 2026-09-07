//! Allowlisted XLA typed-FFI C ABI for the exact JAX/JAXLIB adapter.
//!
//! Layouts and constants are derived from Apache-2.0
//! `xla/ffi/api/c_api.h` shipped by JAXLIB 0.11.0 (XLA FFI API 0.3).
//! The installed-wheel gate compiles that exact header and compares every
//! layout consumed here. No JAX or C++ runtime is linked into the base wheel.

use std::ffi::{CString, c_char, c_int, c_void};
use std::mem::{offset_of, size_of};
use std::ptr;
use std::slice;

use pyo3::prelude::*;
use pyo3::types::{PyAny, PyDict};

use super::batch::{self, Layout};
use super::kernel::{self, Action, ActionResult, FailureKind, HandlerFailure};
use super::{JVP_TARGET, PRIMAL_TARGET, VJP_TARGET};
use crate::error::catch_native_panic;

const XLA_FFI_API_MAJOR: c_int = 0;
const XLA_FFI_API_MINOR: c_int = 3;
const XLA_FFI_EXTENSION_METADATA: c_int = 1;
const XLA_FFI_EXECUTE: c_int = 3;
const XLA_FFI_BUFFER: c_int = 1;
const XLA_FFI_ATTR_STRING: c_int = 4;
const XLA_FFI_F64: c_int = 12;

const FFI_INVALID_ARGUMENT: c_int = 3;
const FFI_NOT_FOUND: c_int = 5;
const FFI_FAILED_PRECONDITION: c_int = 9;
const FFI_INTERNAL: c_int = 13;
const FFI_DATA_LOSS: c_int = 15;

pub(super) fn target_capsules(py: Python<'_>) -> PyResult<Bound<'_, PyDict>> {
    let targets = PyDict::new(py);
    targets.set_item(PRIMAL_TARGET, capsule(py, primal_handler)?)?;
    targets.set_item(JVP_TARGET, capsule(py, jvp_handler)?)?;
    targets.set_item(VJP_TARGET, capsule(py, vjp_handler)?)?;
    Ok(targets)
}

pub(super) fn layout(py: Python<'_>) -> PyResult<Bound<'_, PyDict>> {
    let layout = PyDict::new(py);
    layout.set_item("api_major", XLA_FFI_API_MAJOR)?;
    layout.set_item("api_minor", XLA_FFI_API_MINOR)?;
    layout.set_item("extension_metadata", XLA_FFI_EXTENSION_METADATA)?;
    layout.set_item("execution_stage_execute", XLA_FFI_EXECUTE)?;
    layout.set_item("arg_type_buffer", XLA_FFI_BUFFER)?;
    layout.set_item("attr_type_string", XLA_FFI_ATTR_STRING)?;
    layout.set_item("data_type_f64", XLA_FFI_F64)?;
    layout.set_item("error_invalid_argument", FFI_INVALID_ARGUMENT)?;
    layout.set_item("error_not_found", FFI_NOT_FOUND)?;
    layout.set_item("error_failed_precondition", FFI_FAILED_PRECONDITION)?;
    layout.set_item("error_internal", FFI_INTERNAL)?;
    layout.set_item("error_data_loss", FFI_DATA_LOSS)?;
    layout.set_item("extension_base_size", size_of::<FfiExtensionBase>())?;
    layout.set_item("api_version_size", size_of::<FfiApiVersion>())?;
    layout.set_item("error_create_args_size", size_of::<FfiErrorCreateArgs>())?;
    layout.set_item("buffer_size", size_of::<FfiBuffer>())?;
    layout.set_item("args_size", size_of::<FfiArgs>())?;
    layout.set_item("rets_size", size_of::<FfiRets>())?;
    layout.set_item("byte_span_size", size_of::<FfiByteSpan>())?;
    layout.set_item("attrs_size", size_of::<FfiAttrs>())?;
    layout.set_item("call_frame_size", size_of::<FfiCallFrame>())?;
    layout.set_item("call_frame_attrs_offset", offset_of!(FfiCallFrame, attrs))?;
    layout.set_item("call_frame_future_offset", offset_of!(FfiCallFrame, future))?;
    layout.set_item("call_frame_required_size", CALL_FRAME_REQUIRED_SIZE)?;
    layout.set_item("metadata_size", size_of::<FfiMetadata>())?;
    layout.set_item("metadata_traits_offset", offset_of!(FfiMetadata, traits))?;
    layout.set_item(
        "metadata_state_type_id_offset",
        offset_of!(FfiMetadata, state_type_id),
    )?;
    layout.set_item("metadata_required_size", METADATA_REQUIRED_SIZE)?;
    layout.set_item("metadata_extension_size", size_of::<FfiMetadataExtension>())?;
    layout.set_item(
        "api_error_create_offset",
        offset_of!(FfiApiPrefix, error_create),
    )?;
    layout.set_item(
        "api_error_destroy_offset",
        offset_of!(FfiApiPrefix, error_destroy),
    )?;
    layout.set_item(
        "api_device_ordinal_offset",
        offset_of!(FfiApiPrefix, device_ordinal_get),
    )?;
    layout.set_item(
        "api_device_ordinal_required_size",
        API_DEVICE_ORDINAL_REQUIRED_SIZE,
    )?;
    layout.set_item(
        "device_ordinal_args_size",
        size_of::<FfiDeviceOrdinalArgs>(),
    )?;
    layout.set_item(
        "device_ordinal_args_required_size",
        DEVICE_ORDINAL_ARGS_REQUIRED_SIZE,
    )?;
    layout.set_item("error_destroy_args_size", size_of::<FfiErrorDestroyArgs>())?;
    Ok(layout)
}

type FfiHandler = extern "C" fn(*mut FfiCallFrame) -> *mut c_void;

fn capsule(py: Python<'_>, handler: FfiHandler) -> PyResult<Bound<'_, PyAny>> {
    let address = handler as *const () as *mut c_void;
    // SAFETY: every handler is a process-lifetime `extern "C"` function and
    // JAX expects an unnamed capsule containing that exact function address.
    let capsule = unsafe { pyo3::ffi::PyCapsule_New(address, ptr::null(), None) };
    if capsule.is_null() {
        Err(PyErr::fetch(py))
    } else {
        // SAFETY: `PyCapsule_New` returned one owned, non-null Python reference.
        Ok(unsafe { Bound::from_owned_ptr(py, capsule) })
    }
}

extern "C" fn primal_handler(frame: *mut FfiCallFrame) -> *mut c_void {
    handler_boundary(frame, Action::Primal)
}

extern "C" fn jvp_handler(frame: *mut FfiCallFrame) -> *mut c_void {
    handler_boundary(frame, Action::Jvp)
}

extern "C" fn vjp_handler(frame: *mut FfiCallFrame) -> *mut c_void {
    handler_boundary(frame, Action::Vjp)
}

fn handler_boundary(frame: *mut FfiCallFrame, action: Action) -> *mut c_void {
    if frame.is_null() {
        return ptr::null_mut();
    }
    // The guarded native boundary suppresses panic payloads on XLA worker
    // threads and converts any unwind before it can cross the C ABI.
    match catch_native_panic(|| {
        // SAFETY: the XLA runtime owns the call frame for this synchronous
        // invocation. Every nested pointer is checked before it becomes a
        // Rust reference or slice.
        unsafe { execute_handler(&mut *frame, action) }
    }) {
        Ok(Ok(())) => ptr::null_mut(),
        Ok(Err(failure)) => ffi_error(frame, failure.kind, &failure.message),
        Err(diagnostic) => ffi_error(frame, FailureKind::Internal, &diagnostic.to_string()),
    }
}

unsafe fn execute_handler(frame: &mut FfiCallFrame, action: Action) -> Result<(), HandlerFailure> {
    validate_struct_size(
        "XLA_FFI_CallFrame",
        frame.struct_size,
        CALL_FRAME_REQUIRED_SIZE,
    )?;
    // SAFETY: the runtime owns the extension chain for this invocation; only
    // the common prefix is read before the extension kind is checked.
    if let Some(extension) = unsafe { frame.extension_start.as_ref() }
        && extension.kind == XLA_FFI_EXTENSION_METADATA
    {
        // SAFETY: the checked tag identifies a metadata extension, whose own
        // size is validated before its trailing pointer is read.
        return unsafe { populate_metadata(frame.extension_start.cast()) };
    }
    if frame.stage != XLA_FFI_EXECUTE {
        return Err(HandlerFailure::invalid(format!(
            "wrong XLA FFI stage: expected execute (3), got {}",
            frame.stage
        )));
    }
    // SAFETY: the runtime owns this invocation's API and execution context.
    unsafe { validate_device_ordinal(frame)? };
    validate_struct_size("XLA_FFI_Args", frame.args.struct_size, size_of::<FfiArgs>())?;
    validate_struct_size("XLA_FFI_Rets", frame.rets.struct_size, size_of::<FfiRets>())?;
    validate_struct_size(
        "XLA_FFI_Attrs",
        frame.attrs.struct_size,
        size_of::<FfiAttrs>(),
    )?;
    validate_count("arguments", frame.args.size, action.argument_count())?;
    validate_count("results", frame.rets.size, action.result_count())?;

    // SAFETY: the validated attribute table is owned by XLA for the complete
    // synchronous invocation.
    let (key, descriptor) = unsafe { decode_attributes(&frame.attrs) }?;
    let program = kernel::resolve_program(&key)?;
    let input_dimension = program.identity().input_dimension();
    let output_dimension = program.identity().output_dimension();
    let layout = Layout::new(&descriptor, action, input_dimension, output_dimension)?;

    // SAFETY: the validated argument table remains live until this handler
    // returns; decoding creates only an inert raw-range description.
    let parameters = unsafe { argument_buffer(&frame.args, 0, &layout.parameters) }?;
    let direction = match action {
        Action::Primal => None,
        Action::Jvp => {
            // SAFETY: as above, the second argument is decoded without making
            // a Rust slice until all alias checks have completed.
            Some(unsafe { argument_buffer(&frame.args, 1, &layout.direction)? })
        }
        Action::Vjp => {
            // SAFETY: as above, the second argument is decoded without making
            // a Rust slice until all alias checks have completed.
            Some(unsafe { argument_buffer(&frame.args, 1, &layout.direction)? })
        }
    };
    // SAFETY: the validated result table remains live until this handler
    // returns; decoding creates only an inert raw-range description.
    let first_output = unsafe { result_buffer(&frame.rets, 0, &layout.first_output) }?;
    let second_output = match action {
        Action::Jvp => {
            // SAFETY: as above, this remains a raw-range description until
            // mutual disjointness has been proved.
            Some(unsafe {
                result_buffer(
                    &frame.rets,
                    1,
                    layout.second_output.as_ref().expect("JVP shape"),
                )?
            })
        }
        Action::Primal | Action::Vjp => None,
    };

    ensure_disjoint(&parameters, &first_output)?;
    if let Some(direction) = direction {
        ensure_disjoint(&direction, &first_output)?;
    }
    if let Some(second_output) = second_output {
        ensure_disjoint(&parameters, &second_output)?;
        if let Some(direction) = direction {
            ensure_disjoint(&direction, &second_output)?;
        }
        ensure_disjoint(&first_output, &second_output)?;
    }

    // SAFETY: raw buffer construction proved exact aligned ranges and all
    // mutable output ranges were proved disjoint before creating any slice.
    let parameter_values = unsafe { parameters.as_slice() };
    let direction_values = direction.map(|buffer| {
        // SAFETY: every input/output combination was proved disjoint above.
        unsafe { buffer.as_slice() }
    });
    let result =
        batch::compute_action(action, program, &layout, parameter_values, direction_values)?;

    match (action, result, second_output) {
        (Action::Primal, ActionResult::Primal(values), None) => {
            validate_values(&values, first_output.element_count)?;
            // SAFETY: this XLA-owned result range is exclusive and exact.
            unsafe { first_output.copy_from_slice(&values) };
        }
        (Action::Jvp, ActionResult::Jvp { primal, tangent }, Some(second_output)) => {
            validate_values(&primal, first_output.element_count)?;
            validate_values(&tangent, second_output.element_count)?;
            // SAFETY: both exact XLA-owned result ranges are mutually disjoint.
            unsafe {
                first_output.copy_from_slice(&primal);
                second_output.copy_from_slice(&tangent);
            }
        }
        (Action::Vjp, ActionResult::Vjp(values), None) => {
            validate_values(&values, first_output.element_count)?;
            // SAFETY: this XLA-owned result range is exclusive and exact.
            unsafe { first_output.copy_from_slice(&values) };
        }
        _ => {
            return Err(HandlerFailure::data_loss(
                "Eqiora produced a mismatched JAX FFI action result",
            ));
        }
    }
    Ok(())
}

fn validate_values(values: &[f64], expected: usize) -> Result<(), HandlerFailure> {
    if values.len() != expected || values.iter().any(|value| !value.is_finite()) {
        return Err(HandlerFailure::data_loss(
            "Eqiora produced an invalid JAX FFI result",
        ));
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct RawBuffer {
    address: usize,
    byte_len: usize,
    data: *mut f64,
    element_count: usize,
}

impl RawBuffer {
    unsafe fn as_slice<'a>(self) -> &'a [f64] {
        if self.element_count == 0 {
            return &[];
        }
        // SAFETY: construction validates a non-null aligned pointer and exact
        // byte length. The caller proves that no mutable output aliases it.
        unsafe { slice::from_raw_parts(self.data.cast_const(), self.element_count) }
    }

    unsafe fn copy_from_slice(self, source: &[f64]) {
        debug_assert_eq!(source.len(), self.element_count);
        if self.element_count == 0 {
            return;
        }
        // SAFETY: construction validates the exact writable range. The caller
        // proves output exclusivity and source/output non-overlap.
        unsafe {
            ptr::copy_nonoverlapping(source.as_ptr(), self.data, self.element_count);
        }
    }
}

unsafe fn argument_buffer(
    arguments: &FfiArgs,
    index: usize,
    expected: &[usize],
) -> Result<RawBuffer, HandlerFailure> {
    // SAFETY: the caller promises that the XLA-owned argument table is live.
    let raw = unsafe { buffer_at(arguments.types, arguments.args, arguments.size, index)? };
    // SAFETY: `buffer_at` returned one non-null erased buffer entry.
    unsafe { decode_buffer(raw, expected) }
}

unsafe fn result_buffer(
    results: &FfiRets,
    index: usize,
    expected: &[usize],
) -> Result<RawBuffer, HandlerFailure> {
    // SAFETY: the caller promises that the XLA-owned result table is live.
    let raw = unsafe { buffer_at(results.types, results.rets, results.size, index)? };
    // SAFETY: `buffer_at` returned one non-null erased buffer entry.
    unsafe { decode_buffer(raw, expected) }
}

unsafe fn buffer_at(
    types: *mut c_int,
    buffers: *mut *mut c_void,
    count: i64,
    index: usize,
) -> Result<*mut FfiBuffer, HandlerFailure> {
    if count < 0 || index >= count as usize || types.is_null() || buffers.is_null() {
        return Err(HandlerFailure::invalid("XLA FFI buffer table is malformed"));
    }
    // SAFETY: count/index and both table pointers were validated above.
    if unsafe { *types.add(index) } != XLA_FFI_BUFFER {
        return Err(HandlerFailure::invalid(
            "XLA FFI value is not a dense buffer",
        ));
    }
    // SAFETY: count/index and the pointer table were validated above.
    let raw = unsafe { *buffers.add(index) };
    if raw.is_null() {
        return Err(HandlerFailure::invalid("XLA FFI buffer pointer is null"));
    }
    Ok(raw.cast())
}

unsafe fn decode_buffer(
    raw: *mut FfiBuffer,
    expected_shape: &[usize],
) -> Result<RawBuffer, HandlerFailure> {
    // SAFETY: the caller validated the erased pointer from the XLA-owned table.
    let buffer = unsafe { &*raw };
    validate_struct_size("XLA_FFI_Buffer", buffer.struct_size, size_of::<FfiBuffer>())?;
    if buffer.dtype != XLA_FFI_F64 {
        return Err(HandlerFailure::invalid(format!(
            "XLA FFI buffer must have f64 dtype (12), got {}",
            buffer.dtype
        )));
    }
    if buffer.rank < 1 || buffer.rank as usize != expected_shape.len() || buffer.dims.is_null() {
        return Err(HandlerFailure::invalid(
            "XLA FFI buffer rank disagrees with the admitted batch shape",
        ));
    }
    // SAFETY: the bounded exact rank proves how many XLA-owned dimensions exist.
    let dimensions = unsafe { slice::from_raw_parts(buffer.dims, expected_shape.len()) };
    if dimensions
        .iter()
        .zip(expected_shape)
        .any(|(&actual, &expected)| actual < 0 || actual as usize != expected)
    {
        return Err(HandlerFailure::invalid(
            "XLA FFI dimensions disagree with the admitted batch shape",
        ));
    }
    let expected = batch::elements(expected_shape)?;
    if expected > 0
        && (buffer.data.is_null()
            || !(buffer.data as usize).is_multiple_of(std::mem::align_of::<f64>()))
    {
        return Err(HandlerFailure::invalid(
            "XLA FFI f64 buffer is null or misaligned",
        ));
    }
    let byte_len = expected
        .checked_mul(size_of::<f64>())
        .ok_or_else(|| HandlerFailure::invalid("XLA FFI f64 buffer byte length overflowed"))?;
    Ok(RawBuffer {
        address: buffer.data as usize,
        byte_len,
        data: buffer.data.cast(),
        element_count: expected,
    })
}

fn ensure_disjoint(first: &RawBuffer, second: &RawBuffer) -> Result<(), HandlerFailure> {
    let first_end = first
        .address
        .checked_add(first.byte_len)
        .ok_or_else(|| HandlerFailure::invalid("XLA FFI buffer address range overflowed"))?;
    let second_end = second
        .address
        .checked_add(second.byte_len)
        .ok_or_else(|| HandlerFailure::invalid("XLA FFI buffer address range overflowed"))?;
    if first.byte_len != 0
        && second.byte_len != 0
        && first.address < second_end
        && second.address < first_end
    {
        return Err(HandlerFailure::invalid(
            "XLA FFI input and output buffers must not alias",
        ));
    }
    Ok(())
}

unsafe fn decode_attributes(attributes: &FfiAttrs) -> Result<(String, String), HandlerFailure> {
    validate_count("attributes", attributes.size, 2)?;
    if attributes.types.is_null() || attributes.names.is_null() || attributes.attrs.is_null() {
        return Err(HandlerFailure::invalid(
            "XLA FFI attribute table is malformed",
        ));
    }
    let mut fields = Vec::with_capacity(2);
    // XLA orders named attributes lexicographically. Both exact names are
    // checked, so duplicate, omitted or unexpected metadata fails closed.
    for (index, expected_name) in [b"batch".as_slice(), b"program_key"].iter().enumerate() {
        // SAFETY: the exact count and all three pointer tables were validated.
        let (kind, name, value) = unsafe {
            (
                *attributes.types.add(index),
                *attributes.names.add(index),
                (*attributes.attrs.add(index)).cast::<FfiByteSpan>(),
            )
        };
        if kind != XLA_FFI_ATTR_STRING || name.is_null() || value.is_null() {
            return Err(HandlerFailure::invalid(
                "XLA FFI string attribute is malformed",
            ));
        }
        // SAFETY: XLA owns both non-null byte spans for this invocation.
        let (name, value) = unsafe { (byte_span(&*name)?, byte_span(&*value)?) };
        if name != *expected_name || value.len() > 32 * 22 {
            return Err(HandlerFailure::invalid(
                "XLA FFI requires bounded batch and program_key attributes",
            ));
        }
        fields.push(
            std::str::from_utf8(value)
                .map_err(|_| HandlerFailure::invalid("XLA FFI metadata is not UTF-8"))?
                .to_owned(),
        );
    }
    let key = fields.pop().expect("two fields");
    let value = key.as_bytes();
    if value.len() != 64
        || value
            .iter()
            .any(|byte| !byte.is_ascii_digit() && !(b'a'..=b'f').contains(byte))
    {
        return Err(HandlerFailure::invalid(
            "Eqiora JAX program identity is not a canonical SHA-256 key",
        ));
    }
    Ok((key, fields.pop().expect("batch field")))
}

unsafe fn byte_span(span: &FfiByteSpan) -> Result<&[u8], HandlerFailure> {
    if span.len == 0 {
        return Ok(&[]);
    }
    if span.ptr.is_null() {
        return Err(HandlerFailure::invalid(
            "XLA FFI string span has a null pointer",
        ));
    }
    if span.len > 32 * 22 {
        return Err(HandlerFailure::invalid(
            "XLA FFI metadata span exceeds its bound",
        ));
    }
    // SAFETY: XLA retains this non-null span for the synchronous invocation.
    Ok(unsafe { slice::from_raw_parts(span.ptr.cast::<u8>(), span.len) })
}

unsafe fn populate_metadata(raw: *mut FfiMetadataExtension) -> Result<(), HandlerFailure> {
    if raw.is_null() {
        return Err(HandlerFailure::invalid(
            "XLA FFI metadata extension is null",
        ));
    }
    // SAFETY: the extension tag was checked before this cast.
    let extension = unsafe { &mut *raw };
    validate_struct_size(
        "XLA_FFI_Metadata_Extension",
        extension.extension_base.struct_size,
        size_of::<FfiMetadataExtension>(),
    )?;
    if extension.metadata.is_null() {
        return Err(HandlerFailure::invalid("XLA FFI metadata is null"));
    }
    // SAFETY: the extension contains a non-null metadata output pointer.
    let metadata = unsafe { &mut *extension.metadata };
    validate_struct_size(
        "XLA_FFI_Metadata",
        metadata.struct_size,
        METADATA_REQUIRED_SIZE,
    )?;
    metadata.api_version = FfiApiVersion {
        struct_size: size_of::<FfiApiVersion>(),
        extension_start: ptr::null_mut(),
        major_version: XLA_FFI_API_MAJOR,
        minor_version: XLA_FFI_API_MINOR,
    };
    metadata.traits = 0;
    if metadata.struct_size >= size_of::<FfiMetadata>() {
        metadata.state_type_id = FfiTypeId { type_id: 0 };
    }
    Ok(())
}

fn validate_count(name: &str, actual: i64, expected: usize) -> Result<(), HandlerFailure> {
    if actual < 0 || actual as usize != expected {
        return Err(HandlerFailure::invalid(format!(
            "wrong number of XLA FFI {name}: expected {expected}, got {actual}"
        )));
    }
    Ok(())
}

fn validate_struct_size(name: &str, actual: usize, required: usize) -> Result<(), HandlerFailure> {
    if actual < required {
        return Err(HandlerFailure::invalid(format!(
            "{name} is too small: expected at least {required} bytes, got {actual}"
        )));
    }
    Ok(())
}

fn ffi_error(frame: *mut FfiCallFrame, kind: FailureKind, message: &str) -> *mut c_void {
    if frame.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: the runtime owns the frame for the full handler invocation.
    let api = unsafe { (*frame).api };
    if api.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: the API prefix is readable; its own size guards the trailing
    // function slot before that slot is accessed.
    if unsafe { (*api).struct_size } < API_ERROR_CREATE_REQUIRED_SIZE {
        return ptr::null_mut();
    }
    // SAFETY: the checked API prefix includes `error_create`.
    let create = unsafe { (*api).error_create };
    let Some(create) = create else {
        return ptr::null_mut();
    };
    let message = CString::new(message.replace('\0', " "))
        .unwrap_or_else(|_| c"Eqiora JAX FFI failure".to_owned());
    let mut arguments = FfiErrorCreateArgs {
        struct_size: size_of::<FfiErrorCreateArgs>(),
        extension_start: ptr::null_mut(),
        message: message.as_ptr(),
        code: failure_code(kind),
    };
    // SAFETY: the runtime supplied this C function pointer. The message stays
    // alive for the complete call and XLA copies it into the returned error.
    unsafe { create(&mut arguments) }
}

const fn failure_code(kind: FailureKind) -> c_int {
    match kind {
        FailureKind::InvalidArgument => FFI_INVALID_ARGUMENT,
        FailureKind::NotFound => FFI_NOT_FOUND,
        FailureKind::FailedPrecondition => FFI_FAILED_PRECONDITION,
        FailureKind::Internal => FFI_INTERNAL,
        FailureKind::DataLoss => FFI_DATA_LOSS,
    }
}

#[repr(C)]
struct FfiExtensionBase {
    struct_size: usize,
    kind: c_int,
    next: *mut FfiExtensionBase,
}

#[repr(C)]
struct FfiApiVersion {
    struct_size: usize,
    extension_start: *mut FfiExtensionBase,
    major_version: c_int,
    minor_version: c_int,
}

#[repr(C)]
struct FfiApiPrefix {
    struct_size: usize,
    extension_start: *mut FfiExtensionBase,
    api_version: FfiApiVersion,
    internal_api: *const c_void,
    error_create: Option<unsafe extern "C" fn(arguments: *mut FfiErrorCreateArgs) -> *mut c_void>,
    error_get_message: *const c_void,
    error_destroy: Option<unsafe extern "C" fn(arguments: *mut FfiErrorDestroyArgs)>,
    // Unconsumed function slots: Handler_Register through RunId_Get in the
    // exact 0.11.0 header. The installed C probe checks the following offset.
    unconsumed: [*const c_void; 14],
    device_ordinal_get:
        Option<unsafe extern "C" fn(arguments: *mut FfiDeviceOrdinalArgs) -> *mut c_void>,
}

#[repr(C)]
struct FfiDeviceOrdinalArgs {
    struct_size: usize,
    extension_start: *mut FfiExtensionBase,
    context: *mut c_void,
    device_ordinal: i32,
}

#[repr(C)]
struct FfiErrorDestroyArgs {
    struct_size: usize,
    extension_start: *mut FfiExtensionBase,
    error: *mut c_void,
}

#[repr(C)]
struct FfiErrorCreateArgs {
    struct_size: usize,
    extension_start: *mut FfiExtensionBase,
    message: *const c_char,
    code: c_int,
}

#[repr(C)]
struct FfiBuffer {
    struct_size: usize,
    extension_start: *mut FfiExtensionBase,
    dtype: c_int,
    data: *mut c_void,
    rank: i64,
    dims: *mut i64,
}

#[repr(C)]
struct FfiArgs {
    struct_size: usize,
    extension_start: *mut FfiExtensionBase,
    size: i64,
    types: *mut c_int,
    args: *mut *mut c_void,
}

#[repr(C)]
struct FfiRets {
    struct_size: usize,
    extension_start: *mut FfiExtensionBase,
    size: i64,
    types: *mut c_int,
    rets: *mut *mut c_void,
}

#[repr(C)]
struct FfiByteSpan {
    ptr: *const c_char,
    len: usize,
}

#[repr(C)]
struct FfiAttrs {
    struct_size: usize,
    extension_start: *mut FfiExtensionBase,
    size: i64,
    types: *mut c_int,
    names: *mut *mut FfiByteSpan,
    attrs: *mut *mut c_void,
}

#[repr(C)]
struct FfiCallFrame {
    struct_size: usize,
    extension_start: *mut FfiExtensionBase,
    api: *const FfiApiPrefix,
    context: *mut c_void,
    stage: c_int,
    args: FfiArgs,
    rets: FfiRets,
    attrs: FfiAttrs,
    future: *mut c_void,
}

#[repr(C)]
struct FfiTypeId {
    type_id: i64,
}

#[repr(C)]
struct FfiMetadata {
    struct_size: usize,
    api_version: FfiApiVersion,
    traits: u32,
    state_type_id: FfiTypeId,
}

#[repr(C)]
struct FfiMetadataExtension {
    extension_base: FfiExtensionBase,
    metadata: *mut FfiMetadata,
}

const CALL_FRAME_REQUIRED_SIZE: usize = offset_of!(FfiCallFrame, attrs) + size_of::<FfiAttrs>();
const METADATA_REQUIRED_SIZE: usize = offset_of!(FfiMetadata, traits) + size_of::<u32>();
const API_ERROR_CREATE_REQUIRED_SIZE: usize = offset_of!(FfiApiPrefix, error_create)
    + size_of::<Option<unsafe extern "C" fn(*mut FfiErrorCreateArgs) -> *mut c_void>>();
const API_DEVICE_ORDINAL_REQUIRED_SIZE: usize = offset_of!(FfiApiPrefix, device_ordinal_get)
    + size_of::<Option<unsafe extern "C" fn(*mut FfiDeviceOrdinalArgs) -> *mut c_void>>();
const DEVICE_ORDINAL_ARGS_REQUIRED_SIZE: usize =
    offset_of!(FfiDeviceOrdinalArgs, device_ordinal) + size_of::<i32>();

unsafe fn validate_device_ordinal(frame: &FfiCallFrame) -> Result<(), HandlerFailure> {
    if frame.api.is_null() || frame.context.is_null() {
        return Err(HandlerFailure::invalid(
            "XLA FFI CPU execution context is absent",
        ));
    }
    // SAFETY: the runtime supplies the API prefix; its size guards later slots.
    validate_struct_size(
        "XLA_FFI_Api",
        // SAFETY: the runtime supplies this readable API prefix.
        unsafe { (*frame.api).struct_size },
        API_DEVICE_ORDINAL_REQUIRED_SIZE,
    )?;
    // SAFETY: the size check includes both function slots read here.
    let (Some(get), Some(destroy)) = (unsafe { (*frame.api).device_ordinal_get }, unsafe {
        (*frame.api).error_destroy
    }) else {
        return Err(HandlerFailure::invalid(
            "XLA FFI device ordinal API is unavailable",
        ));
    };
    let mut args = FfiDeviceOrdinalArgs {
        struct_size: DEVICE_ORDINAL_ARGS_REQUIRED_SIZE,
        extension_start: ptr::null_mut(),
        context: frame.context,
        device_ordinal: -1,
    };
    // SAFETY: the exact C arguments and execution context live through the call.
    let error = unsafe { get(&mut args) };
    if !error.is_null() {
        let mut args = FfiErrorDestroyArgs {
            struct_size: size_of::<FfiErrorDestroyArgs>(),
            extension_start: ptr::null_mut(),
            error,
        };
        // SAFETY: the runtime returned ownership of this error to the caller.
        unsafe { destroy(&mut args) };
        return Err(HandlerFailure::invalid(
            "XLA FFI could not provide the CPU device ordinal",
        ));
    }
    if args.device_ordinal < 0 {
        return Err(HandlerFailure::invalid(
            "XLA FFI CPU device ordinal is invalid",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dense_buffers_reject_wrong_axis_order_dtype_rank_and_aliases() {
        let mut values = [0.0; 6];
        let mut dimensions = [2, 3];
        let mut buffer = FfiBuffer {
            struct_size: size_of::<FfiBuffer>(),
            extension_start: ptr::null_mut(),
            dtype: XLA_FFI_F64,
            data: values.as_mut_ptr().cast(),
            rank: 2,
            dims: dimensions.as_mut_ptr(),
        };
        // SAFETY: the test owns the buffer, dimensions and data through each call.
        let raw = unsafe { decode_buffer(&mut buffer, &[2, 3]) }.unwrap();
        assert_eq!(raw.element_count, 6);
        // Equal flat lengths do not admit a different axis association.
        // SAFETY: the test still owns the buffer, dimensions and data.
        assert!(unsafe { decode_buffer(&mut buffer, &[3, 2]) }.is_err());
        buffer.dtype = 11;
        // SAFETY: only the owned dtype changed; all backing storage remains live.
        assert!(unsafe { decode_buffer(&mut buffer, &[2, 3]) }.is_err());
        buffer.dtype = XLA_FFI_F64;
        buffer.rank = 1;
        // SAFETY: both owned dimensions remain allocated; the wrong rank rejects.
        assert!(unsafe { decode_buffer(&mut buffer, &[2, 3]) }.is_err());
        assert!(ensure_disjoint(&raw, &raw).is_err());
        let overflow = RawBuffer {
            address: usize::MAX,
            byte_len: 8,
            ..raw
        };
        assert!(ensure_disjoint(&overflow, &raw).is_err());
        let empty = RawBuffer {
            address: 0,
            data: ptr::null_mut(),
            byte_len: 0,
            element_count: 0,
        };
        assert!(ensure_disjoint(&empty, &raw).is_ok());
        // SAFETY: an empty buffer never constructs a slice from the null pointer.
        assert!(unsafe { empty.as_slice() }.is_empty());
    }

    #[test]
    fn admitted_jaxlib_layout_matches_reviewed_header() {
        assert_eq!(XLA_FFI_EXTENSION_METADATA, 1);
        assert_eq!(XLA_FFI_EXECUTE, 3);
        assert_eq!(XLA_FFI_BUFFER, 1);
        assert_eq!(XLA_FFI_ATTR_STRING, 4);
        assert_eq!(XLA_FFI_F64, 12);
        assert_eq!(FFI_INVALID_ARGUMENT, 3);
        assert_eq!(FFI_NOT_FOUND, 5);
        assert_eq!(FFI_FAILED_PRECONDITION, 9);
        assert_eq!(FFI_INTERNAL, 13);
        assert_eq!(FFI_DATA_LOSS, 15);
        assert_eq!(size_of::<FfiExtensionBase>(), 24);
        assert_eq!(size_of::<FfiApiVersion>(), 24);
        assert_eq!(size_of::<FfiErrorCreateArgs>(), 32);
        assert_eq!(size_of::<FfiBuffer>(), 48);
        assert_eq!(size_of::<FfiArgs>(), 40);
        assert_eq!(size_of::<FfiRets>(), 40);
        assert_eq!(size_of::<FfiByteSpan>(), 16);
        assert_eq!(size_of::<FfiAttrs>(), 48);
        assert_eq!(size_of::<FfiCallFrame>(), 176);
        assert_eq!(offset_of!(FfiCallFrame, attrs), 120);
        assert_eq!(offset_of!(FfiCallFrame, future), 168);
        assert_eq!(CALL_FRAME_REQUIRED_SIZE, 168);
        assert_eq!(size_of::<FfiMetadata>(), 48);
        assert_eq!(offset_of!(FfiMetadata, traits), 32);
        assert_eq!(offset_of!(FfiMetadata, state_type_id), 40);
        assert_eq!(METADATA_REQUIRED_SIZE, 36);
        assert_eq!(size_of::<FfiMetadataExtension>(), 32);
        assert_eq!(offset_of!(FfiApiPrefix, error_create), 48);
        assert_eq!(offset_of!(FfiApiPrefix, error_destroy), 64);
        assert_eq!(offset_of!(FfiApiPrefix, device_ordinal_get), 184);
        assert_eq!(API_DEVICE_ORDINAL_REQUIRED_SIZE, 192);
        assert_eq!(size_of::<FfiDeviceOrdinalArgs>(), 32);
        assert_eq!(DEVICE_ORDINAL_ARGS_REQUIRED_SIZE, 28);
        assert_eq!(size_of::<FfiErrorDestroyArgs>(), 24);
    }
}
