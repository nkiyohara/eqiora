# Spatial-periodic Cartesian transport FVM 2D

This case closes the first executable path from a typed spatial-periodic
Semantic Connection to a conservative numerical seam. The canonical model
identifies two opposite boundary-physical Ports; it does not contain a mesh,
facet map, donor, tolerance, or duplicated translation vector. The
Realization pairs exact Cartesian facets and reuses the ordinary interior-face
action, so every seam contribution scatters once with equal and opposite sign.

Two independent probes make a dormant or incorrectly oriented seam visible:

- [`models/transverse-inflow.eqi`](models/transverse-inflow.eqi) solves a 2D
  problem that is periodic in x and varies in y. Every periodic row agrees,
  global balance closes, and refinement converges against the independent 1D
  spectral inflow-step oracle with observed order greater than 0.8.
- [`models/seam-advection.eqi`](models/seam-advection.eqi) probes the finalized
  operator with an asymmetric basis vector. The exact cross-seam coefficient
  detects an omitted, duplicated, reversed, or tangentially mispaired packet;
  reversing velocity changes the upwind donor without changing canonical
  connectivity.

Constant preservation, complete facet bijection, old/new mass balance,
exterior-flux exclusion, capability substitution, unsupported minmod, and
non-finite/forged seam inputs are executable falsifiers. V6 wire byte/digest
goldens and v1--v5 decoder rejection live at the artifact boundary rather than
being duplicated here.

Run:

```sh
cargo test --locked -p eqiora-numerics --test canonical_transport spatial_periodic
cargo run --locked -p eqiora-verify -- check \
  --case fluid.cartesian-periodic-transport-fvm-2d
```

This slice does not claim rotational periodicity, nonconforming or unstructured
pairing, periodic minmod reconstruction, vector/tensor transforms, ALE, GPU,
or MPI.
