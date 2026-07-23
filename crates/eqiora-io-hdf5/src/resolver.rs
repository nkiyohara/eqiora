use eqiora_core::Diagnostic;
use hdf5_metno::Dataset;

use crate::audit::{checked_product, complete_file, require_at_most};
use crate::contract::{
    Hdf5DatasetRequest, Hdf5FileImage, Hdf5ResolveLimits, Hdf5ResolvedValues, Hdf5RuntimeIdentity,
    Hdf5ScalarType, invalid_hdf5,
};
use crate::file_image;
use crate::native_inspect::{
    exact_scalar_type, exact_shape, read_exact_f64, read_exact_u64, with_plugins_disabled,
};

/// One ordered multi-dataset resolution plus observed native-runtime identity.
#[derive(Debug, Clone, PartialEq)]
pub struct Hdf5FileResolution {
    runtime: Hdf5RuntimeIdentity,
    values: Vec<Hdf5ResolvedValues>,
}

impl Hdf5FileResolution {
    /// Exact binding and native-library runtime identity.
    #[must_use]
    pub const fn runtime(&self) -> &Hdf5RuntimeIdentity {
        &self.runtime
    }

    /// Normalized arrays in immutable request order.
    #[must_use]
    pub fn values(&self) -> &[Hdf5ResolvedValues] {
        &self.values
    }

    /// Consume the resolution and transfer normalized arrays without copying.
    #[must_use]
    pub fn into_values(self) -> Vec<Hdf5ResolvedValues> {
        self.values
    }
}

/// Resolve an ordered request batch from one complete caller-owned file image.
///
/// The whole reachable object graph is audited, then every requested
/// path/shape/type is preflighted, before the first data read. File paths,
/// external storage, external/soft/user links, VDS mappings, and filter plugins
/// are never accepted as data authority.
///
/// # Errors
/// Returns `EQ0810` for malformed input, a rejected HDF5 construct, request
/// mismatch, native-library failure, or resource-limit excess.
pub fn resolve_hdf5_file_image(
    source: Hdf5FileImage<'_>,
    requests: &[Hdf5DatasetRequest],
    limits: Hdf5ResolveLimits,
) -> Result<Hdf5FileResolution, Diagnostic> {
    with_plugins_disabled(|| resolve_inner(source, requests, limits))
}

fn resolve_inner(
    source: Hdf5FileImage<'_>,
    requests: &[Hdf5DatasetRequest],
    limits: Hdf5ResolveLimits,
) -> Result<Hdf5FileResolution, Diagnostic> {
    let limits = limits.validate()?;
    let bytes = source.bytes();
    if bytes.is_empty() {
        return Err(invalid_hdf5("HDF5 file image must not be empty"));
    }
    if requests.is_empty() {
        return Err(invalid_hdf5(
            "HDF5 file-image resolution requires at least one request",
        ));
    }
    require_at_most(bytes.len(), limits.max_source_bytes, "HDF5 source bytes")?;
    require_at_most(requests.len(), limits.max_requests, "HDF5 request count")?;

    let mut request_shapes = Vec::new();
    request_shapes
        .try_reserve_exact(requests.len())
        .map_err(|error| {
            invalid_hdf5(format!(
                "cannot reserve bounded HDF5 request-shape preflight: {error}",
            ))
        })?;
    for request in requests {
        request_shapes.push(prevalidate_request(request, limits)?);
    }

    let file = file_image::open(bytes)?;
    complete_file(&file, limits)?;
    let mut preflight = Vec::new();
    preflight.try_reserve(requests.len()).map_err(|error| {
        invalid_hdf5(format!(
            "cannot reserve bounded HDF5 request preflight: {error}",
        ))
    })?;
    let mut total_resolved_bytes = 0_usize;
    for (request, requested_shape) in requests.iter().zip(request_shapes) {
        let dataset = file.dataset(request.dataset_path()).map_err(|error| {
            invalid_hdf5(format!("cannot open requested HDF5 dataset: {error}"))
        })?;
        let actual_shape = exact_shape(&dataset, limits.max_rank)?;
        if actual_shape != request.shape() {
            return Err(invalid_hdf5(
                "HDF5 dataset shape differs from the immutable request",
            ));
        }
        let actual_scalar = exact_scalar_type(&dataset)?;
        if actual_scalar != request.scalar() {
            return Err(invalid_hdf5(
                "HDF5 dataset scalar type differs from the immutable request",
            ));
        }
        let value_count = checked_product(&requested_shape, "HDF5 requested scalar count")?;
        let resolved_bytes = value_count
            .checked_mul(std::mem::size_of::<u64>())
            .ok_or_else(|| invalid_hdf5("HDF5 requested decoded byte count overflows usize"))?;
        require_at_most(
            resolved_bytes,
            limits.max_dataset_resolved_bytes,
            "HDF5 requested dataset decoded bytes",
        )?;
        total_resolved_bytes = total_resolved_bytes
            .checked_add(resolved_bytes)
            .ok_or_else(|| {
                invalid_hdf5("aggregate HDF5 requested decoded byte count overflows usize")
            })?;
        require_at_most(
            total_resolved_bytes,
            limits.max_total_resolved_bytes,
            "aggregate HDF5 requested decoded bytes",
        )?;
        preflight.push((dataset, actual_scalar, value_count));
    }

    let mut values = Vec::new();
    values.try_reserve(preflight.len()).map_err(|error| {
        invalid_hdf5(format!(
            "cannot reserve bounded HDF5 resolved-array storage: {error}",
        ))
    })?;
    for (dataset, scalar, expected_values) in preflight {
        let value = read_dataset(&dataset, scalar, expected_values)?;
        let actual_values = match &value {
            Hdf5ResolvedValues::U64(values) => values.len(),
            Hdf5ResolvedValues::F64(values) => values.len(),
        };
        if actual_values != expected_values {
            return Err(invalid_hdf5(
                "HDF5 decoded value count differs from the immutable request",
            ));
        }
        values.push(value);
    }
    file.close()
        .map_err(|error| invalid_hdf5(format!("cannot close HDF5 file image: {error}")))?;

    Ok(Hdf5FileResolution {
        runtime: Hdf5RuntimeIdentity::observed()?,
        values,
    })
}

fn prevalidate_request(
    request: &Hdf5DatasetRequest,
    limits: Hdf5ResolveLimits,
) -> Result<Vec<usize>, Diagnostic> {
    require_at_most(
        request.dataset_path().len(),
        limits.max_name_bytes,
        "HDF5 requested dataset-path bytes",
    )?;
    require_at_most(
        request.shape().len(),
        limits.max_rank,
        "HDF5 requested rank",
    )?;
    let mut shape = Vec::new();
    shape
        .try_reserve_exact(request.shape().len())
        .map_err(|error| {
            invalid_hdf5(format!(
                "cannot reserve bounded HDF5 requested shape: {error}",
            ))
        })?;
    for extent in request.shape() {
        shape.push(
            usize::try_from(*extent)
                .map_err(|_| invalid_hdf5("HDF5 requested extent exceeds usize"))?,
        );
    }
    let values = checked_product(&shape, "HDF5 requested scalar count")?;
    require_at_most(
        values,
        limits.max_dataset_values,
        "HDF5 requested scalar count",
    )?;
    Ok(shape)
}

fn read_dataset(
    dataset: &Dataset,
    scalar: Hdf5ScalarType,
    expected_values: usize,
) -> Result<Hdf5ResolvedValues, Diagnostic> {
    match scalar {
        Hdf5ScalarType::U64 => {
            read_exact_u64(dataset, expected_values).map(Hdf5ResolvedValues::U64)
        }
        Hdf5ScalarType::F64 => {
            read_exact_f64(dataset, expected_values).map(Hdf5ResolvedValues::F64)
        }
    }
}
