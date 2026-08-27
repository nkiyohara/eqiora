# Installed Python accepted trajectory projection

This case verifies one immutable installed-Python projection of the completely
replayed fixed-mesh 2D Field trajectory accepted by
[`artifacts.general-fixed-mesh-field-trajectory-2d`](../../artifacts/general-fixed-mesh-field-trajectory-2d/README.md).
The first consumer is the common `Result.trajectory` returned after the
accepted fixed-mesh monolithic FSI Plan runs; Python does not reconstruct
artifact order, field meaning, units, support, or numerical blocks.

`Trajectory`, `State`, and `FieldSnapshot` are general product names,
but the admitted constructor remains deliberately narrow: fixed-step V1 states
over one affine-triangle 2D mesh. Field values preserve separate Vertex and Cell
coefficient blocks as memoized read-only NumPy arrays. Typed FSI evidence keeps
its partition and state observations beside this projection rather than
creating a second spatial result authority.

`FieldSnapshot.support_indices(association)` projects the exact global
canonical cells or sorted unique vertex closure of the snapshot's accepted
support Domain. The complete replay derives this membership from the accepted
correspondence and mesh incidence before Python sees it; coefficient values,
field names, and FSI convenience arrays are never membership authorities.
Repeated access on one snapshot returns the same memoized, irreversibly
read-only array. Equal fixed Domain/association occurrences retain equal
membership; cross-occurrence Python object identity is not part of this claim.

Moving or remeshing trajectories, 3D, single-state spatial results, edge/facet
support, general basis metadata, derived quantities, balances, plotting,
animation, persistence, and production scale are not claimed. The complete
executable boundary and pre-committed falsifiers are in
[`case.toml`](case.toml).
