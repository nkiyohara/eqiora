# RFC 0084: Standalone prescribed dynamic-solid artifacts

- Status: Draft
- Authors: Eqiora contributors
- Created: 2026-08-04
- Depends on: [RFC 0013](0013-realization-and-run-provenance-wire.md),
  [RFC 0048](0048-dynamic-linear-solid-semantics.md), and
  [RFC 0051](0051-durable-spatial-state-and-trajectory.md)
- Reference evidence:
  [`solid.prescribed-dynamic-solid-step-3d`](../verify/solid/prescribed-dynamic-solid-step-3d/README.md)

## Summary

Eqiora will represent one exact serial-host execution of the accepted
prescribed-displacement three-dimensional dynamic-solid reference as one new
content-addressed standalone-solid Realization, two existing fixed-mesh V1
spatial States, and one existing V2 Run whose sole output is the accepted-next
State.

The new durable type is
`PrescribedDynamicSolidRealizationEnvelopeV1`, with schema
`eqiora.prescribed-dynamic-solid-realization-envelope/v1`. The one new
non-wire application owner is `PrescribedDynamicSolidStateRun3d`. No
FieldSnapshot, State, Run, trajectory, checkpoint, request, or result schema is
added.

## Motivation

The accepted numerical path currently ends at an opaque in-memory
`AcceptedPrescribedDynamicSolidStep3d`. Its generation, displacement,
velocity, acceleration, reactions, operators, residuals, assembly report, and
solve report prove one bounded step, but they do not give that execution a
durable Realization identity or a Run output that another process can resolve.

Existing Realization generations cannot be reused honestly:

- `RealizationEnvelopeV3` requires multiple coupled Domains and one exact
  conforming trace quotient;
- `RealizationEnvelopeV4` additionally requires fixed-topology ALE meaning;
  and
- `RealizationEnvelopeV5` retains the same ALE graph with a
  dimension-explicit quadrature extension.

A dummy fluid Domain, fake trace quotient, or inert ALE graph would turn
absent physics into durable meaning. `RealizationArtifactReference` is also
insufficient: it is a non-wire projection of an already durable Realization,
not independently decodable bytes.

The existing `DiscreteFieldEnvelopeV1`, `FieldSnapshotEnvelopeV1`, fixed-mesh
`SpatialStateEnvelopeV1`, and `RunManifestV2` already carry the required
logical values and content links. The missing invariant is therefore one
standalone-solid Realization family and one application owner that makes the
complete accepted publication atomic.

## Bounded claim

This RFC admits exactly the reference occurrence already verified by
`solid.prescribed-dynamic-solid-step-3d`:

- one current canonical Model that replays as the accepted unit-cube,
  `rho = 2 kg/m^3`, `mu = 3 Pa`, `lambda = 0 Pa`, zero-load, first-order
  three-dimensional dynamic solid;
- the exact ordered nine-vertex, twelve-positive-tetrahedron imported mesh and
  its exact Geometry identity and Geometry-to-Mesh correspondence;
- the Geometry identity's exact `1e-12 m` classification tolerance and the
  imported mesh's exact `0.1` minimum-mean-ratio admission gate;
- the solid body Domain, displacement and velocity Fields, fixed `x = 0`
  boundary, and live driven `x = 1` boundary derived from that Model;
- complete canonical prior displacement and velocity at step zero;
- the accepted total driven displacement at `t[n+1]`, not an increment or a
  velocity;
- continuous vector P1, exact affine-tetrahedron mass and stiffness
  integration, and a `0.25 s` backward-Euler step;
- the frozen conjugate-gradient, Identity-preconditioned, Reproducible
  reduction policy; and
- one replicated, offline, one-worker serial-host assembly, solve, and
  verification occurrence.

The numerical formulation, expected values, tolerances, mesh order, candidate
order, and acceptance meaning remain owned by the accepted reference case.
This RFC only gives that unchanged occurrence durable identity and lineage.

## Standalone-solid Realization wire

### Public owner and schema

`PrescribedDynamicSolidRealizationEnvelopeV1` is the only new public artifact
type. Its exact schema identifier is:

```text
eqiora.prescribed-dynamic-solid-realization-envelope/v1
```

Its canonical encoding is `eqiora.canonical-json/v1`. Private wire DTOs deny
unknown fields. No public wire DTO, enum, builder, option bag, registry, or
general standalone-solid trait is introduced.

### Exact canonical grammar

The top-level keys occur in this order:

```text
schema
encoding
model_sha256
model_ulid
semantic_revision
source
geometry_sha256
correspondence_sha256
spatial
time
driven_total_displacement
solver
placement
```

The complete closed logical grammar is:

```text
schema                  "eqiora.prescribed-dynamic-solid-realization-envelope/v1"
encoding                "eqiora.canonical-json/v1"
model_sha256            ArtifactDigest
model_ulid              canonical typed Model ULID
semantic_revision       u64
source                  {
    kind                "explicit"
    realization_revision u64
}
geometry_sha256         ArtifactDigest
correspondence_sha256   ArtifactDigest
spatial                 {
    spatial_dimension   3
    scalar              "f64"
    vector_layout       "replicated"
    solid_domain_ulid   canonical Domain ULID
    displacement_field_ulid canonical Field ULID
    velocity_field_ulid canonical Field ULID
    fixed_boundary_ulid canonical Domain ULID
    driven_boundary_ulid canonical Domain ULID
    space               {
        kind            "continuous-lagrange"
        order           1
    }
    discretization      {
        method          "continuous-galerkin"
        mesh            {
            kind        "imported-simplicial"
            artifact_sha256 ArtifactDigest
        }
        quadrature      "exact-affine-p1-tetrahedron-mass-and-stiffness"
    }
}
time                    {
    method              "backward-euler"
    duration_s          0.25
}
driven_total_displacement [
    {
        vertex_index    u64
        value_m         [f64, f64, f64]
    }
]
solver                  {
    operator_properties "symmetric-positive-definite"
    algorithm           "conjugate-gradient"
    preconditioner      "identity"
    reduction           "reproducible"
    relative_tolerance  1e-13
    absolute_tolerance  1e-15
    maximum_iterations  500
}
placement               {
    target              {
        kind            "host-cpu"
        threads         1
    }
    schedule            {
        kind            "offline"
    }
    assembly_execution  "host-serial"
    solve_execution     "host-serial"
    verification_execution "host-serial"
    layout_artifacts    {
        kind            "replicated"
    }
}
```

The nested object keys occur in the order shown. The driven list contains
exactly the canonical driven-boundary vertex inventory from the accepted mesh:

```text
vertex_index 1, value_m [0.015, 0.0, 0.0]
vertex_index 3, value_m [0.015, 0.0, 0.0]
vertex_index 5, value_m [0.015, 0.0, 0.0]
vertex_index 7, value_m [0.015, 0.0, 0.0]
```

The list is ordered by exact mesh vertex index, is unique, and must equal the
driven vertex inventory reconstructed from the correspondence. Values use
canonical finite binary64 spelling. Negative zero is rejected. The listed
values, solver policy, geometry tolerance, mesh-quality gate, and solver
tolerances are copied from the accepted reference; they are not new scientific
assertions.

Geometry and correspondence digests are present because the unchanged V1
FieldSnapshot and State wires name them. Omitting them from the Realization
would make the application owner reconstruct two lineage edges from ambient
caller convention. The Geometry identity must satisfy
`geometry.tolerance_m().to_bits() == (1.0e-12_f64).to_bits()`, and the mesh
artifact must satisfy
`mesh.mesh().quality_gate().minimum_mean_ratio().to_bits() == (0.1_f64).to_bits()`.
Those constants are already members of the Geometry and mesh canonical bytes;
their referenced digests make both constants digest-bearing in this
Realization without duplicating either value in its wire. Changing either
constant changes its owner artifact's digest, makes the recorded edge stale,
and changes this Realization's canonical bytes and digest. Material values are
not duplicated because their authority is the exact Model artifact.

### Canonical bytes and identity

Canonical bytes are compact UTF-8 JSON emitted directly from the closed wire
DTOs in declaration order. There is no whitespace, map-order dependence,
optional member, null member, or default elision. Digests use lowercase
64-character hexadecimal spelling and ULIDs use their canonical spelling.

The Realization identity is:

```text
SHA-256(
  UTF-8("eqiora.prescribed-dynamic-solid-realization-envelope/v1")
  || 0x00
  || canonical_json
)
```

Every top-level and nested member is in the digest domain. Changing Model,
revision, Geometry, correspondence, mesh, role identity, driven value,
discretization, time integration, solver, or placement changes the
Realization digest.

### Construction and external validation

The application path constructs the envelope only after the exact reference
has returned `AcceptedPrescribedDynamicSolidStep3d`. The artifact constructor
accepts the exact Model, Geometry, correspondence, mesh, explicit Realization
revision, and driven values. It derives every semantic role from the bound
Model; no Domain or Field role is a caller assertion. Every numerical and
placement policy literal is internal and fixed; it is not a caller option. The
artifact crate does not depend on the numerics crate and therefore does not
accept or reconstruct `AcceptedPrescribedDynamicSolidStep3d`.

The public artifact surface is exactly this constructor, decoder, canonical
identity API, role projection, and external validation API:

```rust
pub fn new(
    model: &impl ReplayableCanonicalModelArtifact,
    geometry: &GeometryIdentityEnvelopeV1,
    correspondence: &GeometryMeshCorrespondenceEnvelopeV1,
    mesh: &SimplicialMeshEnvelopeV1,
    realization_revision: RealizationRevision,
    driven_total_displacement: &[(VertexId, [f64; 3])],
) -> Result<Self, Diagnostic>;

pub fn from_json(
    bytes: &[u8],
    limits: RealizationDecoderLimits,
) -> Result<Self, Diagnostic>;
pub fn canonical_json(&self) -> Result<Vec<u8>, Diagnostic>;
pub fn digest(&self) -> Result<ArtifactDigest, Diagnostic>;
pub fn model_artifact(&self) -> ArtifactDigest;
pub fn model(&self) -> Result<OntologyId<Model>, Diagnostic>;
pub const fn semantic_revision(&self) -> SemanticRevision;
pub const fn realization_revision(&self) -> RealizationRevision;
pub fn geometry_artifact(&self) -> ArtifactDigest;
pub fn correspondence_artifact(&self) -> ArtifactDigest;
pub fn mesh_artifact(&self) -> ArtifactDigest;
pub fn solid_domain(&self) -> Id<kinds::Domain>;
pub fn displacement_field(&self) -> Id<kinds::Field>;
pub fn velocity_field(&self) -> Id<kinds::Field>;
pub fn fixed_boundary(&self) -> Id<kinds::Domain>;
pub fn driven_boundary(&self) -> Id<kinds::Domain>;
pub fn driven_total_displacement(&self) -> &[(VertexId, [f64; 3])];
pub fn validate_against(
    &self,
    model: &impl ReplayableCanonicalModelArtifact,
    geometry: &GeometryIdentityEnvelopeV1,
    correspondence: &GeometryMeshCorrespondenceEnvelopeV1,
    mesh: &SimplicialMeshEnvelopeV1,
) -> Result<(), Diagnostic>;
```

The implementation may cache the decoded driven projection privately so the
borrowed selector does not allocate. It must not expose a public wire DTO or a
constructor that accepts the fixed policy objects separately.

Construction and detached `validate_against` replay all durable semantic and
artifact conditions from scratch:

1. The Model artifact digest, typed Model identity, and semantic revision
   agree. The implementation lowers the replayed bound Model to the same exact
   first-order isotropic-elastodynamics meaning used by the accepted #352
   reference. From that result it derives, rather than accepts, the sole solid
   body, length-valued spatial-Cartesian displacement Field, velocity-valued
   spatial-Cartesian velocity Field, exact Cartesian `x = 0` lower boundary
   whose disposition is exactly `TraceZero`, and exact Cartesian `x = 1`
   upper boundary whose disposition is a live `PortBinding { .. }`. The four
   `y`/`z` sides remain exactly `FluxZero`. Raw identity existence, graph
   membership, Geometry membership, or caller-supplied IDs never establishes
   one of these roles.
2. The role identities stored in the wire equal those newly derived
   identities. `PrescribedDynamicSolidRealizationEnvelopeV1` is the sole owner
   of this durable role-binding invariant. The existing numerics lowerer
   remains the owner of executable #352 admission and acceptance; the
   application owner requires both boundaries against the same Model and
   resources and never treats either as a substitute for the other.
3. Geometry validates against that Model, correspondence validates against
   Geometry, Model, and mesh, and their digests equal the wire.
4. The body, displacement, velocity, fixed boundary, and driven boundary are
   distinct where their roles require it. The two Fields share the solid body,
   are rank-one spatial-Cartesian three-vectors, and retain their existing
   coherent-SI length and length-per-time dimensions.
5. Geometry tolerance is exactly `1e-12 m`; the imported mesh quality gate is
   exactly `0.1`; the mesh digest equals the wire; and the mesh is the exact
   accepted ordered nine-vertex, twelve-tetrahedron fixture.
6. The fixed and driven boundaries have the exact correspondence memberships,
   and the driven entries equal the reconstructed canonical driven-boundary
   vertex order.
7. Every singleton discretization, time, solver, layout, target, schedule,
   assembly, solve, and verification value equals the grammar above.

The application owner then replays the accepted numerical lowerer, proves the
exact material, load, boundary-disposition, prior-field, candidate, generation,
and backend-evidence contract, and matches the accepted result's driven
displacement to this validated envelope. This is the sole acceptance path for
the bounded executable claim. The artifact layer's private role derivation
owns no material, load, prior-field, candidate, generation, solver, residual,
or backend acceptance and does not claim that locally valid bytes prove
execution.

The reference application uses explicit Realization revision `1`. The wire
stores it rather than treating it as a default. Decoding an envelope alone
establishes local canonical validity. Detached `validate_against` establishes
the complete durable semantic-role and resource invariant above, including a
fresh bound-Model derivation; it does not establish candidate acceptance,
provider evidence, or that execution occurred. The complete application owner
below is the only path that joins that durable validation to the nonforgeable
accepted numerical result.

### Run-compatible projection

The envelope implements the existing sealed
`CanonicalRealizationArtifact` contract. Its
`RealizationArtifactReference` projection is exact and fixed:

```text
artifact          this envelope's schema-domain digest
model_artifact    model_sha256
semantic_revision semantic_revision
target            HostCpu { threads: 1 }
vector_layout     Replicated
layout_artifacts  Replicated
reduction         Reproducible
```

No new public Realization trait, reference type, target, layout, schedule,
solver, preconditioner, or reduction variant is required.

### Decoder limits

`from_json` accepts the existing `RealizationDecoderLimits`; no new public
limit type is added. Admission proceeds in this order:

1. Apply `limits.json.max_bytes` and `limits.json.max_nesting_depth` before
   deserialization. Defaults remain 16 MiB and 64 levels.
2. Decode the closed DTO and reject malformed JSON, unknown or missing fields,
   unknown literal values, noncanonical digests or ULIDs, non-finite numbers,
   and negative zero.
3. Require `limits.max_realization_fields >= 2`; this family always contains
   exactly the named displacement and velocity roles and never allocates a
   caller-sized Field inventory.
4. Require exactly four driven entries, each with exactly three finite
   components. Reject a different count before Model, Geometry, mesh, or other
   external resources are accessed.
5. Convert `vertex_index` and `maximum_iterations` through checked portable
   integer conversion, then reconstruct and compare the exact singleton
   policy.

The existing constraint and block limits are not reinterpreted because this
wire has no algebraic-constraint or scaled-block collection. A configured
limit cannot widen the fixed grammar. Decoding performs no filesystem,
network, artifact-catalog, backend, or Model replay operation.

## Existing FieldSnapshot and State wires

### Exact family-specific bridge

The accepted `ValidatedFixedSpatialContextV1` is concretely V3-specific: its
public constructor accepts `RealizationEnvelopeV3`, and its stored Realization
and represented-Field machinery cannot carry this family. It is not widened,
renamed, generalized, or overloaded. The generalized snapshot machinery that
already exists behind `ValidatedFieldSnapshotContext` stays crate-private.

Instead, the implementation adds exactly these four family-specific associated
functions to the two existing public envelope types:

```rust
impl FieldSnapshotEnvelopeV1 {
    pub fn new_prescribed_dynamic_solid(
        model: &impl ReplayableCanonicalModelArtifact,
        realization: &PrescribedDynamicSolidRealizationEnvelopeV1,
        geometry: &GeometryIdentityEnvelopeV1,
        correspondence: &GeometryMeshCorrespondenceEnvelopeV1,
        mesh: &SimplicialMeshEnvelopeV1,
        field: Id<kinds::Field>,
        blocks: &[DiscreteFieldEnvelopeV1],
    ) -> Result<Self, Diagnostic>;

    pub fn validate_against_prescribed_dynamic_solid(
        &self,
        model: &impl ReplayableCanonicalModelArtifact,
        realization: &PrescribedDynamicSolidRealizationEnvelopeV1,
        geometry: &GeometryIdentityEnvelopeV1,
        correspondence: &GeometryMeshCorrespondenceEnvelopeV1,
        mesh: &SimplicialMeshEnvelopeV1,
        blocks: &[DiscreteFieldEnvelopeV1],
    ) -> Result<(), Diagnostic>;
}

impl SpatialStateEnvelopeV1 {
    pub fn new_prescribed_dynamic_solid(
        model: &impl ReplayableCanonicalModelArtifact,
        realization: &PrescribedDynamicSolidRealizationEnvelopeV1,
        geometry: &GeometryIdentityEnvelopeV1,
        correspondence: &GeometryMeshCorrespondenceEnvelopeV1,
        mesh: &SimplicialMeshEnvelopeV1,
        step: u64,
        time_s: f64,
        snapshots: &[FieldSnapshotEnvelopeV1],
    ) -> Result<Self, Diagnostic>;

    pub fn validate_against_prescribed_dynamic_solid(
        &self,
        model: &impl ReplayableCanonicalModelArtifact,
        realization: &PrescribedDynamicSolidRealizationEnvelopeV1,
        geometry: &GeometryIdentityEnvelopeV1,
        correspondence: &GeometryMeshCorrespondenceEnvelopeV1,
        mesh: &SimplicialMeshEnvelopeV1,
        snapshots: &[FieldSnapshotEnvelopeV1],
    ) -> Result<(), Diagnostic>;
}
```

Each constructor first calls the Realization's detached `validate_against`, so
its private proof receives roles derived from the bound Model rather than raw
identity membership. A new crate-private prescribed-solid context then
implements the existing private snapshot machinery and the state module's
corresponding factored `new_in_context` path. It proves the exact Model,
Realization, Geometry, correspondence, mesh, complete solid-body cell support,
continuous vector P1 space, and exact two-Field inventory once per public call.
It is neither returned nor accepted as a public parameter.

The Field constructor accepts only the Realization-derived displacement or
velocity Field, one Vertex block, and the lineage below. The State constructor
accepts only the complete two-snapshot inventory and exactly coordinate
`(step, time_s) == (0, 0.0)` or `(1, 0.25)`. Both validation functions rebuild
through their matching constructor and require complete envelope equality.
They never weaken the detached Realization validation and never infer a role
from a snapshot's stored Field ID.

No new public context, trait, proof token, builder, DTO, or top-level type is
introduced. The existing `FieldSnapshotEnvelopeV1::new`,
`FieldSnapshotEnvelopeV1::validate_against`, `SpatialStateEnvelopeV1::new`, and
`SpatialStateEnvelopeV1::validate_against` signatures and behavior remain
byte-for-byte compatible; the four names above are additive. All existing
schemas, canonical DTOs, digest domains, and decoder limits remain unchanged.

### Exact numerical blocks and snapshots

Each displacement or velocity observation is represented by one existing
`DiscreteFieldEnvelopeV1` with:

```text
mesh_sha256       exact imported mesh digest
association       vertex
component_shape   vector with 3 components
entity_count      9
values            entity-major canonical vertex order
```

Each logical observation uses one `FieldSnapshotEnvelopeV1` with exactly one
Vertex block. Snapshot metadata is derived from the Model and Realization:

- support is the exact solid body Domain;
- displacement has the Model's coherent-SI length dimension;
- velocity has the Model's coherent-SI length-per-time dimension;
- both have value shape `[3]` and frame `spatial-cartesian`; and
- every Model, Realization, Geometry, correspondence, mesh, Domain, and Field
  reference equals the new standalone lineage.

The prior and accepted-next velocity values happen to be equal for the
accepted affine reference. Their discrete Field and snapshot digests therefore
may be the same. Content deduplication is correct; role and state occurrence
are established by each State edge, not by inventing duplicate bytes.

### Exact State inventory and coordinates

The application constructs exactly two existing
`SpatialStateEnvelopeV1` values:

| State role | `accepted.step` | `accepted.time_s` | Field inventory |
| --- | ---: | ---: | --- |
| prior | `0` | `0.0` | displacement, velocity |
| accepted next | `1` | `0.25` | displacement, velocity |

Each inventory contains exactly the two snapshot references, sorted by exact
Field ULID as required by the existing V1 wire. Declaration order is not
identity. Both references use the solid Domain; no acceleration, reaction,
multiplier, assembly, reduced-system, solver, or acceptance-evidence entry is
added.

The prior State is a retained exact input observation. It is not a Run output,
checkpoint, restart point, transition-input edge, or claim that an earlier Run
produced it. The accepted-next State is the sole durable output of this
occurrence.

## Atomic application owner and Run publication

### Public application surface

`PrescribedDynamicSolidStateRun3d` is one owned, non-serializable application
value in `eqiora-api`. It is the only public construction path that executes
the reference and publishes the complete State/Run composition. Its reference
entry point has this exact shape:

```rust
pub fn solve_reference(
    document: &ModelDocument,
    assembly: &dyn AssemblyBackend,
    solver: &dyn LinearSolverBackend,
) -> Result<PrescribedDynamicSolidStateRun3d, Diagnostic>
```

It structurally admits only the accepted reference Model while retaining the
caller's exact current Model artifact identity. The mesh, correspondence,
prior fields, driven candidate, Realization revision, and policy are not
caller options.

The complete owner retains:

- the exact current `ModelEnvelope`;
- `GeometryIdentityEnvelopeV1`, `GeometryMeshCorrespondenceEnvelopeV1`, and
  `SimplicialMeshEnvelopeV1`;
- `PrescribedDynamicSolidRealizationEnvelopeV1`;
- the nonforgeable `AcceptedPrescribedDynamicSolidStep3d`;
- every discrete Field block and Field snapshot needed by the two States;
- the prior and accepted-next `SpatialStateEnvelopeV1`; and
- the final `RunManifestV2`.

Read-only selectors expose references to those retained owners and the two
role-specific States. There is no public constructor from an accepted result,
no constructor from detached artifact catalogs, no setter, no mutable state,
no partial builder, and no consuming method that can publish a Run separately
from its exact dependencies. Adding a public selector later does not authorize
a second construction or validation path.

The exact read-only surface, in addition to `solve_reference`, is:

```rust
pub const fn model(&self) -> &ModelEnvelope;
pub const fn geometry(&self) -> &GeometryIdentityEnvelopeV1;
pub const fn correspondence(&self) -> &GeometryMeshCorrespondenceEnvelopeV1;
pub const fn mesh(&self) -> &SimplicialMeshEnvelopeV1;
pub const fn realization(&self) -> &PrescribedDynamicSolidRealizationEnvelopeV1;
pub const fn accepted(&self) -> &AcceptedPrescribedDynamicSolidStep3d;

pub const fn prior_displacement_block(&self) -> &DiscreteFieldEnvelopeV1;
pub const fn prior_velocity_block(&self) -> &DiscreteFieldEnvelopeV1;
pub const fn accepted_displacement_block(&self) -> &DiscreteFieldEnvelopeV1;
pub const fn accepted_velocity_block(&self) -> &DiscreteFieldEnvelopeV1;
pub const fn prior_displacement_snapshot(&self) -> &FieldSnapshotEnvelopeV1;
pub const fn prior_velocity_snapshot(&self) -> &FieldSnapshotEnvelopeV1;
pub const fn accepted_displacement_snapshot(&self) -> &FieldSnapshotEnvelopeV1;
pub const fn accepted_velocity_snapshot(&self) -> &FieldSnapshotEnvelopeV1;
pub const fn prior_state(&self) -> &SpatialStateEnvelopeV1;
pub const fn accepted_state(&self) -> &SpatialStateEnvelopeV1;
pub const fn run(&self) -> &RunManifestV2;
pub fn revalidate(&self) -> Result<(), Diagnostic>;
```

The two velocity selectors may return equal-content artifacts. They remain
role-specific selectors into the complete owner and do not assert distinct
content identities.

`revalidate` first runs the Realization's detached `validate_against`, then
revalidates every block against the mesh, every snapshot through
`validate_against_prescribed_dynamic_solid`, both States through
`validate_against_prescribed_dynamic_solid`, and the Run through its existing
Realization and output checks. It finally repeats exact equality between the
nonforgeable accepted displacement/velocity, the recorded driven candidate,
and the next-State numerical leaves, and checks the accepted generation and
fixed solver/execution evidence. It does not turn a decoded detached
Realization, snapshot, State, or Run into evidence that an execution occurred.

### Exact execution provenance and Run

The application captures the solver provider before execution and relies on
the accepted step's existing equality check against its `SolveReport`. It
constructs existing `ExecutionProvenanceV1` from:

```text
solver provider       accepted SolveReport provider release
execution provider    existing serial execution provider release
topology              Host { workers: 1 }
reduction             Reproducible
additional components empty
```

This is reuse of existing Run V2 provenance, not a new provider protocol or
provider-discovery claim. Assembly evidence remains in memory and is not
flattened into the Run's solver/execution roles.

The Run is constructed through `RunManifestV2::new(&realization, execution)`
and receives exactly one output:

```text
[accepted_next_state.digest()]
```

The owner requires exact vector equality, not mere membership, after ordinary
`RunManifestV2::validate_against`. A Run that also names the prior State, a
block, a snapshot, an acceptance report, or any other digest is rejected.

### Failure atomicity

Execution and artifact composition occur in local unpublished values. The
application owner is returned only after all of the following succeed:

1. exact reference admission and candidate acceptance;
2. standalone Realization construction and replay;
3. all block construction, then every snapshot through
   `FieldSnapshotEnvelopeV1::new_prescribed_dynamic_solid`, then both States
   through `SpatialStateEnvelopeV1::new_prescribed_dynamic_solid`;
4. all block-to-snapshot and snapshot-to-State relational validation through
   the matching family-specific validation methods;
5. Run construction, Realization validation, and singleton-output equality;
   and
6. final comparison between accepted in-memory displacement/velocity and the
   next State's exact numerical leaves.

Any error returns no application owner and exposes no partial Run or State.
The internally advanced reference is dropped and cannot be observed or
continued. Artifact stores, filesystems, networks, Python, and Studio are not
mutated by this operation.

Substitution of an accepted result from another candidate, Model, mesh,
generation, solver policy, provider, or placement fails before publication.
The implementation must not recreate an accepted result from public getter
values or deserialize one from caller data.

## Compatibility and migration

This change is additive:

- all existing Model, Realization V1--V5, Run V1/V2, discrete Field,
  FieldSnapshot, spatial State, trajectory, checkpoint, and restart bytes and
  digest domains remain unchanged;
- existing decoders do not reinterpret another schema as the new family;
- `FieldSnapshotEnvelopeV1` and `SpatialStateEnvelopeV1` gain only the four
  named family-specific methods above; every pre-existing public signature and
  behavior, and all three existing wire contracts, remain unchanged;
- no existing Realization gains a standalone-solid variant or optional
  member; and
- no migration from V3, V4, or V5 is offered because those artifacts have
  different coupled or ALE meaning.

A consumer that does not know the new schema rejects it as unsupported while
continuing to read every previously supported artifact. A consumer that knows
the new schema still needs the exact referenced Model, Geometry,
correspondence, mesh, blocks, snapshots, States, and Run to validate the full
composition.

Before 1.0, replacing the dedicated public names requires another reviewed
compatibility decision and a common standalone-solid artifact/application
boundary that preserves the exact role binding, candidate identity,
failure-atomic publication, and State/Run lineage. An anticipated Python or
Studio consumer alone does not justify a general trait, registry, hierarchy,
or option bag. `PrescribedDynamicSolidStateRun3d` is deliberately a
`transitional_export` in `eqiora::api` and `api/eqiora-facade-v1.json`; this RFC
does not place it in `stable_exports` or promise stable facade compatibility.

## Public architecture budget

The public surface is itself the bounded product claim. It adds exactly two
public types, four associated methods on existing artifact types, and no new
crate:

| Crate | Accepted base | Maximum after implementation | Addition |
| --- | ---: | ---: | --- |
| `eqiora-numerics` | `309` | `309` | none |
| `eqiora-artifact` | `146` | `147` | `PrescribedDynamicSolidRealizationEnvelopeV1` |
| `eqiora-api` | `138` | `139` | `PrescribedDynamicSolidStateRun3d` |
| `eqiora` | `280` | `281` | application owner re-export only |
| `eqiora-realization` | `117` | `117` | none |

The artifact type remains reachable through the facade's existing transitional
`eqiora::artifact` module; that existing glob is not widened textually or
converted into another stable registration. The one new named curated-facade
item is the application owner under `eqiora::api`, classified explicitly as a
`transitional_export`, never as a stable export. Existing root ownership
conventions still apply inside the workspace. The four family-specific methods
and the Realization's role-deriving constructor/validator add no top-level
public item and therefore do not move the table's type ceilings.

The three one-item increases in the table are planned, reviewed architecture
changes. The integrator changes exactly the existing `[[public_surface]]`
entries in `tools/ci/architecture-debt.toml` and no others:

| Crate | Exact ceiling | Exact reason appended to the existing `reason` | Exact deletion condition appended to the existing `removal` |
| --- | ---: | --- | --- |
| `eqiora-artifact` | `147` | `PrescribedDynamicSolidRealizationEnvelopeV1 is one closed standalone-solid wire for the exact prescribed-step occurrence; it reuses existing decoder limits and Snapshot, State, and Run families, derives every durable role from the bound Model, and adds no registry, trait, builder, or option bag.` | `Withdraw PrescribedDynamicSolidRealizationEnvelopeV1 and lower this ceiling to 146 when one already-counted accepted common fixed-spatial Realization preserves its exact role derivation, Geometry and mesh gates, canonical bytes, and Run projection.` |
| `eqiora-api` | `139` | `PrescribedDynamicSolidStateRun3d is one failure-atomic application composition joining the accepted prescribed step to existing Field, Snapshot, State, and Run artifacts; it adds no request, result, provider, or registry protocol.` | `Withdraw PrescribedDynamicSolidStateRun3d and lower this ceiling to 138 when one already-counted accepted common native structural Result owns and revalidates the same exact lineage and accepted evidence without an open option bag.` |
| `eqiora` | `281` | `PrescribedDynamicSolidStateRun3d is one named transitional eqiora::api export of the bounded application owner; the facade adds no second implementation or stable compatibility promise.` | `Withdraw that transitional export and lower this ceiling to 280 when an accepted existing facade type subsumes its exact structural lineage and accepted evidence without a replacement top-level facade item.` |

These are ceiling amendments, not debt permission for another item. An
implementation that ends below a listed maximum ratchets that ceiling down in
the same integration. Growth beyond `147`, `139`, or `281`, or growth in any
other crate, is an unplanned architecture change and a stop condition.

Any additional public type, trait, enum variant, registry, context, builder,
option bag, durable schema, facade item, or ceiling growth beyond the three
exact reviewed maxima is a stop condition. A private helper is permitted only
when it does not establish a second invariant owner or public extension point.

## Successor path ownership

The contract owner writes only:

```text
rfcs/0084-standalone-prescribed-dynamic-solid-artifacts.md
```

The independent exact-artifact oracle owns:

```text
crates/eqiora/tests/prescribed_dynamic_solid_state_run_3d.rs
verify/artifacts/prescribed-dynamic-solid-state-run-3d/**
```

The frozen current-Model relational sweep applies unconditionally. Its separate
oracle owner must pre-commit the exact admission delta below before this RFC or
any successor path is integrated, and may write only:

```text
crates/eqiora-artifact/tests/current_model_relational_identity_transition/transition_contract.rs
crates/eqiora-artifact/tests/current_model_relational_identity_transition/post_reset_admission.rs
verify/artifacts/current-model-relational-identity-transition/README.md
verify/artifacts/current-model-relational-identity-transition/expected/README.md
verify/artifacts/current-model-relational-identity-transition/expected/classification.json
verify/artifacts/current-model-relational-identity-transition/references/derive_transition_identities.py
```

The transition amendment adds these exact identity-free paths to the existing
containment-only `post_reset_admitted` permission, with no glob, directory,
suffix rule, inferred sibling, or membership in a historical frozen set:

| Exact path | Exact search signals | `identity_literals` | Class |
| --- | --- | ---: | --- |
| `rfcs/0084-standalone-prescribed-dynamic-solid-artifacts.md` | `eqiora.model-envelope/v`, `model_sha256` | `0` | `current-owner-assertion` |
| `crates/eqiora-artifact/src/prescribed_dynamic_solid_realization.rs` | `model_sha256` | `0` | `non-fixture-search-hit` |
| `crates/eqiora/tests/prescribed_dynamic_solid_state_run_3d.rs` | `model_sha256` | `0` | `non-fixture-search-hit` |
| `verify/artifacts/prescribed-dynamic-solid-state-run-3d/references/derive_prescribed_dynamic_solid_state_run_3d.py` | `eqiora.model-envelope/v`, `model_sha256` | `0` | `non-fixture-search-hit` |

The transition contract's prose and regression names must describe this
existing permission as later identity-free classified paths rather than only
product paths, because the RFC and independent derivation route are ordinary
classified search hits too. The optional/exact/zero-identity predicate itself
does not change.

The expected bytes are signal-bearing fixtures rather than identity-free
consumer surfaces. The transition owner therefore adds a separate
containment-only `post_reset_fixture_admitted` record whose predicate is:
optional after the reset; absent before it; admitted only by exact path; exact
ordered search-signal list; exact same-line Model-derived lower-hex-64 literal
count; declared fixture class, owner, and note; and membership in none of
`inventory`, `retired`, `required_post_reset`, `preserved_evidence`, promotion,
or the existing `post_reset_admitted` set. It does not weaken the existing
zero-identity admission predicate. Its exact initial rows are:

| Exact path | Exact search signals | `identity_literals` | Class |
| --- | --- | ---: | --- |
| `verify/artifacts/prescribed-dynamic-solid-state-run-3d/expected/model.json` | `eqiora.model-envelope/v` | `0` | `delegated-current-owner-evidence` |
| `verify/artifacts/prescribed-dynamic-solid-state-run-3d/expected/geometry-identity.json` | `model_sha256` | `1` | `delegated-current-owner-evidence` |
| `verify/artifacts/prescribed-dynamic-solid-state-run-3d/expected/realization.json` | `model_sha256` | `1` | `delegated-current-owner-evidence` |
| `verify/artifacts/prescribed-dynamic-solid-state-run-3d/expected/prior-displacement-snapshot.json` | `model_sha256` | `1` | `delegated-current-owner-evidence` |
| `verify/artifacts/prescribed-dynamic-solid-state-run-3d/expected/prior-velocity-snapshot.json` | `model_sha256` | `1` | `delegated-current-owner-evidence` |
| `verify/artifacts/prescribed-dynamic-solid-state-run-3d/expected/accepted-displacement-snapshot.json` | `model_sha256` | `1` | `delegated-current-owner-evidence` |
| `verify/artifacts/prescribed-dynamic-solid-state-run-3d/expected/accepted-velocity-snapshot.json` | `model_sha256` | `1` | `delegated-current-owner-evidence` |
| `verify/artifacts/prescribed-dynamic-solid-state-run-3d/expected/prior-state.json` | `model_sha256` | `1` | `delegated-current-owner-evidence` |
| `verify/artifacts/prescribed-dynamic-solid-state-run-3d/expected/accepted-state.json` | `model_sha256` | `1` | `delegated-current-owner-evidence` |
| `verify/artifacts/prescribed-dynamic-solid-state-run-3d/expected/run.json` | `model_sha256` | `1` | `delegated-current-owner-evidence` |

`transition_contract.rs`, `post_reset_admission.rs`, `classification.json`,
both transition READMEs, and the independent transition derivation script must
all enforce and describe that exact two-permission distinction. Historical
counts and sets do not move. A successor file that carries another search
signal, or any listed file whose exact signal list or literal count differs,
returns to the transition-oracle owner before the lane starts. The new
lineage-case README, case manifest, model source, nonsignal-bearing mesh,
correspondence, discrete blocks, and reference README prose must remain
free of the sweep's search spellings unless first admitted by another exact
row.

The production implementation writer owns only:

```text
crates/eqiora-artifact/src/prescribed_dynamic_solid_realization.rs
crates/eqiora-artifact/src/realization_reference.rs
crates/eqiora-artifact/src/spatial_data/context.rs
crates/eqiora-artifact/src/spatial_data/field.rs
crates/eqiora-artifact/src/spatial_data/state.rs
crates/eqiora-api/src/prescribed_dynamic_solid.rs
```

The integrator alone owns:

```text
rfcs/README.md
crates/eqiora-artifact/src/lib.rs
crates/eqiora-api/src/lib.rs
crates/eqiora/src/lib.rs
api/eqiora-facade-v1.json
tools/ci/architecture-debt.toml
docs/capability-matrix.md
docs/roadmap.md
CHANGELOG.md
```

The integrator's exact registration delta is: add
`- [RFC 0084: Standalone prescribed dynamic-solid artifacts](0084-standalone-prescribed-dynamic-solid-artifacts.md)`
to `rfcs/README.md`; declare and named-re-export
`PrescribedDynamicSolidRealizationEnvelopeV1` from `eqiora-artifact`; declare
and named-re-export `PrescribedDynamicSolidStateRun3d` from `eqiora-api`; add
only that application owner to the curated `eqiora::api` Rust namespace and
to that namespace's JSON `transitional_exports` array; make no addition to any
`stable_exports` array; apply exactly the three reviewed public-surface ceiling
amendments above; update the existing capability rows below; and record the
accepted roadmap/changelog item. Existing artifact access continues through
the transitional `eqiora::artifact` module, so no curated facade entry for the
Realization is added. The integrator must run the index, facade, and
architecture checks after these registrations.

The capability matrix gains no artifact-specific row. The integrator narrows
and extends exactly these existing rows, retaining their current C/X/V/M
statuses and all existing evidence links:

- Both occurrences of `First-order dynamic linear solid` add that the same
  exact serial-host unit-cube 3D P1 step now publishes one content-addressed
  standalone-solid Realization, exact prior and accepted-next two-Field States,
  and one Run whose sole output is the accepted-next State. They link
  `artifacts.prescribed-dynamic-solid-state-run-3d` and replace the former
  blanket nonclaim of durable standalone structural time integration with the
  narrower nonclaim: no other Model, mesh, candidate, step, multi-step
  trajectory, restart, or general standalone structural integration is
  durable.
- `Linear elasticity` adds only that exact one-step unit-cube occurrence and
  its standalone Realization/State/Run lineage, links
  `artifacts.prescribed-dynamic-solid-state-run-3d`, and continues to disclaim
  general standalone 3D analysis, wider boundary data, elements, materials,
  performance, and scale.
- `Structural dynamics` replaces `Durable standalone State/Run lineage` as an
  unqualified remainder with the exact admitted one-step lineage above, links
  `artifacts.prescribed-dynamic-solid-state-run-3d`, and keeps multiple-step
  standalone trajectories, restart/continuation, damping, modal, harmonic,
  response-spectrum, finite-strain, and contact paths as nonclaims. The prior
  State remains a retained input observation, not a Run output or evidence of
  an earlier execution.

No Cargo manifest or lockfile change is expected. A writer that needs another
path, public item, registration, or ceiling change beyond the three exact
amendments stops and returns the requirement to the contract owner or
integrator.

## Independent exact-artifact oracle

This RFC adds durable schema and exact-artifact meaning, so a fresh
non-implementer must pre-commit the oracle before production implementation.
No dual scientific derivation is required because the oracle consumes every
scientific value and tolerance from the accepted reference unchanged.

The independent oracle freezes at least:

- complete canonical Realization bytes, byte length, schema-domain digest,
  field order, nested field order, and exact decoder boundaries;
- the exact `1e-12 m` Geometry classification tolerance and exact `0.1` mesh
  minimum-mean-ratio gate as canonical, digest-bearing members of their owner
  artifacts and stale Realization edges when either owner changes;
- independently rendered discrete Field, FieldSnapshot, prior State,
  accepted-next State, and Run bytes and identities;
- every Realization-to-block-to-snapshot-to-State-to-Run edge;
- the exact two-Field inventory, canonical Field-ULID order, step/time
  coordinates, and accepted-next singleton output; and
- the accepted in-memory displacement and velocity equality with the next
  State's numerical leaves.

At minimum, mutations must reject:

- wrong schema, encoding, key order, unknown member, malformed or uppercase
  digest, malformed or noncanonical ULID, byte limit, depth limit, or Field
  limit;
- changed Model digest/identity/revision, Realization revision, Geometry,
  correspondence, mesh, body, Field, or boundary role;
- a Model in which all recorded role IDs still exist but `x = 0` is not exact
  `TraceZero`, `x = 1` is not a live `PortBinding { .. }`, either boundary is
  on another Cartesian side, or a `y`/`z` side is not exact `FluxZero`;
- Geometry classification tolerance other than exact `1e-12 m` or a mesh
  minimum-mean-ratio gate other than exact `0.1`;
- changed, missing, duplicated, reordered, non-finite, negative-zero, or
  noncanonical driven displacement;
- another space, method, quadrature, time method/duration, solver axis,
  reduction, target, worker count, schedule, execution mode, or layout;
- a block on another mesh, wrong association or shape, changed vertex order,
  missing coefficient, or changed coefficient;
- a snapshot with a stale lineage, wrong role, wrong physical metadata,
  wrong block, or nonzero value outside the exact support;
- a State with a missing, duplicated, reordered, foreign, or substituted
  snapshot, wrong step/time, or additional Field;
- substitution of the prior State for the accepted-next State;
- a Run with stale Model/Realization, wrong provider/topology/reduction, no
  output, the prior State, an additional output, or another accepted-next
  State; and
- an accepted result from another candidate, generation, provider, solver
  plan, execution report, or verification report.

The oracle separately proves detached behavior: `from_json` alone accepts
locally canonical bytes without claiming referenced resources or execution;
`validate_against` rejects every semantic-role or resource mutant above but
still makes no execution claim; and `PrescribedDynamicSolidStateRun3d` is the
only public value whose successful construction and `revalidate` join those
bytes to accepted in-memory execution evidence.

The writer may wire the pre-committed fixtures but must not derive, tune,
relax, reorder, or replace their bytes, expected identities, or mutations. If
the RFC or implementation changes a numerical formulation, expected value, or
tolerance, work stops and the dual independent scientific-oracle gate applies.

## Semantically affected verification

The successor implementation explicitly runs these registered cases:

```text
solid.prescribed-dynamic-solid-step-3d
artifacts.prescribed-dynamic-solid-state-run-3d
artifacts.realization-run-wire
artifacts.fixed-reference-fsi-spatial-trajectory
artifacts.general-fixed-mesh-field-trajectory-2d
artifacts.current-model-relational-identity-transition
packages.typed-execution-lineage
fsi.fixed-topology-ale-monolithic-3d
```

The new case owns only the exact standalone lineage in this RFC. Existing
cases prove that Run V2, fixed-mesh State/Field behavior, current-Model
relational identity, typed package execution lineage, and ALE families remain
unchanged.

## Alternatives considered

### Encode the step as Realization V3 with a dummy fluid

Rejected. A dummy Domain and trace quotient would become content-addressed
physics and make every validator preserve a coupling that never occurred.

### Encode the step as Realization V4 or V5 with inert ALE

Rejected. An identity motion map is still an ALE graph. Persisting it would
make the Run claim geometry-motion policy absent from the reference execution.

### Use only `RealizationArtifactReference`

Rejected. The reference has no canonical bytes or decoder and can only project
an existing durable Realization. It cannot be the object a Run or external
catalog resolves.

### Add a new State or Run generation

Rejected. Existing V1 fixed-mesh snapshots/States already express the complete
two-Field observations, and Run V2 already owns exact Realization lineage,
execution provenance, and sorted outputs. A new generation would duplicate
meaning without closing another invariant.

### Persist the complete accepted result

Rejected. Acceleration, reactions, matrices, residuals, assembly reports, and
solve reports are acceptance evidence, not the durable physical observation
selected for this slice. Persisting them would require new schemas, resource
budgets, compatibility promises, and independent oracles while widening no
current user claim.

### Add a transition, trajectory, or checkpoint edge

Rejected. The prior State is retained to close the application composition,
but this slice does not claim restart, continuation, multi-step ordering, or a
durable transition input. The Run publishes only one accepted-next State.

### Build Python or Studio in the same slice

Rejected. They are outward consumers of the accepted Rust contract. Python
provider integration follows after this lineage is accepted; Studio remains a
later thin consumer of that Python-facing contract.

## Security, safety, and governance

All decoding uses safe Rust, closed DTOs, syntax admission before serde,
checked integer conversion, finite canonical binary64 values, and explicit
external resource validation. Content digests provide identity and integrity,
not authenticity or authorization. Signing, trust policy, remote artifact
resolution, and access control remain Evidence and deployment concerns.

The application does not write files, mutate stores, contact providers,
spawn subprocesses, or publish over a network. Failure returns no durable
owner. Provider and assembly objects are caller-supplied in-process execution
capabilities only; their accepted reports are checked by the existing
numerical contract.

Because this change defines a persisted schema, public API, compatibility
promise, and exact artifact, the complete risky delta requires fresh-context
non-writer review before integration. The exact-artifact oracle is separately
owned and pre-committed. Confidence or implementation self-tests cannot
replace either gate.

## Nonclaims and stop conditions

This RFC does not add Python, subprocess execution, Studio, callback protocol,
provider discovery, a new provider-provenance wire, a durable request,
candidate artifact, transcript, transition-input edge, trajectory,
checkpoint, restart, or continuation.

It does not admit an arbitrary solid Model, material, mesh, boundary,
candidate, time step, time integrator, quadrature, solver, placement, schedule,
or State inventory. It adds no traction loading, two-way conservation, ALE,
remeshing, nonlinear solve, MPI, GPU, parallel execution, performance, or
scale claim. It persists no acceleration, reaction, matrix, residual, assembly
trace, solver report, or general acceptance evidence.

It adds no general solid Realization hierarchy, public trait, registry,
builder, option bag, context, result schema, or State/Run generation. Work
stops rather than broadening this RFC if implementation requires any of those,
changes accepted science or tolerances, cannot preserve existing artifact
bytes, exceeds the exact public budget, needs a Cargo dependency, or cannot
publish the complete owner failure-atomically.

## Unresolved questions

None within the bounded reference. Python/provider integration is owned by the
next outward slice, and Studio is intentionally deferred as a later thin
consumer.
