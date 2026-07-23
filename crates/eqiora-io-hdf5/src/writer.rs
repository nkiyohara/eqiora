use std::collections::BTreeSet;
use std::ptr;

use eqiora_core::Diagnostic;
use hdf5_metno::{File, h5check};
use hdf5_metno_sys::h5f::H5Fget_file_image;

use crate::contract::{
    Hdf5DatasetWrite, Hdf5RuntimeIdentity, Hdf5WriteLimits, Hdf5WriteValues, invalid_hdf5_export,
};
use crate::native_inspect::with_plugins_disabled;

const MEMORY_FILE_NAME: &str = "eqiora-generated.h5";

/// Complete deterministic in-memory HDF5 output plus the exact native stack
/// that produced it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WrittenHdf5FileImage {
    bytes: Vec<u8>,
    runtime: Hdf5RuntimeIdentity,
}

impl WrittenHdf5FileImage {
    /// Complete generated file-image bytes.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Exact binding and native-library runtime identity.
    #[must_use]
    pub const fn runtime(&self) -> &Hdf5RuntimeIdentity {
        &self.runtime
    }

    /// Consume the result and transfer its complete file-image bytes.
    #[must_use]
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
}

/// Write a closed ordered set of primitive arrays into one complete HDF5 file
/// image without granting filesystem or network authority.
///
/// Declaration order is non-semantic: datasets and derived groups are created
/// in canonical path order. The Core VFD has backing storage disabled, object
/// timestamps are disabled, datasets are fixed-size, contiguous, unfiltered,
/// and use no attributes or committed datatypes. The output therefore lies in
/// the exact subset accepted by [`crate::resolve_hdf5_file_image`].
///
/// # Errors
/// Returns `EQ0811` for duplicate or over-budget declarations, invalid local
/// conversions, native HDF5 failure, or an oversized output image.
pub fn write_hdf5_file_image(
    datasets: &[Hdf5DatasetWrite<'_>],
    limits: Hdf5WriteLimits,
) -> Result<WrittenHdf5FileImage, Diagnostic> {
    with_plugins_disabled(|| write_inner(datasets, limits))
}

fn write_inner(
    datasets: &[Hdf5DatasetWrite<'_>],
    limits: Hdf5WriteLimits,
) -> Result<WrittenHdf5FileImage, Diagnostic> {
    let limits = limits.validate()?;
    if datasets.is_empty() {
        return Err(invalid_hdf5_export(
            "HDF5 file-image export requires at least one dataset",
        ));
    }
    require_at_most(datasets.len(), limits.max_datasets, "HDF5 export datasets")?;

    let mut ordered = datasets.iter().collect::<Vec<_>>();
    ordered.sort_by(|left, right| left.dataset_path().cmp(right.dataset_path()));
    if ordered
        .windows(2)
        .any(|pair| pair[0].dataset_path() == pair[1].dataset_path())
    {
        return Err(invalid_hdf5_export(
            "HDF5 export dataset paths must be unique",
        ));
    }

    let mut total_name_bytes = 0_usize;
    let mut total_values = 0_usize;
    let mut shapes = Vec::new();
    shapes.try_reserve_exact(ordered.len()).map_err(|error| {
        invalid_hdf5_export(format!(
            "cannot reserve bounded HDF5 export shape preflight: {error}",
        ))
    })?;
    for dataset in &ordered {
        require_at_most(
            dataset.dataset_path().len(),
            limits.max_name_bytes,
            "HDF5 export dataset-path bytes",
        )?;
        total_name_bytes = total_name_bytes
            .checked_add(dataset.dataset_path().len())
            .ok_or_else(|| invalid_hdf5_export("HDF5 export name-byte total overflows usize"))?;
        require_at_most(
            total_name_bytes,
            limits.max_total_name_bytes,
            "aggregate HDF5 export dataset-path bytes",
        )?;
        require_at_most(
            dataset.shape().len(),
            limits.max_rank,
            "HDF5 export dataset rank",
        )?;
        let shape = dataset
            .shape()
            .iter()
            .map(|extent| {
                usize::try_from(*extent)
                    .map_err(|_| invalid_hdf5_export("HDF5 export extent exceeds usize"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let value_count = shape.iter().try_fold(1_usize, |total, extent| {
            total
                .checked_mul(*extent)
                .ok_or_else(|| invalid_hdf5_export("HDF5 export shape product overflows usize"))
        })?;
        require_at_most(
            value_count,
            limits.max_dataset_values,
            "HDF5 export dataset scalar values",
        )?;
        total_values = total_values
            .checked_add(value_count)
            .ok_or_else(|| invalid_hdf5_export("HDF5 export value total overflows usize"))?;
        require_at_most(
            total_values,
            limits.max_total_values,
            "aggregate HDF5 export scalar values",
        )?;
        shapes.push(shape);
    }

    let groups = group_paths(&ordered, limits)?;
    let file = File::with_options()
        .with_fapl(|properties| properties.core_filebacked(false).libver_earliest())
        .with_fcpl(|properties| properties.obj_track_times(false))
        .create(MEMORY_FILE_NAME)
        .map_err(|error| native_export_error("cannot create in-memory HDF5 output", error))?;
    for path in groups {
        file.create_group_builder()
            .obj_track_times(false)
            .create(path.as_str())
            .map_err(|error| native_export_error("cannot create HDF5 export group", error))?;
    }
    for (dataset, shape) in ordered.into_iter().zip(shapes) {
        match dataset.values() {
            Hdf5WriteValues::U64(values) => {
                let output = file
                    .new_dataset::<u64>()
                    .shape(shape)
                    .no_chunk()
                    .obj_track_times(false)
                    .create(dataset.dataset_path())
                    .map_err(|error| {
                        native_export_error("cannot create HDF5 u64 export dataset", error)
                    })?;
                output.write_raw(values).map_err(|error| {
                    native_export_error("cannot write HDF5 u64 export dataset", error)
                })?;
            }
            Hdf5WriteValues::F64(values) => {
                let output = file
                    .new_dataset::<f64>()
                    .shape(shape)
                    .no_chunk()
                    .obj_track_times(false)
                    .create(dataset.dataset_path())
                    .map_err(|error| {
                        native_export_error("cannot create HDF5 f64 export dataset", error)
                    })?;
                output.write_raw(values).map_err(|error| {
                    native_export_error("cannot write HDF5 f64 export dataset", error)
                })?;
            }
        }
    }
    file.flush()
        .map_err(|error| native_export_error("cannot flush in-memory HDF5 output", error))?;
    let bytes = file_image(&file, limits.max_output_bytes)?;
    file.close()
        .map_err(|error| native_export_error("cannot close in-memory HDF5 output", error))?;
    Ok(WrittenHdf5FileImage {
        bytes,
        runtime: Hdf5RuntimeIdentity::observed()?,
    })
}

fn group_paths(
    datasets: &[&Hdf5DatasetWrite<'_>],
    limits: Hdf5WriteLimits,
) -> Result<Vec<String>, Diagnostic> {
    let mut groups = BTreeSet::new();
    for dataset in datasets {
        let mut path = String::new();
        let mut segments = dataset.dataset_path().split('/').skip(1).peekable();
        while let Some(segment) = segments.next() {
            if segments.peek().is_none() {
                break;
            }
            path.push('/');
            path.push_str(segment);
            require_at_most(
                path.len(),
                limits.max_name_bytes,
                "HDF5 export group-path bytes",
            )?;
            groups.insert(path.clone());
        }
    }
    require_at_most(groups.len(), limits.max_groups, "HDF5 export groups")?;
    Ok(groups.into_iter().collect())
}

fn file_image(file: &File, limit: usize) -> Result<Vec<u8>, Diagnostic> {
    let required = hdf5_metno::sync::sync(|| {
        // SAFETY: `file` is a live flushed Core-VFD handle and a null output
        // pointer with zero length is the documented size query.
        h5check(unsafe { H5Fget_file_image(file.id(), ptr::null_mut(), 0) })
            .map_err(|error| native_export_error("cannot size generated HDF5 file image", error))
    })?;
    let required = usize::try_from(required)
        .map_err(|_| invalid_hdf5_export("generated HDF5 file-image size exceeds usize"))?;
    if required == 0 {
        return Err(invalid_hdf5_export(
            "generated HDF5 file image is unexpectedly empty",
        ));
    }
    require_at_most(required, limit, "generated HDF5 file-image bytes")?;
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(required).map_err(|error| {
        invalid_hdf5_export(format!("cannot reserve generated HDF5 file image: {error}",))
    })?;
    bytes.resize(required, 0);
    let written = hdf5_metno::sync::sync(|| {
        // SAFETY: `bytes` exposes exactly `required` initialized writable
        // bytes and `file` remains live and flushed under the binding mutex.
        h5check(unsafe { H5Fget_file_image(file.id(), bytes.as_mut_ptr().cast(), bytes.len()) })
            .map_err(|error| native_export_error("cannot copy generated HDF5 file image", error))
    })?;
    if usize::try_from(written).ok() != Some(required) {
        return Err(invalid_hdf5_export(
            "generated HDF5 file-image size changed during extraction",
        ));
    }
    Ok(bytes)
}

fn require_at_most(actual: usize, limit: usize, label: &str) -> Result<(), Diagnostic> {
    if actual > limit {
        Err(invalid_hdf5_export(format!(
            "{label} {actual} exceeds configured limit {limit}",
        )))
    } else {
        Ok(())
    }
}

fn native_export_error(context: &str, error: hdf5_metno::Error) -> Diagnostic {
    invalid_hdf5_export(format!("{context}: {error}"))
}
