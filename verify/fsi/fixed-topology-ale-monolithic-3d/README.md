# Fixed-topology ALE monolithic FSI in 3D

Status: verified for the bounded serial-host tetrahedral 3D slice below.

This case is the bounded evidence target for [RFC
0070](../../../rfcs/0070-dimension-parametric-tetrahedral-ale-fsi.md). It
extends the accepted 2D ALE contract without introducing a second semantic
lowerer, fluid law, solid law, interface meaning, or geometry-action owner.
Ambient dimension selects typed vector extent, simplex topology, local spaces,
quadrature exactness, physical scaling, and artifact geometry generation.

## Verified evidence

The integration target establishes all of the following:

- direct canonical V5 and exact-package authoring lower to matching complete
  three-dimensional physical roles and coefficients, while accepted 2D
  package-release identities remain unchanged;
- one 15-vertex, 28-tetrahedron conforming fluid/solid partition resolves
  tetrahedral MINI/P1 fluid and vector-P1 solid spaces with positive
  degree-eleven Duffy quadrature;
- accepted solid/interface displacement drives one sealed vector-P1 harmonic
  fluid-mesh action with a genuine unconstrained fluid-interior vertex, and
  every published GeometryState independently replays that relation from the
  complete displacement block rather than trusting supplied coordinates;
- moving tetrahedra satisfy the endpoint metric identity, complete cubic-path
  orientation and quality gates, and an active compatible constant-stream GCL
  probe with an omitted-correction witness;
- complete nonlinear residual reassembly and centered differences check every
  analytic Jacobian column at each accepted step;
- weak incompressibility, solid kinematics, the shared interface velocity
  quotient, and independently recovered interface action and power balance
  close at every accepted step;
- three accepted states publish complete vector velocity, displacement,
  MINI-bubble velocity, scalar pressure, GeometryState, SpatialState, and
  immutable trajectory-prefix evidence;
- `h`, `h/2`, and `h/4` close the bounded first-order temporal-refinement gate
  in one consistent solid reference-mass norm; and
- one checked-in public result asset is projected directly from the accepted
  trajectory and binds exact Model, geometry, mesh, Realization, Run, state,
  and trajectory digests.

The dimension-parametric local element additionally has a unit invariant that
stationary geometry is exactly the fixed-domain local action. The accepted 2D
case remains an independent compatibility gate; neither fact is attributed to
the 3D integration target itself.

Run:

```bash
cargo test --locked -p eqiora --features faer --test fixed_topology_ale_fsi_3d
cargo run --locked -p eqiora-verify -- run --case fsi.fixed-topology-ale-monolithic-3d

# Independent compatibility gate required by RFC 0070:
cargo run --locked -p eqiora-verify -- run --case fsi.fixed-topology-ale-monolithic-2d
```

This case does not claim topology change, tetrahedral remeshing or AMR, curved
or high-order geometry, finite-strain structure, contact, turbulence, free
surface, production mesh smoothing or preconditioning, ALE sensitivity, FSI
adjoints, shape optimization, GPU or MPI ALE, an exact moving-volume energy
identity, performance, or scale.
