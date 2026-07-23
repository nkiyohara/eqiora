# Expected evidence

The admitted source resolves the same normalized two-triangle unit-square mesh
and node/cell fields as the independent caller-resolved XDMF case. The native
manifest additionally records the exact Rust binding and observed static HDF5
runtime, so it is intentionally not identical to the caller-resolved manifest.

Every `reject-*.h5` image must fail with the stable external-import diagnostic
before a value batch or accepted artifact is returned. Exact checked-in source
digests are recorded in `source.sha256`; values and artifact identities are
recomputed through the public integration path.
