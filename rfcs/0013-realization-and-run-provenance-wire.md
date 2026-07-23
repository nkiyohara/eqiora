# RFC 0013: Realization and run-provenance wire

- Status: Draft implementation
- Authors: Eqiora contributors
- Created: 2026-07-18

## Summary

Eqiora serializes resolved numerical/deployment policy as a versioned
Realization envelope and records the actually resolved adapter, libraries,
topology, and reduction policy in a linked run-manifest v2.

## Motivation

`RunManifestV1` can refer to an opaque realization digest and stores numerical
settings in a string map. That was sufficient before Realization policy had a
typed contract, but it cannot prove which problem requirements admitted a
plan, whether a run used the requested target and reduction, or which layout
and partition artifacts defined distributed ownership.

Adding optional fields to v1 would change its canonical bytes and still leave
policy split across unrelated maps. Backend-specific serialized structs would
instead make faer, Rayon, MPI, or CUDA API changes into Eqiora wire changes.

## Proposed design

### Realization envelope v1

`eqiora.realization-envelope/v1` contains:

- the canonical Semantic Model digest, typed model ULID, and semantic revision;
- resolution source: default-policy version or explicit realization revision;
- admitted spatial dimension, scalar type, and vector-layout requirement;
- exact space, mesh, quadrature, solver, target, and deployment schedule plan;
- either replicated layout or typed layout and partition artifact digests.

Construction accepts a `ModelEnvelopeV1` and a `ResolvedRealization`, not three
independent IDs. Their model identity and source/semantic revision must match.
The admitted requirements are retained by `ResolvedRealization`; resolution no
longer discards them after capability checking. Distributed requirements must
carry both layout and partition digests, while replicated requirements cannot
smuggle them in.

Wire DTOs are private. Decoding reconstructs `Space`, `Discretization`,
`SolverPlan`, `Target`, `ExecutionSchedule`, and `RealizationPlan` through
their validated constructors. Platform-sized counts use portable `u64` wire
values and fail when they cannot fit local `usize`. Default-policy/v0 is the
only default admitted by this v1 schema; later defaults require an explicit
schema decision rather than reinterpretation.

### Run manifest v2

`eqiora.run-manifest/v2` requires a Realization digest. It removes v1's opaque
numerical-setting map because exact numerical policy belongs to the linked
Realization envelope. Its execution provenance contains:

- stable execution-adapter and solver-backend identities and versions;
- a sorted map of resolved library/runtime versions;
- exactly one typed topology: host, distributed, or CUDA;
- the reduction policy actually used; and
- sorted, unique output artifact digests.

Host topology records an exact worker count. Distributed topology records
partitions, workers per partition, and either loopback or an MPI
implementation/version/thread-support profile. CUDA topology records ordinal,
device name, compute capability, and driver version. These are Eqiora-owned
DTOs; no third-party handle or enum crosses the artifact boundary.

For deployment-bound linear execution, the stable IDs and implementation
versions originate in typed provider descriptors carried by both the selected
binding and the actual producer report. Receipt acceptance requires exact
descriptor equality, including sorted dependency-release versions, before the
application projects the accepted receipt into this wire. Live driver,
system-MPI, and dynamically loaded vendor-library observations remain
adapter-owned Run inputs rather than static-provider claims.

The v2 fields identify the primary solver and execution provider. Its
`libraries` map is a unique component-version inventory, not a role-preserving
encoding of solver, execution, verifier, nested device action, and live runtime
ownership. Conflicting versions under one component name must fail projection.
If a durable consumer needs the complete role graph, that requires a new
schema; v2 is not widened by reinterpretation.

`RunManifestV2::new` accepts the Realization envelope itself. It derives model,
revision, and realization digests and rejects any disagreement between:

- realized and executed reduction policy;
- host target threads and resolved host workers;
- distributed requirements/artifacts and distributed topology;
- per-partition host threads and distributed worker count; or
- CUDA target ordinal and resolved device ordinal.

After loading the two artifacts separately, `validate_against` repeats the
same content-link and policy checks. Parsing a run manifest alone proves local
wire validity, not availability of its referenced artifacts.

### Identity boundary

Model, Realization, and run manifests have separate domain-separated SHA-256
identities. A change in mathematical meaning changes the model artifact; a
change in method/policy changes the Realization artifact; a change in resolved
backend environment changes the run artifact. Wall-clock timestamps and host
paths remain outside these content identities.

### Imported affine-simplex mesh artifact

`eqiora.simplicial-mesh-envelope/v1` is an independent Realization artifact.
It records runtime topological dimension, affine `f64` coordinates, simplex
connectivity, the accepted mean-ratio threshold, and recomputed minimum
mean-ratio/signed-measure evidence. It contains no model equation, field,
filesystem path, importer configuration, solver policy, or partition.

Decoding first applies byte/nesting and mesh-specific count limits, then
reconstructs `SimplicialMesh` through the shared `eqiora-meshing` constructor.
Duplicate/isolated/non-manifold topology, invalid indices, non-finite or
inverted geometry, and quality-gate failure are therefore rejected by the
same contract used by assembly. Stored quality evidence must bitwise equal the
fresh reconstruction; it is evidence to verify, not trusted cached state.

`MeshPolicy::ImportedSimplicial` stores only the artifact's SHA-256 identity.
`RealizationCapabilities` admits mesh kind explicitly, independently of method
and dimension. The v0 cross-contract permits only continuous P1 Galerkin with
simplex-centroid quadrature. `RealizationEnvelopeV1::validate_mesh_artifact`
checks both digest and admitted spatial dimension before the typed mesh reaches
the numerical entry point. Generated Cartesian policy retains its existing v1
representation.

### Orthogonal residual-time restart lineage

Residual-native time continuation composes with a run's sorted output digests
without widening run-manifest v2. An `ImplicitTimeCheckpointEnvelopeV1` owns
one accepted semantic
`(time, state, derivative)` point, canonical residual replay norm, and
acceptance tolerance. It references the model and residual-native lowering but
deliberately not the producing run. The parent can therefore list the
checkpoint as an output without a content-digest cycle.

`ImplicitTimeRestartManifestV1` is the separate edge artifact. It links the
parent run, accepted checkpoint, checkpoint-derived `Provided` initial-data
artifact, and child run. External validation requires the child plan to start
at checkpoint time and both child initial-data identities to equal the derived
artifact. This proves semantic restart only; adaptive-controller, BDF,
factorization, or backend-native checkpoint history requires a future typed
payload and capability contract.

## Alternatives considered

### Widen run-manifest v1

Rejected. It would change frozen v1 canonical bytes, preserve the untyped
settings map, and conflate requested policy with resolved execution.

### Serialize public Rust enums directly

Rejected. Harmless Rust refactors would become wire compatibility events and
deserialization could bypass constructor validation. Dedicated DTOs keep wire
and API evolution independent.

### Put backend/library versions in Realization policy

Rejected. Realization chooses portable policy; run provenance records the
adapter and environment that actually executed it. Pinning a library version
would prevent comparing valid backends for the same Realization.

### Accept unrelated digests in constructors

Rejected. Strong field types do not prevent a caller from combining the wrong
model, revision, and Realization. Constructors derive linked identities from
the referenced envelopes and verify them before producing a manifest.

## Compatibility and migration

Model-envelope/v1 and run-manifest/v1 bytes and decoders are unchanged.
Existing v1 manifests remain readable. New reproducible execution evidence
uses Realization-envelope/v1 plus run-manifest/v2. Migration is explicit:
decode the old manifest, independently reconstruct and resolve typed policy,
then emit new artifacts; no reader guesses typed policy from arbitrary v1
setting strings.

The imported-simplicial mesh and simplex-centroid variants are append-only v1
decoder variants. Existing generated-mesh realization bytes are unchanged and
retain their prior meaning. Implementations that do not support the new mesh
kind reject it through capability admission; they never substitute a generated
mesh.

These Rust APIs remain provisional before 1.0, while the named wire schemas
are append-only contracts. New methods, targets, or topology variants require
either a decoder that preserves existing v1 meaning or a new schema.

## Verification

- Round-trip a compiled canonical model, resolved Realization, and run v2 to
  byte-identical canonical JSON and domain-separated digests.
- Require typed model identity, semantic revision, requirements, and plan to
  survive reconstruction.
- Require a distributed realization to carry layout and partition digests and
  accept explicit loopback topology.
- Reject unrelated model/Realization pairing, replicated/distributed layout
  mismatch, reduction drift, worker-count drift, malformed digests, unknown
  fields, duplicate outputs, and decoder-limit violations.
- Keep all model/run v1 tests byte-for-byte unchanged.
- Round-trip an accepted affine-simplex mesh to byte-identical canonical JSON
  and a domain-separated digest, link it through Realization, and execute one
  canonical 2D P1 problem with independently derived one-DOF value and global
  balance.
- Reject absent mesh capability, digest drift, dimension drift, resource
  excess, unknown fields, malformed topology/geometry, and forged quality
  evidence before accepted solve evidence.
- Round-trip a residual-native checkpoint and restart edge byte-for-byte,
  independently replay residual acceptance, and reject missing parent output,
  child-time drift, content drift, and a parent/child cycle.

## Security, safety, and governance

All decoding uses safe Rust, denies unknown fields, applies byte/nesting
limits before serde decoding, applies mesh vertex/cell/coordinate/connectivity
limits before topology reconstruction, validates bounded integer conversion,
and reconstructs typed values before returning an envelope. Digests provide
content addressing, not authenticity; signatures and trust policy remain an
Evidence layer responsibility.

Changing a wire schema, default-policy interpretation, or the fields included
in content identity requires RFC review. A typed CUDA or MPI topology is
provenance vocabulary, not a support claim; support still requires executable
capability and verification evidence under RFC 0010.

## Unresolved questions

- Whether layout and partition gain dedicated canonical envelope schemas or
  are bundled in a future distributed-realization artifact.
- How derivative source and linearization identity join run provenance without
  making the smooth v2 contract hybrid-specific.
- How backend-native durable checkpoint payloads and adjoint trajectory
  schedules reference the semantic restart edge without weakening it.
- Whether a future evidence bundle signs each artifact or one Merkle root.
