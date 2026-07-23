# Ownership model

The modeled inputs are normalized repository-relative changed paths plus the
workflow event kind. The output is a fixed Boolean vector for Rust, MSRV,
Python, Studio, dependency policy, and the isolated CubeCL experiment.

Documentation paths select no heavy surface. Workflow/classifier paths,
unrecognized paths, and manual runs select all surfaces. A separate
protected-base predicate rejects changes to merge/release trust definitions.
