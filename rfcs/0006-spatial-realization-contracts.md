# RFC 0006: Spatial realization contracts

- Status: Draft implementation
- Authors: Eqiora contributors
- Created: 2026-07-17

## Summary

Spatial realization is factored into four independent contracts: an
arbitrary-dimensional oriented mesh, reference-cell quadrature, pure
entity-local operators, and constraint-aware local-to-global assembly.

## Motivation

Eqiora must support finite elements, finite volumes, discontinuous Galerkin,
mimetic methods, and future matrix-free backends without making any one of
them the canonical meaning of a `Field` or `Relation`.

Two seemingly attractive decompositions fail this requirement:

1. A cell-only operator makes FEM volume terms convenient but hides the
   interior/boundary face fluxes that are native to FVM and DG.
2. A global variational-form contract is elegant for FEM but forces FVM to be
   translated into a foreign abstraction before its conservative fluxes can
   be inspected.

The common native object is instead a contribution local to an oriented mesh
entity. It may be evaluated on a cell, interior facet, or boundary facet and
then mapped independently into global algebra.

## Current best formulation

```text
Mesh       = entity strata + oriented incidence + geometry map
Quadrature = reference cell + points + weights + exactness
LocalOp    = (entity context, quadrature) -> anonymous local contribution
Assembly   = local map + contribution -> global algebra
```

The contracts are realization data. They do not add Semantic Kernel nodes and
do not change the meaning of an implicit Relation.

## Mesh contract

`MeshEntity` is identified by `(topological_dimension, local_index)`. A
`MeshTopology` reports the mesh dimension, entity counts, and oriented
incidence in either direction. Each incidence carries:

- the incident entity;
- the lower-dimensional entity's local ordinal in the containing reference
  cell, independent of query direction;
- a stable orientation/permutation code.

The orientation code is interpreted with the reference-cell topology. Code
zero is identity. This leaves room for the permutations required by vector
finite elements and high-order traces instead of reducing orientation to a
single normal sign.

`MeshGeometry` is separate from topology. It supplies a `GeometryMap` for an
entity. The map declares its reference cell and physical embedding dimension,
maps reference coordinates to physical coordinates, and returns a row-major
`geometric_dimension x topological_dimension` Jacobian into caller-provided
storage.

Topological and geometric dimensions are runtime values. This is necessary
for imported and mixed-cell artifacts to remain inspectable before backend
lowering. A compiled backend may specialize validated dimensions and cell
families later.

V0 includes `LineMesh`, whose cells are affine segments. It is one concrete
implementation of the arbitrary-dimensional contract, not the definition of
a mesh.

### Reference topology and orientation

The runtime-dimensional reference topology is now explicit. A
`ReferenceTopology` generates simplex strata from non-empty vertex subsets and
hypercube strata from fixed-lower, fixed-upper, and free axis states. Entity
counts, closure vertex references, and allocations are checked before
construction. Incidence can be queried in either direction and returns the
same lower-entity local ordinal.

`VertexPermutation` is the single validated orientation representation shared
by reference topology and discrete spaces. It retains the image of every
canonical vertex ordinal and implements identity, composition, and inverse.
Reducing this value to a sign would lose the information required by future
high-order and vector traces; compact backend `OrientationCode` values may
intern, but do not replace, the inspectable permutation contract.

`AffineGeometryMap` realizes `x = origin + J ξ` with an explicit rectangular
`geometric_dimension x topological_dimension` Jacobian. It therefore covers
embedded lines and surfaces without padding a fictitious square map. Its
unsigned physical measure is computed from the Jacobian column volume
`sqrt(det(J^T J))` through a rank-revealing orthogonalization; degenerate and
non-finite maps fail construction.

## Quadrature contract

`ReferenceCell` carries a runtime dimension and one of three families:
point, simplex, or hypercube. Custom rules work in any valid dimension;
tensor-product Gauss–Legendre rules are constructed directly for
`[-1,1]^d`. Every `QuadratureRule` owns:

- its reference cell;
- ordered runtime-dimensional points;
- finite weights with positive total measure;
- optional declared polynomial exactness.

Quadrature remains an explicit policy supplied to a local operator. It is not
hidden inside a mesh, global assembler, or physics relation.

In addition to tensor-product Gauss--Legendre rules, v0 provides the centroid
rule on a unit simplex of arbitrary representable runtime dimension. Its
`1/d!` weight and degree-one exactness are explicit, and combinatorial/resource
checks precede allocation.

## Discrete-space contract

`DiscreteSpace` owns only reference-cell-local basis and degree-of-freedom
facts. It exposes topological `LocalDof` descriptors, value/reference-gradient
tabulation, and orientation of local DOF order through the same validated
`VertexPermutation`. It has no global sparse index, constraint, solver, Field
meaning, or physical unit.

V0 provides cell-constant P0 on every reference-cell family, simplex nodal P1,
and hypercube nodal Q1 in runtime dimension. Their tests require partition of
unity, zero gradient sum, and the nodal Kronecker property where applicable.
Physical gradients remain a later composition with `GeometryMap`; basis
tabulation does not absorb geometry ownership.

## Local-operator contract

`LocalOperator<Context>` is pure:

```text
evaluate(context, quadrature) -> LocalContribution
```

`LocalContribution` is a finite dense row-major matrix plus local right-hand
side. Its rows and columns are anonymous. It contains no global DOF indices,
constraints, sparse-matrix handles, or backend state.

`Context` is method-owned and may represent a cell, an interior facet, or a
boundary facet. The mesh and quadrature contracts remain method-neutral.

## Assembly contract

`AssemblyMap` gives each local row either a global equation or no equation,
and gives each local column either a free global DOF or a fixed finite value.
The assembler therefore performs affine essential-boundary elimination by
moving fixed-column terms to the right-hand side; local operators do not know
about global constraints.

`CooAssembler` transactionally accumulates mapped contributions in a
deterministic coordinate map and finalizes sorted CSR storage. A failed
scatter leaves the assembler unchanged. This is a reference implementation,
not a commitment to COO, CSR, serial execution, or assembled matrices. Other
assemblers may consume the same two input contracts for distributed, block,
partial, or matrix-free realization.

## Poisson comparison

The same manufactured problem is solved on the same uniform mesh revisions:

```text
-u'' = pi^2 sin(pi x),  x in (0, 1)
u(0) = u(1) = 0
u_exact = sin(pi x)
```

The FEM realization uses continuous piecewise-linear basis functions. Its
cell operator integrates stiffness and load, and vertex Dirichlet values are
handled by `AssemblyMap`.

The FVM realization uses cell-centered unknowns. Cell operators integrate the
source; interior and boundary facet operators contribute conservative
two-point diffusive fluxes. They use the same assembler without pretending
that a face flux is a cell variational form.

Both sparse systems use the same deterministic conjugate-gradient verification
oracle. For comparable continuous `L2` evidence, FEM uses its native linear
field and FVM exposes a declared linear reconstruction through boundary and
cell-center values. The raw unknown locations remain available and distinct.

## Alternatives considered

### Compile-time dimension everywhere

Const-generic arrays provide strong local typing, but force imported mesh
dimensions and heterogeneous artifact data into monomorphized types too
early. Rejected as the artifact contract. Backend IR may still specialize.

### Runtime dimension everywhere, including lowered kernels

Simple to serialize, but would permanently impose dynamic indexing on CPU/GPU
kernels. Rejected. Runtime realization data and dimension-specialized lowering
are separate layers.

### Variational form as the shared local contract

Mathematically natural for FEM, but not method-neutral for conservative FVM
face fluxes. Rejected as the common denominator; a future form language may
lower into local operators.

### Cell-only residual callbacks

Compact but unable to express interior-facet coupling without hidden neighbor
access and double counting. Rejected.

## Compatibility and migration

This pre-alpha change replaces the private duplicate `UniformLine` used by
`Diffusion1d`; transient diffusion now consumes the same validated `LineMesh`
as Poisson. The Semantic Kernel and source language are unchanged.

The contract deliberately defers wire schemas and canonical spatial lowering.
Those additions must preserve the four boundaries above or document a
superseding RFC.

## Verification

- Reject invalid mesh geometry and reference-cell/quadrature combinations.
- Exercise a five-dimensional tensor-product quadrature rule.
- Verify oriented line-cell incidence and geometry mapping.
- Reject malformed/non-finite local contributions.
- Verify fixed-value elimination and out-of-range assembly maps.
- Preserve atomic assembly on failed accumulation.
- Solve one SPD system through finalized CSR and conjugate gradients.
- Solve the same Poisson problem with FEM and FVM for 8, 16, 32, 64, and 128
  cells.
- Require monotonically decreasing continuous `L2` error and observed order
  above 1.9 for both methods in CI.

The reproducible results are recorded in
[`docs/verification/poisson-fem-fvm.md`](../docs/verification/poisson-fem-fvm.md).

## Research basis

The boundary agrees with established primary implementations while avoiding
adopting any one package's full object model:

- [MFEM integration](https://mfem.org/integration/) separates reference
  integration rules, geometry transformations, local integrators, and global
  forms.
- [MFEM code overview](https://mfem.org/code-overview/) separates mesh,
  element-to-DOF mappings, local integrators, and constraint transformations.
- [deal.II MeshWorker](https://www.dealii.org/current/doxygen/deal.II/namespaceMeshWorker.html)
  distinguishes cell, boundary, face workers, and generic assembly.
- [PETSc DM](https://petsc.org/release/manual/dmbase/) separates mesh/discrete
  layout, local/global vectors, and additive assembly.
- [OpenFOAM numerical schemes](https://www.openfoam.com/documentation/user-guide/6-solving/6.2-numerical-schemes)
  make face interpolation and surface-normal flux choices explicit for FVM.
- [FEniCSx Basix quadrature](https://docs.fenicsproject.org/basix/main/python/_autosummary/basix.quadrature.html)
  ties points, weights, and exactness to a reference cell.

## Security, safety, and governance

The implementation uses safe Rust. Public constructors validate dimensions,
finite values, topology bounds, allocation overflow, and local/global shape
agreement. Diagnostic codes remain append-only. This draft does not establish
a project-wide default FEM/FVM choice or canonical PDE support.

## Unresolved questions

- Stable mesh, quadrature, and sparse-artifact schemas.
- High-order/vector orientation transforms and curved facet embeddings.
- Curved/high-order geometry maps and mixed-cell validation.
- Vector/tensor spaces, higher polynomial orders, and hanging-node constraints.
- Canonical `Field`/`Representation` to spatial Operator IR lowering.
- Partial/matrix-free assembly and device-memory contracts.
- Multidimensional Poisson evidence on simplex and tensor-product meshes.
