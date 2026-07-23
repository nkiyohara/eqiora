# Reference construction

The reference is the accepted content-addressed trajectory and resource
catalog produced by `fsi.remeshing-transfer-2d`; no external file is treated
as a numerical oracle. The export presentation follows the official XDMF
temporal-Collection model, while the HDF5 image uses fixed-size contiguous,
unfiltered datasets with object timestamps disabled.

- XDMF model and format: <https://www.xdmf.org/index.php/XDMF_Model_and_Format>
- HDF5 dataset layout: <https://support.hdfgroup.org/documentation/hdf5/latest/_l_b_dset_layout.html>

The native reader independently audits the generated HDF5 image and retrieves
the hidden MINI bubble. This establishes lossless storage for the registered
profile without making the external formats semantic authority.

