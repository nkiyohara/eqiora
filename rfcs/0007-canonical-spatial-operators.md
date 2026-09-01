# RFC 0007: Canonical spatial operators and interval realization

- Status: Draft implementation
- Authors: Eqiora contributors
- Created: 2026-07-17

## Summary

Eqiora expresses continuous spatial meaning through runtime-dimensional
Cartesian `Domain` definitions, continuum `Representation` definitions, and
four shape-aware expression operators: `grad`, `div`, `trace`, and `normal`.
`coordinate(axis)` and dimension-aware unary mathematics express spatially
varying data without a discretization callback. Strong-form Relations remain
canonical. A separate lowering recognizes a strict scalar elliptic subset and
selects P1 finite-element or cell-centered finite-volume realizations on
one-dimensional intervals.

This introduces no Semantic Kernel node kind. Spatial mechanics, thermal
diffusion, and other physics continue to be compositions of the existing nine
nodes.

## Motivation

RFC 0006 fixed the realization boundary:

```text
Mesh + GeometryMap + Quadrature -> LocalOperator -> LocalContribution
                                             + AssemblyMap -> global algebra
```

That contract deliberately could not say what continuous equation a mesh was
realizing. Consequently, a correct Poisson FEM/FVM comparison remained a
numerical precursor rather than a canonical Eqiora model. The missing layer
must satisfy four constraints:

1. finite elements and finite volumes must start from the same model meaning;
2. physical dimensions and tensor shapes must be checked before realization;
3. essential and natural boundary meaning must remain explicit;
4. an unsupported form must fail rather than silently acquire a method-owned
   interpretation.

The first roadmap case, `solid.axial-bar`, is the smallest model that exercises
all four constraints while also verifying a derived field and a reaction.

## Current best formulation

```text
Canonical:
  Cartesian Domain + boundary Domains
  continuum Representation
  Field defined on (Domain, Representation)
  strong implicit Relations using grad/div/trace/normal
  relation-scoped coordinate(axis) + dimension-aware unary mathematics

Lowered semantic form:
  -div(k grad(u)) = f
  trace(u) = prescribed value
  normal(k grad(u)) = prescribed outward flux

Default realization, when selected:
  affine LineMesh + P1 space + Gauss quadrature
  entity-local volume/boundary contributions + AssemblyMap
  deterministic CSR + conjugate-gradient verification solve
```

The strong relation is method-neutral. FEM derives its weak volume and boundary
contributions; FVM may derive conservative facet fluxes. Neither discrete form
becomes canonical.

## Domain and Representation contract

`DomainKind::CartesianBox` owns finite increasing coordinate bounds in coherent
SI length units. Its dimension is the runtime number of coordinate axes.
`DomainKind::CartesianBoundary` selects an axis and an outward lower/upper side;
exactly one `BoundaryOf` edge identifies its parent box.

Continuous geometry is model meaning. Mesh topology, geometry maps, quadrature,
basis choice, and DOF layout remain realization data.

A distributed scalar Field has exactly one `DefinedOn` edge to a Cartesian box
and one to a continuum Representation. Existing abstract Domains and lumped
Fields remain valid and retain no implicit spatial support.

## Expression type contract

Expression validation tracks three independent properties:

```text
ExpressionType = physical dimension + value shape + support Domain
Value shape    = scalar | spatial tensor(rank)
```

- `grad` appends one spatial axis and divides the physical dimension by length.
- `div` contracts the final spatial axis and divides by length.
- `trace` restricts an expression from a parent volume to the owning boundary
  Relation's Domain.
- `normal` contracts one spatial axis against that boundary's outward normal.
- `coordinate(axis)` is a scalar length supported on the owning Relation's
  Cartesian Domain; the axis is validated against that Domain at runtime.
- `sin` requires a dimensionless scalar and preserves its Domain support.
- addition and subtraction require equal dimensions, shapes, and support;
  v0 multiplication requires at least one scalar operand; division requires a
  scalar denominator.
- every Relation root is scalar and its support equals the Relation's optional
  `AppliesOn` Domain.

The rules intentionally match standard tensor conventions without importing a
finite-element form language. The current public Field payload is scalar; the
intermediate shape contract is already rank-aware so vector/tensor Fields can
be added without redefining `grad` or `div`.

## Eqiora Language surface

The spatial source slice is deliberately small:

```text
domain bar = box(0, 2);
domain fixed = boundary(bar, axis = 0, side = lower);
representation body = continuum;
field u on bar as body: m = 0;

relation equilibrium continuous on bar {
  -div(E * A * grad(u)) = 0;
}
relation clamp continuous on fixed {
  trace(u) = 0;
}
```

Coordinate-dependent data remains equally explicit:

```text
parameter wave_number: 1 / m = 3.141592653589793;
parameter source_scale: 1 / m ^ 2 = 9.869604401089358;
relation balance continuous on bar {
  -div(grad(u)) - source_scale * math.sin(wave_number * coordinate(0)) = 0;
}
```

Box coordinates are coherent SI metres. Rich coordinate charts, CAD regions,
and imported geometry require later schema work; they are not encoded as
unchecked strings.

## Default scalar elliptic lowering

The first lowerer accepts exactly:

- one 1D Cartesian volume Domain;
- one scalar continuum Field on that Domain;
- one volume Relation of the form `-div(k grad(u)) - f = 0`;
- one explicit essential or natural Relation on each endpoint;
- constant and Parameter expressions for `k` and boundary data;
- scalar arithmetic, `coordinate(0)`, and supported unary mathematics for
  `f`, lowered to one immutable method-neutral evaluation tape.

It rejects ambiguity, unsupported expression structure, non-positive
coefficients, pure-Neumann nullspaces without a gauge, and boundary fluxes that
do not use the same constitutive expression as the volume Relation.

The default realization is continuous P1 FEM on a generated affine `LineMesh`.
The mesh resolution and solver tolerance are explicit configuration. Essential
values pass only through `AssemblyMap`; natural fluxes are boundary-local right-
hand-side contributions. Reactions are recovered as residuals of the complete
uneliminated equilibrium system.

## Alternatives considered

### Canonical variational form

A global weak form is elegant for Galerkin FEM but makes conservative FVM face
fluxes pass through a method-foreign abstraction. Rejected as canonical model
meaning. A variational-form frontend may still lower to the same strong
Relation network or directly to a chosen realization.

### Physics- or equation-specific Relation payload

An `ElasticBar`, `Poisson`, or generic-looking `EllipticProblem` descriptor
would make this case smaller, but each new equation family would expand a
parallel semantic type hierarchy. Rejected because it recreates example-
specific kernel nodes inside a payload.

### Implicit boundary restriction

Automatically interpreting a volume Field inside a boundary Relation as a
trace saves one operator but hides an important Sobolev-space obligation.
Rejected. `trace` and `normal` make restriction and outward orientation
inspectable.

## Verification

The canonical `solid.axial-bar` case executes this complete path:

```text
.eqi source -> typed transaction -> immutable KernelProgram
            -> scalar elliptic lowering -> default P1 realization
            -> displacement, stress, and reaction evidence
```

For `E = 200 GPa`, `A = 0.01 m^2`, `L = 2 m`, and `P = 10 kN`, CI checks:

- tip displacement `P L / (E A) = 1e-5 m`;
- constant stress `P / A = 1e6 Pa` on every cell;
- fixed-end reaction `-P = -10000 N`;
- essential/natural boundary classification and the lowered coefficient;
- execution through the machine-readable case evidence runner.

The exact linear solution is reproduced to floating-point tolerance on the P1
space. This case therefore verifies semantics, units, boundary handling,
derived stress, equilibrium reaction, and default realization selection; it
does not establish multidimensional elasticity or convergence on a nontrivial
solution.

The canonical `numerics.poisson-fem-fvm` case closes that convergence gap:

```text
.eqi source -> typed coordinate/math DAG -> immutable KernelProgram
            -> one scalar elliptic model + one source tape
            -> P1 FEM local cells -------+
            -> TPFA FVM cells/facets ----+-> common L2/balance evidence
```

For `u(x) = sin(pi x)` on `[0, 1]`, both continuous evidence views converge at
observed order greater than `1.9` over 8–128 cells. FEM endpoint reactions and
FVM outward boundary fluxes independently balance the quadrature-integrated
source to relative error below `2e-12`. The exact solution is reference
evidence, not a second model or an input to either solve.

## Red-team notes

- The sine Poisson case now exposes the convergence defect that the exact
  linear axial case could not. It remains a uniform orthogonal 1D problem and
  says nothing about skew-mesh consistency, non-orthogonal flux correction, or
  multidimensional orientation transforms.
- The current lowerer recognizes a deliberately narrow expression shape. It is
  evidence that the canonical operators compose, not a general symbolic PDE
  compiler claim.
- Matching the volume and boundary constitutive coefficient by evaluated value
  is sufficient for this immutable constant case, but future nonlinear or
  field-valued constitutive expressions need structural/provenance identity.
- Reaction sign follows the residual convention of the complete equilibrium
  system. Every future structural case must state its outward-normal and load
  sign conventions explicitly.

## Research basis

- [UFL form-language operators](https://docs.fenicsproject.org/ufl/2025.2.0.post0/manual/form_language.html)
  provide explicit tensor shapes, `SpatialCoordinate`, scalar nonlinear
  functions, and standard `grad`/`div` conventions. Eqiora adopts those
  mathematical conventions, not UFL's discrete variational form as canonical
  meaning.
- [MFEM boundary-condition documentation](https://mfem.org/fem_bc/) separates
  essential elimination from natural/Neumann boundary integrals and states the
  continuous boundary operator associated with `-div(k grad(u))`.
- [PETSc boundary-condition types](https://petsc.org/release/manualpages/DM/DMBoundaryConditionType/)
  distinguish essential and natural conditions across discretizations.
- [PETSc `DMFieldContinuity`](https://petsc.org/release/manualpages/DM/DMFieldContinuity/)
  distinguishes vertex-continuous finite-element data from cell-local
  finite-volume data. Eqiora likewise shares model meaning and artifact
  contracts without forcing the two realizations into one DOF layout.

## Compatibility and migration

The new Domain and Representation variants are non-exhaustive. Existing
`DomainDef::new` and `RepresentationDef::new` remain abstract. Existing lumped
Fields, scalar Relations, interpreter behavior, and Operator IR are unchanged.
Scalar Operator IR deliberately rejects spatial expression nodes; the spatial
lowerer owns them instead of pretending they are pointwise scalar operations.

## Unresolved questions

- vector/tensor Field storage and constitutive tensor algebra;
- general coordinate charts, embedded manifolds, CAD/implicit regions, and
  boundary-set schemas;
- multidimensional meshes, spaces, orientation transforms, and evidence;
- coordinate-dependent or Field-valued coefficients and source Fields in the
  default lowerer;
- Robin, interface, periodic, inequality, and weakly imposed conditions;
- pure-Neumann gauges and nullspace-aware solvers;
- Realization Graph payload schemas and explicit override of default policy.
