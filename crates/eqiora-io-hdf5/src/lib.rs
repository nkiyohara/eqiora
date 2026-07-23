//! Native HDF5 resolution and production of caller-owned complete file images.
//!
//! No Eqiora API grants a path, directory, URL, network, or plugin capability.
//! The adapter opens only complete caller-supplied bytes through HDF5's Core
//! VFD file-image facility, fixes the native VOL, and disables plugin loading
//! for the serialized operation. Before the first dataset read, it audits the
//! whole reachable hard-link graph and admits only a bounded tree of groups
//! plus unfiltered, internally stored, non-virtual `u64`/`f64` datasets.
//! The symmetric writer accepts only bounded primitive arrays and returns a
//! deterministic complete in-memory image under one exact recorded runtime
//! profile; it grants no persistence authority.
//! Effects of a hostile process environment before native-library
//! initialization, and defects or unbounded work inside HDF5 itself, require a
//! future isolated worker and are deliberately not claimed here. Native
//! handles and binding types never cross this crate boundary.

mod audit;
mod contract;
#[allow(unsafe_code)]
mod file_image;
#[allow(unsafe_code)]
mod native_inspect;
mod resolver;
#[allow(unsafe_code)]
mod writer;

pub use contract::{
    HDF5_BINDING_ID, HDF5_BINDING_VERSION, HDF5_NATIVE_LIBRARY_ID, Hdf5DatasetRequest,
    Hdf5DatasetWrite, Hdf5FileImage, Hdf5ResolveLimits, Hdf5ResolvedValues, Hdf5RuntimeIdentity,
    Hdf5ScalarType, Hdf5WriteLimits, Hdf5WriteValues,
};
pub use resolver::{Hdf5FileResolution, resolve_hdf5_file_image};
pub use writer::{WrittenHdf5FileImage, write_hdf5_file_image};
