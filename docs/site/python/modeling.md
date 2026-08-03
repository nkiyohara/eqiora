# Modeling and realization

Python authoring produces immutable declarations that close into the same
Rust-owned semantic model as Eqiora Language and Studio.

The current source-tree Python surface includes native builders for bounded scalar `Field`,
`Parameter`, and continuous `Relation` declarations, physical domains and
ports, exact model identity, and one Rust-owned exact
axis-aligned-rectangle-with-circular-hole geometry. That exact family can enter
one explicit, error-controlled chordal reference-mesh operation while the
source remains exact. A typed `Realization` separately selects an admitted
numerical path; choosing FEM or FVM never changes model or geometry meaning.
One explicit-store package operation consumes exact canonical resolution bytes
and a bare root-local Model selector, then returns the ordinary immutable
`Model` with read-only package-compilation lineage. It performs no discovery,
authoring, installation, network access, execution, or Studio workflow.
The package also reaches the common `Result` through explicit resolved Plans
for the accepted exact-cylinder flow, mixed-boundary structure, and two-step
fixed-mesh monolithic FSI cases, with typed application evidence and optional
caller-owned Matplotlib stills.

Start with the complete [five-minute example](../get-started.md), then read
the maintained
[modeling contract](https://github.com/nkiyohara/eqiora/blob/main/docs/python/modeling.md)
for spatial support, revision identity, transaction behavior, supported
expressions, and fail-closed examples.

General vector/tensor authoring, state charts, component declarations, generic
CAD/Boolean builders, production or imported meshing, durable generated-mesh
replay, solve composition for authored geometry, arbitrary realization graphs,
general FSI/ALE, Python time loops, and animation remain outside this alpha's
Python surface.
