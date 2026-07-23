# Models

`two-boundaries.eqi` is the local canonical fixture. `BoundaryState` declares a
two-dimensional Volume slot and a Boundary slot whose exact parent is that
Volume. Its scalar Field is defined on the Volume; its Relations exercise the
Volume, boundary trace, and coordinate-aware expression contracts.

`Coupled` binds two occurrences to the same `fluid` Volume and to distinct
`left` and `right` boundaries. The fixture is deliberately free of mesh,
Realization, solver, and FSI content so that the observed graph shape belongs
only to semantic support elaboration.

The integration test constructs the exact-package sources in Rust. Keeping the
small package pair beside the assertions makes the two dependency-alias
spellings and the additional wrapper forwarding level explicit without
duplicating generated release or lock artifacts here.
