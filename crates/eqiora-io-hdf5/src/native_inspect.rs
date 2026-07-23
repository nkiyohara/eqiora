use eqiora_core::Diagnostic;
use hdf5_metno::{Dataset, PropertyList, h5check};
use hdf5_metno_sys::h5d::H5Dread;
use hdf5_metno_sys::h5i::hid_t;
use hdf5_metno_sys::h5p::H5P_DEFAULT;
use hdf5_metno_sys::h5p::{H5Pget_external_count, H5Pget_nfilters};
use hdf5_metno_sys::h5s::{
    H5S_ALL, H5Sget_simple_extent_dims, H5Sget_simple_extent_ndims, H5Sis_simple,
};
use hdf5_metno_sys::h5t::{H5Tcommitted, H5Tequal};
use std::os::raw::{c_int, c_uint};
use std::ptr;

use crate::contract::{Hdf5ScalarType, invalid_hdf5};

unsafe extern "C" {
    #[link_name = "H5PLget_loading_state"]
    fn h5pl_get_loading_state(plugin_control_mask: *mut c_uint) -> c_int;
    #[link_name = "H5PLset_loading_state"]
    fn h5pl_set_loading_state(plugin_control_mask: c_uint) -> c_int;
}

pub(crate) fn with_plugins_disabled<T>(
    operation: impl FnOnce() -> Result<T, Diagnostic>,
) -> Result<T, Diagnostic> {
    hdf5_metno::sync::sync(|| {
        let mut guard = PluginLoadingGuard::enter()?;
        let result = operation();
        let restore = guard.restore();
        match (result, restore) {
            (Ok(value), Ok(())) => Ok(value),
            (Err(error), Ok(())) => Err(error),
            (_, Err(error)) => Err(error),
        }
    })
}

struct PluginLoadingGuard {
    previous: c_uint,
    active: bool,
}

impl PluginLoadingGuard {
    fn enter() -> Result<Self, Diagnostic> {
        let mut previous = 0;
        // SAFETY: `previous` is a live out pointer, the symbol's signature is
        // the official HDF5 C ABI (the upstream Rust binding declares the set
        // argument incorrectly), and the binding mutex is held by the caller.
        h5check(unsafe { h5pl_get_loading_state(&mut previous) })
            .map_err(|error| native_error("cannot observe HDF5 plugin loading state", error))?;
        // SAFETY: zero is the documented mask disabling every plugin family;
        // the binding mutex is held by the caller.
        h5check(unsafe { h5pl_set_loading_state(0) })
            .map_err(|error| native_error("cannot disable HDF5 plugin loading", error))?;
        Ok(Self {
            previous,
            active: true,
        })
    }

    fn restore(&mut self) -> Result<(), Diagnostic> {
        // SAFETY: `previous` was returned by H5PLget_loading_state and the
        // binding mutex remains held by the caller.
        h5check(unsafe { h5pl_set_loading_state(self.previous) })
            .map_err(|error| native_error("cannot restore HDF5 plugin loading state", error))?;
        self.active = false;
        Ok(())
    }
}

impl Drop for PluginLoadingGuard {
    fn drop(&mut self) {
        if self.active {
            // SAFETY: best-effort unwind restoration uses the exact state read
            // on entry while the outer binding mutex is still held.
            let _ = unsafe { h5pl_set_loading_state(self.previous) };
        }
    }
}

pub(crate) fn require_internal_unfiltered_storage(dcpl: &PropertyList) -> Result<(), Diagnostic> {
    let (external_count, filter_count) = hdf5_metno::sync::sync(|| {
        // SAFETY: `dcpl.id()` is a live, owned dataset-creation property-list
        // identifier and the binding's global recursive mutex is held.
        let external = h5check(unsafe { H5Pget_external_count(dcpl.id()) })
            .map_err(|error| native_error("cannot count HDF5 external storage", error))?;
        // SAFETY: the same live property-list and binding mutex conditions hold.
        let filters = h5check(unsafe { H5Pget_nfilters(dcpl.id()) })
            .map_err(|error| native_error("cannot count HDF5 filters", error))?;
        Ok::<_, Diagnostic>((external, filters))
    })?;
    if external_count != 0 {
        return Err(invalid_hdf5("HDF5 v1 rejects external raw storage"));
    }
    if filter_count != 0 {
        return Err(invalid_hdf5("HDF5 v1 rejects every filter pipeline"));
    }
    Ok(())
}

pub(crate) fn exact_scalar_type(dataset: &Dataset) -> Result<Hdf5ScalarType, Diagnostic> {
    let datatype = dataset
        .dtype()
        .map_err(|error| native_error("cannot inspect HDF5 dataset datatype", error))?;
    hdf5_metno::sync::sync(|| {
        // SAFETY: `datatype` is a live HDF5 datatype handle and the binding's
        // global recursive mutex is held.
        let committed = h5check(unsafe { H5Tcommitted(datatype.id()) })
            .map_err(|error| native_error("cannot inspect HDF5 datatype commitment", error))?;
        if committed > 0 {
            return Err(invalid_hdf5(
                "HDF5 v1 rejects committed datatypes whether linked or unlinked",
            ));
        }
        let candidates = [
            (Hdf5ScalarType::U64, *hdf5_metno::globals::H5T_STD_U64LE),
            (Hdf5ScalarType::U64, *hdf5_metno::globals::H5T_STD_U64BE),
            (Hdf5ScalarType::F64, *hdf5_metno::globals::H5T_IEEE_F64LE),
            (Hdf5ScalarType::F64, *hdf5_metno::globals::H5T_IEEE_F64BE),
        ];
        for (scalar, candidate) in candidates {
            // SAFETY: both identifiers are live HDF5 datatype handles and the
            // binding's global recursive mutex is held.
            let equal = h5check(unsafe { H5Tequal(datatype.id(), candidate) })
                .map_err(|error| native_error("cannot compare HDF5 datatypes", error))?;
            if equal > 0 {
                return Ok(scalar);
            }
        }
        Err(invalid_hdf5(
            "HDF5 v1 admits only exact IEEE float64 or standard uint64, in either byte order",
        ))
    })
}

pub(crate) fn exact_shape(dataset: &Dataset, max_rank: usize) -> Result<Vec<u64>, Diagnostic> {
    let dataspace = dataset
        .space()
        .map_err(|error| native_error("cannot inspect HDF5 dataset dataspace", error))?;
    hdf5_metno::sync::sync(|| {
        // SAFETY: `dataspace.id()` is a live HDF5 dataspace handle and the
        // binding's recursive mutex is held.
        let simple = h5check(unsafe { H5Sis_simple(dataspace.id()) })
            .map_err(|error| native_error("cannot classify HDF5 dataspace", error))?;
        if simple == 0 {
            return Err(invalid_hdf5("HDF5 v1 requires a simple dataspace"));
        }
        // SAFETY: the same live dataspace and binding mutex conditions hold.
        let rank = h5check(unsafe { H5Sget_simple_extent_ndims(dataspace.id()) })
            .map_err(|error| native_error("cannot inspect HDF5 dataspace rank", error))?;
        let rank =
            usize::try_from(rank).map_err(|_| invalid_hdf5("HDF5 dataspace rank is negative"))?;
        if rank == 0 || rank > max_rank {
            return Err(invalid_hdf5(
                "HDF5 v1 requires a positive non-scalar rank within the configured limit",
            ));
        }
        let mut shape = Vec::new();
        shape
            .try_reserve_exact(rank)
            .map_err(|error| invalid_hdf5(format!("cannot reserve bounded HDF5 shape: {error}")))?;
        shape.resize(rank, 0_u64);
        // SAFETY: `shape` exposes exactly `rank` initialized hsize_t-compatible
        // u64 elements, maxdims is intentionally ignored, the dataspace is
        // live, and the binding mutex is held.
        let observed_rank = h5check(unsafe {
            H5Sget_simple_extent_dims(dataspace.id(), shape.as_mut_ptr(), ptr::null_mut())
        })
        .map_err(|error| native_error("cannot inspect HDF5 dataspace extents", error))?;
        if usize::try_from(observed_rank).ok() != Some(rank) || shape.contains(&0) {
            return Err(invalid_hdf5(
                "HDF5 dataspace extents are nonpositive or changed during inspection",
            ));
        }
        Ok(shape)
    })
}

pub(crate) fn read_exact_u64(
    dataset: &Dataset,
    value_count: usize,
) -> Result<Vec<u64>, Diagnostic> {
    read_exact(
        dataset,
        value_count,
        *hdf5_metno::globals::H5T_NATIVE_UINT64,
    )
}

pub(crate) fn read_exact_f64(
    dataset: &Dataset,
    value_count: usize,
) -> Result<Vec<f64>, Diagnostic> {
    read_exact(
        dataset,
        value_count,
        *hdf5_metno::globals::H5T_NATIVE_DOUBLE,
    )
}

fn read_exact<T: Copy + Default>(
    dataset: &Dataset,
    value_count: usize,
    memory_type: hid_t,
) -> Result<Vec<T>, Diagnostic> {
    let mut values = Vec::new();
    values.try_reserve_exact(value_count).map_err(|error| {
        invalid_hdf5(format!(
            "cannot reserve bounded HDF5 decoded values: {error}",
        ))
    })?;
    values.resize(value_count, T::default());
    hdf5_metno::sync::sync(|| {
        // SAFETY: the dataset and native fixed-size memory datatype are live;
        // `values` contains exactly the preflighted dataset element count;
        // H5S_ALL selects the complete file and memory extents; the default
        // transfer policy requires no callback authority; and the binding's
        // recursive mutex is held.
        h5check(unsafe {
            H5Dread(
                dataset.id(),
                memory_type,
                H5S_ALL,
                H5S_ALL,
                H5P_DEFAULT,
                values.as_mut_ptr().cast(),
            )
        })
        .map_err(|error| native_error("cannot read preflighted HDF5 dataset", error))?;
        Ok(values)
    })
}

fn native_error(context: &str, error: hdf5_metno::Error) -> Diagnostic {
    invalid_hdf5(format!("{context}: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_loading_state_is_restored_after_rejection() {
        let before = plugin_loading_state();
        let error = with_plugins_disabled(|| {
            assert_eq!(plugin_loading_state(), 0);
            Err::<(), _>(invalid_hdf5("deliberate rejection"))
        })
        .unwrap_err();

        assert_eq!(
            error.code(),
            eqiora_core::diagnostic::codes::INVALID_EXTERNAL_DATA_IMPORT
        );
        assert_eq!(plugin_loading_state(), before);
    }

    fn plugin_loading_state() -> c_uint {
        hdf5_metno::sync::sync(|| {
            let mut state = 0;
            // SAFETY: `state` is a live out pointer and the binding's recursive
            // mutex is held for the exact official HDF5 C ABI call.
            assert!(unsafe { h5pl_get_loading_state(&mut state) } >= 0);
            state
        })
    }
}
