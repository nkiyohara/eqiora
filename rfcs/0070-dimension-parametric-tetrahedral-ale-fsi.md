# RFC 0070: Dimension-parametric tetrahedral ALE fluid--structure interaction

- Status: Implemented and verified for the bounded serial-host tetrahedral 3D slice
- Authors: Eqiora contributors
- Created: 2026-07-21
- Evidence: [`fsi.fixed-topology-ale-monolithic-3d`](../verify/fsi/fixed-topology-ale-monolithic-3d/README.md)
  (`verified`)
- Depends on: [RFC 0050](0050-fixed-reference-monolithic-fsi.md), [RFC
  0058](0058-portable-realization-and-execution-graphs.md), [RFC
  0064](0064-fixed-topology-ale-fsi.md), and [RFC
  0065](0065-remeshing-correspondence-and-transfer.md)

## Decision

The three-dimensional reference slice is another realization of the same
typed relation network accepted in two dimensions. Ambient dimension selects
the extent of spatial vectors, simplex topology, local spaces, geometric
maps, and dimensional scaling. It does not select another fluid, solid, FSI,
or ALE meaning.

The implementation therefore uses one dimension-parametric ownership chain:

```text
dimension-aware Semantic Model
  -> one fluid / solid / conserving-interface recognizer
  -> one dimension-typed coupled Realization
  -> one affine-simplex fixed-reference operator
  -> one sealed fixed-topology Geometry Action
  -> one monolithic residual and analytic JVP
  -> one accepted moving-state lineage
```

Rust boundaries use a const dimension where it prevents component-count
mistakes. Local basis inventories and matrices remain runtime-sized because
stable Rust 1.85 cannot generally express array lengths such as `D + 1` and
`D * D`. Existing `*2d` public names remain compatibility aliases or thin
entry points; `*3d` names may be entry points, never owners of copied
mathematics.

## Alternatives considered

### Parallel three-dimensional pipeline

Copying the 2D lowerer and ALE modules would be locally fast, but it would
duplicate interface closure, GCL signs, nonlinear acceptance, and artifact
publication. Passing two implementations would not prove that dimension is
orthogonal to meaning. This option is rejected.

### Entirely runtime-dimensional public values

Dynamic vectors and matrices are mathematically uniform and convenient at an
artifact boundary. Using them for every numerical state would defer a
two-versus-three-component error until execution. This option remains correct
for wire and imported-mesh inspection, but is rejected for the typed numerical
shell.

### Const-typed shell with dynamic local algebra

`[f64; D]` retains typed velocity and displacement, while bases, gradients,
and assembled blocks derive their sizes from the admitted simplex. It
preserves the MSRV, permits one 2D/3D implementation, and keeps wire data
runtime-dimensional. This RFC adopts this option.

## Semantic and lowering boundary

The canonical meanings remain:

```text
rho_f * derivative(u_f)
  + div(rho_f * outer_product(u_f, u_f))
  - div(sigma_f) - grad(load_f) = 0
div(u_f) = 0

derivative(d_s) - u_s = 0
rho_s * derivative(u_s) - div(sigma_s) - grad(load_s) = 0

trace(u_f) = trace(u_s)
traction_f + traction_s = 0
```

`spatial_vector` derives its extent from the owning support. The common
recognizer admits one ambient dimension and rejects mismatched Field shapes,
mixed-dimensional subdomains, incomplete box boundaries, or an interface
whose two sides do not share that dimension. It produces the same typed roles
for direct and exact-package authoring.

An ALE node, mesh-velocity Field, tetrahedral Relation, or dimension-specific
semantic dispatch remains forbidden.

## Simplex integration and stable spaces

The fluid uses the MINI pair on an affine simplex ([Arnold, Brezzi, and
Fortin](https://doi.org/10.1007/BF02576171)):

```text
velocity: (continuous P1 + one cell bubble)^D
pressure: continuous P1
```

The solid uses continuous vector P1 velocity and displacement. The interface
quotient identifies the complete P1 velocity trace. This is one
dimension-parametric use of the classical inf-sup-stable MINI construction,
not a tetrahedral stabilization switch.

A positive Duffy--Gauss--Legendre family maps `[0,1]^D` to the unit simplex.
With `n` points per cube axis its declared total-polynomial exactness is
`2n - D`. Required exactness is derived from the admitted local relation, not
from a dimension-specific constant. The fixed-domain transient MINI action
requires at least `2(D + 1)`: eight in three dimensions. The nonlinear ALE
action requires at least `3D + 2`, because its highest-order term is the
product of two MINI velocity factors and one MINI velocity gradient: eleven
in three dimensions. Quadrature is a Realization choice and remains absent
from semantic meaning.

## Dimension-dependent physical scaling

Dimension-independent formulas must not conceal dimension-dependent measure.
For ambient dimension `D`, the common scale contract derives:

```text
volume             ~ L^D
interface measure  ~ L^(D - 1)
nodal action       ~ P L^(D - 1)
interface power    ~ P U L^(D - 1)
energy             ~ P L^D
```

The isotropic small-strain material gate is

```text
mu > 0
lambda + 2 mu / D > 0.
```

These factors are derived from `D`; callers cannot supply a separate 3D scale
profile.

## Fixed-reference numerical core

Partition admission proves that fluid and solid cells form a disjoint and
complete cover of one immutable affine-simplex topology. Every interface
facet has exactly one incident fluid cell and one incident solid cell. Its
oriented witness binds both incident cells and opposite physical normals;
facet identity alone is insufficient in three dimensions.

The common local core owns fixed-domain and ALE views of one MINI
Navier--Stokes action. The fixed view supplies zero mesh velocity, zero
geometry tangent, and zero geometric correction. The ALE view supplies all
three from a sealed Geometry Action. Consequently exact zero motion reduces
by construction to the fixed-domain action rather than merely comparing two
separately transcribed formulas.

The monolithic layout derives its vertex, bubble, pressure, and solid blocks
from `D` and the admitted topology. There is one pressure-gauge contract and
one interface velocity quotient. No 3D-only offsets or interface multiplier
inventory may appear.

## Fixed-topology Geometry Action

Current coordinates are absolute values reconstructed as reference
coordinates plus accepted solid/interface displacement and its sealed fluid
harmonic extension. Consecutive accepted states alone derive mesh velocity.

For every affine simplex,

```text
F(theta) = F_0 + theta (F_1 - F_0),  theta in [0,1]
J(theta) = det(F(theta)).
```

The action evaluates the complete determinant polynomial. In three dimensions
`J` is cubic, so the path-quality gate evaluates both endpoints and every real
root of `dJ/dtheta` in the closed interval. Endpoint positivity is not path
evidence.

At the current endpoint the action independently verifies

```text
dJ/dt = J div_x(w).
```

It derives the current map, mesh velocity, velocity gradient, metric rate,
and GCL correction together. None is an input.

The vector P1 harmonic action is assembled by the dimension-independent
affine-simplex form

```text
K_ij = integral grad(phi_i) . grad(phi_j) dx.
```

Interface displacement is the driver, physical exterior displacement is
zero, and every admitted fluid-interior component is solved and replayed.

## Realization and wire evolution

Existing accepted wire bytes are immutable. A generation whose grammar says
triangle or 2D is not silently widened.

- the typed fixed-topology coupled plan remains dimension-neutral;
- a new Realization generation records the runtime simplex quadrature family
  and dimension without reinterpreting Realization v1--v4;
- a new fixed-topology Geometry State generation records explicit spatial
  dimension and 3D path evidence without reinterpreting Geometry State v1;
- remesh Geometry State v2 remains unchanged because remeshing is outside this
  slice;
- Spatial State and Spatial Trajectory reuse an existing dimension-neutral
  wire only if their replay API can bind the new Geometry State without
  changing accepted bytes; otherwise their next generation is additive.

Mesh, Geometry Identity, geometry--mesh correspondence, FieldSnapshot, Run,
and XDMF topology/geometry contracts are already runtime-dimensional and are
reused.

## Acceptance and evidence

The registered evidence closes one small conforming tetrahedral fluid/solid
topology with a complete shared triangular interface and a genuine fluid
interior motion solve. It must demonstrate:

- direct and exact-package authoring lower through the same complete 3D roles;
- the stable tetrahedral MINI/P1 and solid P1 spaces are resolved explicitly;
- zero motion is the fixed-domain local action;
- every moving tetrahedron satisfies the endpoint metric identity and remains
  positive over its entire linear path;
- the compatible constant free stream closes with GCL, while omitting the GCL
  term produces a nonzero registered witness;
- current coordinates replay from absolute solid displacement and the sealed
  harmonic action, while mesh velocity replays only as the consecutive-state
  quotient;
- Geometry State publication resolves the exact displacement leaves, rebuilds
  the same solver-free P1 harmonic relation from correspondence and
  Realization roles, and rejects a positive coordinate payload that is not its
  admitted projection;
- every analytic nonlinear Jacobian column agrees with centered complete-
  residual reassembly;
- weak incompressibility, solid kinematics and momentum, shared interface
  velocity, opposite interface action, and interface power balance close;
- at least three accepted states publish complete vector velocity,
  displacement, bubble, scalar pressure, Geometry State, Spatial State, and
  immutable trajectory-prefix evidence;
- `h`, `h/2`, and `h/4` demonstrate bounded first-order temporal refinement in
  one reference-topology mass norm; and
- the accepted 2D evidence passes unchanged beside the 3D case.

The public result asset is derived from the accepted trajectory. It uses
tetrahedral connectivity, XYZ coordinates, and exact Run/trajectory
provenance; a renderer-owned reconstruction is not evidence.

## Required falsifiers

Admission or execution rejects:

- a 3D Domain with a two-component spatial Field, incomplete `z` boundary,
  or a mixed-dimensional coupled subdomain;
- missing, duplicate, extra, or same-oriented interface facets;
- an endpoint-positive tetrahedral path that inverts between endpoints;
- a modified harmonic influence coefficient, unconstrained fluid component,
  discontinuous interface displacement, or positive but non-harmonic current
  coordinate payload;
- substituted displacement block content or a mismatch between the immutable
  mesh quality gate and the ALE Realization quality policy;
- caller-authored mesh velocity, a substituted predecessor, driver snapshot,
  Model, mesh, correspondence, Realization, or Geometry State;
- an omitted GCL correction;
- a 2D interface/energy scale reused in 3D;
- a copied 3D lowerer, dimension-specific semantic node, duplicate pressure
  gauge, or fallback to fixed-domain assembly;
- nonlinear nonconvergence, incomplete Jacobian differentiation, or
  publication before all acceptance checks pass.

## Nonclaims

This RFC does not claim topology change, tetrahedral remeshing or AMR, curved
or high-order geometry, finite-strain solid mechanics, contact, turbulence,
free surface, production mesh smoothing, ALE sensitivity, FSI adjoint, shape
optimization, GPU/MPI ALE, a production preconditioner, performance, or scale.
