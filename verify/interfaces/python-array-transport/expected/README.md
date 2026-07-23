# Expected observations

The descriptor is exactly one-dimensional, native-endian `float64`, dense,
C-contiguous, aligned, read-only, owned, and resident on CPU device 0. Native
Result materialization reports ownership transfer rather than a copy.

NumPy no-copy projections share the exact allocation and cannot become
writable. Explicit NumPy copies and all admitted DLPack exports do not share
Result storage. Unsupported copy, stream, version, and device requests fail
before a capsule is published.
