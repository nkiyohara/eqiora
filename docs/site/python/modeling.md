# Modeling and realization

Python authoring produces immutable declarations that close into the same
Rust-owned semantic model as Eqiora Language and Studio.

The public alpha includes native builders for bounded scalar `Field`,
`Parameter`, and continuous `Relation` declarations, physical domains and
ports, exact model identity, and one Rust-owned exact
axis-aligned-rectangle-with-circular-hole geometry. A typed `Realization`
separately selects an admitted numerical path; choosing FEM or FVM never
changes model or geometry meaning.

Start with the complete [five-minute example](../get-started.md), then read
the maintained
[modeling contract](https://github.com/nkiyohara/eqiora/blob/main/docs/python/modeling.md)
for spatial support, revision identity, transaction behavior, supported
expressions, and fail-closed examples.

General vector/tensor authoring, state charts, component declarations, generic
CAD/Boolean builders, meshing and solve composition for authored geometry, and
arbitrary realization graphs remain outside this alpha's Python surface.
