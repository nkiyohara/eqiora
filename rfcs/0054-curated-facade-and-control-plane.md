# RFC 0054: Curated facade and one control plane

- Status: Accepted and implemented for the bounded compile/check slice
- Authors: Eqiora contributors
- Created: 2026-07-20
- Depends on: [RFC 0008](0008-canonical-artifact-wire-v1.md),
  [RFC 0012](0012-python-interop-boundaries.md), and
  [RFC 0013](0013-realization-and-run-provenance-wire.md)

## Summary

Eqiora will make public compatibility and client control explicit: a checked,
machine-readable inventory owns the stable Rust facade, while one exact,
versioned control-plane contract is projected into Rust, TypeScript, and
Python without creating another model or scientific-data semantics.

## Motivation

The `eqiora` facade is intended to shield applications and public physics
packages from internal crate organization. Most of its historical modules,
however, glob re-export complete internal crates. A harmless new public helper
inside `eqiora-numerics` can therefore become a facade addition without a
compatibility decision. That defeats the boundary precisely when internal
spatial implementation is changing fastest.

The Rust API, Studio, and Python adapters also need the same small operations:
compile/check, preview, execute, cancel, inspect, and diff. Hand-written DTOs
and dispatch in each adapter would make protocol admission, diagnostic
identity, and feature negotiation presentation-dependent. Encoding arrays,
meshes, Fields, or trajectories into a universal command object would create
the opposite error: a control protocol that becomes a second scientific data
model.

The two problems share one discipline. A public name or wire field enters a
compatibility boundary only through an explicit, reviewed inventory.

## Decision

### Rust facade inventory

`api/eqiora-facade-v1.json` is the machine-readable inventory for the
`eqiora` crate. Each admitted export records both its downstream name and its
exact source path. The inventory distinguishes:

- stable exports, which are the deliberately small pre-1.0 compatibility
  surface; and
- transitional exports and modules, which remain source-compatible during
  incremental migration but acquire no implicit stable-surface claim.

`cargo xtask check-facade` compares the inventory to
`crates/eqiora/src/lib.rs`. It fails when a stable namespace contains a glob,
an export changes provider under the same name, a root module is unclassified,
or source and inventory otherwise differ. The check itself is part of the
ordinary local and hosted reproduction gates.

The first inventory keeps all existing root modules available. It curates
`eqiora::api` explicitly rather than removing existing names in one breaking
rewrite. The dependency-minimal core vocabulary and current-authoring
`ModelDocument` are stable entries. Exact historical selection is isolated in
`eqiora::compatibility` as `ExactModelCodec`; it is not part of the ordinary
authoring prelude. Existing numerical, CAD, execution, and package bootstrap
surfaces are transitional until a bounded provider or application contract
selects them deliberately.

New stable namespaces, including `eqiora::control`, must contain only explicit
re-exports and must be added to the inventory in the same change. The
historical glob modules are migrated when their public boundary is understood;
this RFC does not turn twenty-two mechanical rewrites into a prerequisite for
the first useful gate.

### Control-plane ownership

`eqiora-api` owns transport-neutral command values and exact dispatch. The
public facade exposes the selected subset under `eqiora::control`. Client
adapters translate language ergonomics and transport concerns only; they do
not compile, validate, or infer model meaning independently.

Every protocol generation has:

- one exact protocol and command identity;
- explicit supported features and version admission;
- bounded request sizes before expensive parsing or allocation;
- structured diagnostics with stable code, severity, source, optional graph
  path and source span, and a bounded patch projection;
- deterministic response serialization; and
- committed, generated-or-checked language projections whose drift is a test
  failure.

Unsupported identity or feature combinations fail closed. There is no
heuristic wire-generation detection and no fallback from a rejected new
generation to an older interpretation.

The first command is a bounded compile/check slice. It migrates the existing
compiler and `ModelTransactionEnvelope` path; it does not define a parallel
transaction. A successful response identifies the exact immutable Model
artifact and the transaction wire generation used to construct it. A failed
response carries diagnostics and no partial accepted Model.

Preview-to-execute replay keys, cancellation, run lifecycle, artifact
inspect/diff, resource estimates, and verification commands follow as
independent slices. Their existing typed owners remain authoritative until
each migration closes.

### Control plane and data plane

The control plane may carry small descriptors, artifact references, evidence
summaries, diagnostics, and capability decisions. Scientific arrays, mesh
connectivity, Field snapshots, and trajectories remain the data plane owned by
[RFC 0051](0051-durable-spatial-state-and-trajectory.md). A control response
references those durable objects; it never embeds them merely to avoid a
second transport.

This separation is semantic, not size-only. A small mesh is still data-plane
content, and a large diagnostic list is still rejected by the control
protocol's explicit bounds.

## Compatibility and migration

The first facade change removes no existing `eqiora` path. Replacing
`eqiora::api`'s glob with an explicit list is source-neutral for every listed
name. Transitional classification records the honest compatibility boundary
during the pre-1.0 migration; it is not permission to delete a name without a
normal review and migration note.

Adding a public item to an internal crate no longer changes a curated facade.
Adding, moving, stabilizing, or retiring a facade export requires an inventory
change. Moving an export to a different provider is detected even when its
downstream spelling is unchanged.

Existing canonical Model envelopes and transaction generations remain
immutable. The compile/check control command composes them and advertises the
exact selected generation. Studio and Python migrate to the same fixture
incrementally; no client must switch to an incomplete second protocol.

## Alternatives considered

### Rewrite every facade glob immediately

This would produce a superficially uniform `lib.rs`, but deciding hundreds of
internal names mechanically would turn accidental exposure into an accidental
compatibility promise. The explicit stable/transitional inventory establishes
the enforcement boundary now and leaves each legacy module to a bounded
migration.

### Snapshot rustdoc output only

A generated symbol diff detects additions, but by itself it does not say which
names are intentional, which are transitional, or which provider owns an
alias. The reviewed inventory is the authority; rustdoc or semver tooling may
later add a second compatibility check.

### Hand-write DTOs in every client

This avoids generation machinery initially, but creates multiple admission
rules and diagnostic projections. A single schema-owned fixture and checked
projections keep adapters thin without forcing all clients to share a runtime.

### Put scientific data in the command protocol

One universal JSON request would simplify toy examples and immediately blur
ownership, zero-copy array exchange, content identity, and streaming. Durable
data-plane artifacts and buffers remain separate.

## Verification

The facade gate is falsified by tests that introduce:

- a glob in a stable namespace;
- an unregistered public root module;
- a missing or extra export;
- the correct downstream name re-exported from the wrong provider; or
- duplicate or contradictory inventory entries.

The bounded compile/check slice is complete only when one committed fixture:

1. has the same exact request and response meaning in Rust, Studio TypeScript,
   and Python;
2. succeeds through the ordinary compiler, transaction, graph commit, and
   immutable Model artifact path;
3. rejects unsupported protocol, command, feature, malformed UTF-8, and
   resource-limit inputs with stable diagnostic identities; and
4. detects generated projection or schema drift locally.

Passing the facade inventory does not verify the control protocol, and a Rust
round-trip alone does not claim cross-language conformance.

The registered
[`interfaces.control-plane-compile-check`](../verify/interfaces/control-plane-compile-check/README.md)
case closes this slice. Later commands retain the ownership rules in this RFC
but do not widen its compile/check claim.

## Security, safety, and governance

The inventory grants no dynamic loading or provider execution authority. The
control protocol accepts data, not native callbacks or executable plugins.
All lengths are checked before expensive work; diagnostics must not expose
unbounded source content or host filesystem paths.

Promoting a transitional export to stable is a compatibility decision and
requires review of its abstraction and provider boundary. Generated files are
committed for auditability, but the schema/checker remains authoritative and
must reject manual drift.

## Nonclaims and deferred decisions

This RFC does not define a universal RPC framework, remote code execution,
dynamic Studio plugins, Python callbacks inside solver loops, scientific array
transport, or automatic Model wire detection. It does not promise stability
for every historical `eqiora::<internal-family>` glob.

The provider SDK, transport framing, authentication, remote cancellation, and
the storage container used by the data plane remain separate decisions. They
must be justified by at least two concrete consumers rather than anticipated
through a generic registry.
