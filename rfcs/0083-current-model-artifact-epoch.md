# RFC 0083: One current Model artifact epoch before 1.0

- Status: Accepted contract; implementation tracked by Issue #258
- Authors: Eqiora contributors
- Created: 2026-07-31
- Depends on: [RFC 0037](0037-version-neutral-model-artifact-reference.md),
  [RFC 0054](0054-curated-facade-and-control-plane.md),
  [RFC 0077](0077-exact-cartesian-domain-edit.md), and
  [RFC 0078](0078-direct-parameter-driven-cartesian-coordinates.md)
- Supersedes: the live multi-generation Model/Transaction compatibility policy
  in RFC 0037 and the stable `eqiora::compatibility` decision in RFC 0054

## Summary

Eqiora will use its pre-1.0 period for one explicit Model artifact epoch reset.
The accepted Model and Model Transaction v8 meanings become the only live
runtime contracts. Their persisted schemas, canonical bytes, digest domains,
decoder limits, and semantic meaning do not change, but product APIs stop
presenting v8 as one selectable member of a historical v1--v8 ladder.

Model and Transaction v1--v7 decoders, encoders, selectors, compatibility
tests, and compatibility-only fixtures are removed. Old bytes reject as
unsupported. No schema identifier is reassigned, no decoder sniffs or retries,
and no migration is inferred from semantic similarity.

This is a bounded breaking compatibility decision, not a general removal of
artifact versions. Every other artifact family keeps its current contract
until a separate decision names it.

## Why now

Direct Parameter-driven Cartesian coordinates and their atomic regeneration
owner close the current Model meaning required by the next CAD work. Keeping
eight Model and Transaction branches would force every new Studio, Python,
CAD, package, and project consumer to carry compatibility policy that the
public alpha has not promised to preserve.

Removing that ladder after those consumers exist would be more expensive and
would reward a temporary architecture with permanent product shape. The reset
therefore lands after RFC 0078 and its regeneration slice, and before the
shared Studio/Python CAD authoring graph.

## Frozen current contract

The implementation retains exactly these persisted identities:

| Contract | Frozen identity |
| --- | --- |
| Model schema | `eqiora.model-envelope/v8` |
| Model Transaction schema | `eqiora.model-transaction-envelope/v8` |
| Canonical encoding | `eqiora.canonical-json/v1` |
| Model meaning | the complete accepted v8 Semantic Kernel vocabulary |
| Transaction meaning | the complete accepted ordered v8 transaction vocabulary |
| Model digest domain | the existing v8 Model schema domain |
| Transaction digest domain | the existing v8 Transaction schema domain |

The producer-owned reference fixture in
`crates/eqiora-artifact/tests/model_v8_wire.rs` currently observes:

| Fixture output | Exact value |
| --- | --- |
| Model canonical byte length | `2347` |
| Model SHA-256 | `e410295337a3a51a271f272e03ae7d7a4b8e7df1b04faf76645bb1e18567e4b3` |
| Transaction canonical byte length | `2646` |
| Transaction SHA-256 | `132168803ac8882f0f35187215d3f2ce44817d03921d6ad95b73a9cac62aa102` |

These candidate values commit the current producer's exact ordering, but the
producer fixture is not an independent oracle. Before implementation, another
provider lineage must independently freeze the complete canonical Model and
Transaction JSON for the same public fixture, reproduce both hashes, and
commit old-schema specimens used only as a negative rejection corpus. A
semantically equivalent fixture with different local order is not byte
evidence. The implementation writer may not author, tune, or relax that
oracle.

The same independent lane must re-encode and freeze the shipped
`steady-flow-past-cylinder` Model as current v8. It owns the new complete
canonical bytes, artifact digest, raw resource digest, byte length, and
unversioned packaged resource name used by the Python and Studio registered
cases. It also owns the replacement registered identity oracle
`artifacts.current-model-canonical-identity`; consumers replace the dangling
`model.model-envelope-v7-canonical-identity` reference with that exact case.
The lane proves that the semantic Model identity, revision, and replayed
program are unchanged while the schema-domain artifact identity changes from
v7 to v8. The implementation writer may wire those values but cannot derive
or adjust them. The existing fluid, geometry, mesh, balance, and solver
oracles remain unchanged.

Persisted names retain `v8` because they identify released bytes. Public Rust
and Python owners become unversioned current owners; the suffix is not reused
for future meaning. A future incompatible Model meaning requires a new schema
identifier and a new compatibility decision.

## Removal inventory

The following live Model/Transaction compatibility surfaces are removed
together:

- `ModelEnvelopeV1` through `ModelEnvelopeV7` and
  `ModelTransactionEnvelopeV1` through `ModelTransactionEnvelopeV7`, including
  their public exports, runtime encode/decode branches, and old schema
  admission;
- the public v8 spellings `ModelEnvelopeV8` and
  `ModelTransactionEnvelopeV8`, replaced by one unversioned current
  `ModelEnvelope` and `ModelTransactionEnvelope`;
- `ModelArtifactGeneration`, the generation variants inside
  `AcceptedModelArtifact`, and the versioned Transaction wrapper;
- Rust `ExactModelCodec`, `eqiora::compatibility`, exact-codec constructors,
  capability predicates, codec fields retained by `ModelDocument`, and caller
  generation selection in compile, define, replay, edit, and package paths;
- Python `eqiora.compatibility`, its `ExactModelCodec`, corresponding stubs,
  and exact-generation arguments;
- Studio and control-plane inputs that let a caller choose a historical Model
  generation; and
- compatibility-only v1--v7 wire goldens, route matrices, documentation, and
  capability claims.

There is no deprecated alias or facade for these names. A compatibility facade
would preserve the architecture this reset exists to delete.

Shared implementation code may survive under current, unversioned ownership
when v8 still uses it. In particular, the current implementation hosts v8
encoding in `model_v2.rs` and shares the canonical-encoding constant and wire
DTOs in the original Model/Transaction modules. The owner slice moves or
renames that code under current ownership while preserving its exact behavior;
retaining a private helper does not retain `ModelEnvelopeV2` or its decoder. A
file or helper is not retained merely because deleting it is inconvenient,
nor is current logic rewritten merely because it originated in an older
module. The checked outcome is one current path and no branch that admits or
emits a historical schema.

## Consumer migration inventory

The reset is incomplete until every row reaches the current owner:

| Consumer seam | Required migration |
| --- | --- |
| `eqiora-artifact` | expose one current Model and Transaction owner; retain exact v8 wire and typed replay/reference checks |
| `eqiora-api` and facade | make `ModelDocument` current-only; remove codec state, exact selectors, and the stable compatibility namespace |
| control compile/check | retire control v1 from ordinary dispatch without reinterpreting it; introduce control v2 without caller-selected Model history |
| compiler and package paths | emit the current Transaction and Model only; recompute dependent exact artifact references without changing source/package semantics |
| Rust applications and examples | replace fixed historical codec choices with the current ordinary path |
| Python extension, package, stubs, and tests | expose one ordinary current path and reject old persisted Model bytes |
| Studio native commands and demonstrations | consume the same current Rust owner without a route-local switch |
| spatial authored-field/context projections | change only their Model input to the current typed owner; retain the spatial artifact schemas and replay rules |
| verification and documentation | preserve semantic and scientific assertions, remove compatibility-only claims, and index only the current artifact boundary |

The inventory currently spans `crates/eqiora-artifact`,
`crates/eqiora-api`, `crates/eqiora`, `crates/eqiora-python`,
`bindings/python`, `studio`, `packages`, `examples`, `verify`, `docs`,
`schemas`, `tools/ci`, `tools/docs`, `tools/release`, `tools/xtask`, `api`,
`CHANGELOG.md`, `pyproject.toml`, and affected RFC references. This list is an
ownership boundary, not permission to make unrelated cleanups in those trees.

## Evidence retention rule

Artifact history and scientific evidence are different things.

- Delete a fixture only when its sole claim is that a removed Model or
  Transaction generation remains callable or byte-stable.
- Migrate a fixture when it proves semantic admission, rejection, geometry,
  package, numerical, physical, or lineage behavior independent of the
  removed generation.
- Preserve every scientific expected value, tolerance, balance, convergence
  order, mutant, and falsifier unless a separate oracle review proves it
  wrong.
- Regenerated current artifact digests are implementation outputs, not
  permission to alter an oracle.
- Keep the v8 exact Model/Transaction fixture above and current malformed,
  bounded-decoder, canonical-order, replay, and wrong-schema rejection tests.

`artifacts.model-reference-lineage` and
`interfaces.current-authoring-profile` currently contain historical
compatibility claims. `interfaces.control-plane-compile-check` is a control-v1
case and must be replaced by a control-v2 conformance kit rather than silently
narrowed. `interfaces.python-control-plane` retains exact-codec references that
must move to the current owner.

Unknown-protocol falsifiers in Rust unit/integration tests, Studio tests, and
registered control fixtures currently use the future spelling
`eqiora.control/v2`. Before v2 becomes valid, every such falsifier moves to a
deliberately invalid non-version spelling such as
`eqiora.control/unknown-test`. The replacement control-v2 conformance kit
retains that invalid-spelling rejection; losing unknown-protocol coverage is
not an allowed migration.

The packaged cylinder resource is an oracle input, not merely producer output.
`interfaces.python-exact-cylinder-stokes-result` and
`interfaces.studio-exact-cylinder-stokes-demo` must consume the independently
frozen current resource and retain all non-codec scientific and lineage
assertions.

The implementation PR updates `docs/capability-matrix.md`, especially the
rows for deterministic serialization, versioned semantic Model artifacts,
typed Model artifact references, exact package-to-Run lineage, curated Rust
facade, shared application service, current authoring versus exact codecs,
Transaction wire, Python source/native authoring, and Python revision/edit
surfaces. The matrix must describe the implemented current-only boundary, not
the accepted plan. Downstream Realization/Run identity and current-authoring
assertions that remain true must survive.

The existing `model_v8_wire.rs` is decomposed rather than retained verbatim.
Its positive current fixture, canonical permutation checks, and operation-order
checks move to the independent full-byte oracle. Its calls into v1--v7
encoders disappear with those encoders. Literal old-schema specimens form the
separate negative corpus that proves the current decoder rejects historical
bytes without recreating historical runtime branches.

## Separately versioned families retained

This RFC does not collapse or rename these artifact families:

| Family | Retained owners |
| --- | --- |
| CAD and Geometry | CAD design/build evidence, Geometry definition/identity/state/revision association, mesh correspondence, and circular-hole source binding |
| Mesh and import | simplicial Mesh, revision overlap, external-import manifests/observations, resolved arrays, and root registration |
| Realization and execution | Realization v1--v5, Realization references, Run manifests, distributed layouts/systems, and execution provenance |
| Time and restart | time lowering/run, general implicit time, checkpoint, and restart artifacts |
| Spatial data | discrete Fields, spatial states/trajectories and segments, storage chunks, and XDMF/HDF5 trajectory storage |
| Transfer and derived data | remesh transfer/projection, datasets, and ML dataset artifacts |
| Exposure and comparison | physical-exposure artifacts and structural semantic fingerprints |

Their suffixes identify different wire families, not historical Model
compatibility. Any later current-only reset must prove its own ladder,
consumers, exact retained contract, and rejection behavior.

Retaining a family means preserving its schema identity, digest domain,
decoder meaning, and public contract. A Realization, Run, spatial context, or
other exact instance that names a migrated Model digest necessarily receives
new relationally derived bytes and identity; its independently owned semantic
and scientific assertions do not change. Fixtures unrelated to a migrated
Model reference remain byte-identical.

## Control and release behavior

The control protocol remains an independently versioned family.
`eqiora.control/v1` includes caller-selected `modelWire` meaning and therefore
cannot be rewritten in place as current-only. Its schema identity and
historical bytes are not reassigned; ordinary runtime dispatch retires v1 and
rejects it as unsupported.

A new `eqiora.control/v2` compile/check contract removes `modelWire` from the
request. Its response may report the fixed current Model schema as an observed
output fact, but not as caller policy. Studio, Python, and Rust consume v2
without a route-local generation switch. This RFC does not otherwise collapse
the control protocol family.

The Python pre-1.0 release policy permits this immediate removal only because
this RFC fixes the scope, retained bytes, rejection behavior, consumer
inventory, and migration note. This exception does not authorize unrelated
public API removal or silent behavior changes.

The implementation records the break under the `Changed` heading of
`CHANGELOG.md` `[Unreleased]`. That entry names the removed Rust
`ExactModelCodec` and
`eqiora::compatibility` surfaces, Python `eqiora.compatibility`, Model and
Transaction v1--v7 runtime support, and control v1 dispatch. It names the
ordinary current replacements: Rust `ModelDocument::{compile, define,
replay}`, unversioned current Model/Transaction artifact owners, Python
`eqiora.compile`, `Model.define`, and `eqiora.replay`, plus control v2. It also
states that old Model/Transaction bytes and control-v1 requests reject and
that no automatic migration is provided. Release notes derived from the
changelog retain those facts.

## Implementation ownership and order

1. Merge this decision and inventory before implementation.
2. A different provider lineage independently freezes the complete canonical
   v8 Model/Transaction fixture, the v1--v7 negative rejection corpus, and the
   current packaged cylinder Model resource. Implementation does not begin
   until the reference fixture agrees with the candidate lengths and digests
   above.
3. One writer owns the invariant-bearing Model/Transaction seam across the
   artifact owner, API, facade, and every compiling consumer. The accepted
   change must not leave a second compatibility path.
4. Migrate Python, Studio, packages, demonstrations, and registered evidence
   through that same current owner. Work may be parallelized only after its
   consumed contract and writable paths are disjoint.
5. Remove historical modules and ratchet public-surface, file-size, and
   architecture counts down. Do not add a debt entry or raise a ceiling as an
   ordinary step of the reset.
6. Consider other artifact families separately after this reset is accepted.

If a mergeable intermediate revision would require a public compatibility
alias, broaden the owner slice instead of introducing the alias.

## Required falsifiers

- bytes with Model or Transaction schema v1--v7 reject before semantic use;
- control v1 rejects as unsupported and cannot be interpreted as control v2;
- replacing an old schema string with `v8` cannot make old bytes admissible;
- unknown Model/Transaction schemas reject without sniffing, retry, fallback,
  or migration;
- the current v8 fixture retains the exact bytes and digests frozen above;
- no Rust, Python, Studio, control, package, example, or verification caller
  can select a historical Model codec;
- Rust, Studio, and registered control-v2 evidence reject one deliberately
  invalid non-version protocol spelling;
- the release note names every removed public route, its current replacement,
  old-byte/control-v1 rejection, and the absence of automatic migration;
- no accepted consumer remains pinned to a removed envelope;
- changing only a removed generation does not delete or relax an independent
  scientific oracle;
- Realization, Run, Geometry, Mesh, spatial, and other retained artifact
  families keep their schema identities and meanings; only instances that
  name a migrated Model acquire relationally derived new identities; and
- public-surface and architecture counts do not rise to accommodate the reset.

## Alternatives

### Keep v1--v8 behind a compatibility namespace

Rejected. That is the present architecture and continues the audit and
fan-out cost.

### Auto-detect the schema from JSON

Rejected. Sniffing turns malformed or substituted bytes into a decoder search
and makes rejection behavior depend on ordering.

### Rewrite v8 as a new v1

Rejected. Reusing an old schema identifier would reinterpret persisted bytes
and destroy content identity.

### Remove every version suffix in the repository

Rejected. Unrelated artifact families have independent meanings and
compatibility histories.

### Keep aliases for one prerelease

Rejected for this bounded epoch. An alias would keep the generation ladder in
the exact period when new CAD and project consumers would begin depending on
it.

## Nonclaims

This RFC does not promise 1.0 stability, automatic migration, an offline
migrator, cross-schema semantic equality, a universal artifact envelope, or a
reset of any non-Model artifact family.

It does not change Semantic Kernel meaning, physical equations, numerical
methods, scientific tolerances, Geometry meaning, mesh correspondence,
execution plans, Result meaning, or visualization. It does not authorize
deleting evidence whose claim survives the wire reset.
