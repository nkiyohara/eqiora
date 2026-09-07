# Modeling and realization

Python authoring produces immutable declarations that close into the same
Rust-owned semantic model as Eqiora Language and Studio.

The current source-tree Python surface includes native builders for typed `Field`,
`Parameter`, and continuous `Relation` declarations, physical domains and
ports, exact model identity, and one Rust-owned exact
axis-aligned-rectangle-with-circular-hole geometry. That exact family can enter
one explicit, error-controlled Gmsh meshing operation while the
source remains exact. A resolved `Plan` separately selects an admitted
numerical path; choosing FEM or FVM never changes model or geometry meaning.
One explicit-store package operation consumes exact canonical resolution bytes
and a bare root-local Model selector, then returns the ordinary immutable
`Model` with read-only package-compilation lineage. It performs no discovery,
authoring, installation, network access, execution, or Studio workflow.
The installed distribution also checks that same explicit locked closure under
the exact `eqiora.package.structural-conformance-v1` profile. Its immutable
in-process report records structural replay agreement and exact identities;
even deliberately false scientific documentation can pass. The report proves
no physical truth, numerical correctness, execution support, trust,
certification, evidence lookup, or Studio workflow.
The package also reaches the common `Result` through explicit resolved Plans
for the accepted exact-cylinder flow, mixed-boundary structure, and two-step
fixed-mesh monolithic FSI cases, with typed application evidence and optional
caller-owned Matplotlib stills.

The bounded `eqiora.lang.Source` draft additionally authors one equations-only
Component as deterministic readable `.eqi`. Direct Source compilation and
emitted-file compilation both enter the existing Rust parser, type checker,
lowerer, and Geometry binder; Python owns neither equation meaning nor a second
lowerer. Its immutable `math.pi` expression and `math.sin(...)` operation emit
those exact compiler-owned spellings and leave value and typing semantics to
the native compiler. Equation-structure operators remain in the small implicit
prelude; scalar mathematics lives under reserved `math.*`. The complete
steady-cylinder Stokes Component and scalar Poisson examples use this vocabulary.
Native and Source declarations share
`ValueType`: real or complex scalars with exact physical dimensions, spatial
vectors/tensors, and channel arrays. Spatial axes must match the exact support;
channel axes do not become spatial vectors merely because their extents agree.
Complex execution remains under development.

`doc=` emits attached `///` documentation in the same `.eqi` source. Use a blank
paragraph inside the Python string for further prose; the emitted block remains
attached to its declaration. Documentation changes the source bundle, not the
physical Model.

Numeric declaration initializers inherit an explicitly declared dimension's
coherent unit, so `parameter rate: 1 / s = 1;` needs no repeated unit on the right.
Explicit compatible input units still convert normally; general expressions
do not gain this declaration-only context.

Start with the complete [five-minute example](../get-started.md), then read
the maintained
[modeling contract](https://github.com/nkiyohara/eqiora/blob/main/docs/python/modeling.md)
for spatial support, revision identity, transaction behavior, supported
expressions, and fail-closed examples.

State charts, generic CAD/Boolean builders, production or imported meshing,
durable generated-mesh
replay, solve composition for authored geometry, arbitrary realization graphs,
general FSI/ALE, Python time loops, and animation remain outside this alpha's
Python surface.
