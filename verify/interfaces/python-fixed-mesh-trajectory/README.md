# Installed Python accepted trajectory projection

This case verifies one immutable installed-Python projection of the completely
replayed fixed-mesh 2D Field trajectory accepted by
[`artifacts.general-fixed-mesh-field-trajectory-2d`](../../artifacts/general-fixed-mesh-field-trajectory-2d/README.md).
The first consumer is the existing fixed-reference FSI result; Python does not
reconstruct artifact order, field meaning, units, support, or numerical blocks.

`Trajectory`, `TrajectoryState`, and `FieldSnapshot` are general product names,
but the admitted constructor remains deliberately narrow: fixed-step V1 states
over one affine-triangle 2D mesh. Field values preserve separate Vertex and Cell
coefficient blocks as memoized read-only NumPy arrays. The existing FSI arrays
are checked as derived views of those blocks rather than a second result
authority.

Moving or remeshing trajectories, 3D, single-state spatial results, general
basis metadata, derived quantities, balances, plotting, animation, persistence,
and production scale are not claimed. The complete executable boundary and
pre-committed falsifiers are in [`case.toml`](case.toml).
