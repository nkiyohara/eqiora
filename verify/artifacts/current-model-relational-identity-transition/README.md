# Current Model relational identity transition

RFC 0083 resets the Model artifact epoch. Changing the Model digest changes a
downstream artifact **only** when that exact artifact embeds the Model
reference. This case is the independent oracle that classifies every checked-in
Model reference and preserves the exact alpha.1 Model-epoch observation. For
four later live artifacts it separates that history from eight release-owned
identities whose exact current values belong to their registered producer cases.

The classification is by producer semantics, not by a closed fixture list.
[`expected/classification.json`](expected/classification.json) records the
complete search, every classified entry, and the exact two-state transition
contract below;
[`expected/classification-inventory.txt`](expected/classification-inventory.txt)
freezes its 303 exact candidate paths so the registered test can repeat the
same repository sweep;
[`expected/transition.json`](expected/transition.json) records the precommitted
identities.

## What this case owns

- the complete classification of every Model-bearing fixture, and exactly one
  fate for each of the 303 candidate paths;
- complete precommitted Model-epoch canonical bytes for the four deterministic
  fixtures, plus every historical downstream identity they imply;
- four live expected-artifact projections in which exactly eight compilation,
  Run, and binding identity pointers are release-owned and every other raw byte
  remains equal to the sealed alpha.1 history;
- the canonical bytes of each downstream artifact whose identity changes, so
  every artifact-reference edge is re-derivable from bytes alone;
- the complete ALE segment-to-root identity chain, the opaque exact
  Realization v4 golden, and the structural semantic bridge for the two
  recorded accelerator bundles;
- the current Model input and the three replacement identities of the one
  consumer whose Model *input* the reset moves rather than whose fixture it
  rewrites; and
- the executable post-reset acceptance route for the retained Realization v4
  golden, which decodes no Model at all.

## The reset is an exact two-state transition

The repository is either wholly before the reset or wholly after it. There is no
sentinel file whose absence declares the reset done, and no directory, suffix, or
glob allowance: `search.transition` in
[`expected/classification.json`](expected/classification.json) names every path
by hand.

| Set | Count | Meaning |
| --- | --- | --- |
| `retired` | 52 | disappears; present in every pre-reset state, absent in every post-reset one |
| preserved | 261 | the rest of the frozen inventory; still exists afterwards, though an in-place migration may stop it matching the sweep |
| `required_post_reset` | 13 | the complete set of paths the reset may add: 11 byte-frozen promotions — 10 staged control-v2 targets plus the historical cylinder — and 2 existence-only unversioned Rust owners |
| `preserved_evidence` | 40 | invariant evidence — the same path in both states — whose deletion the reset must never reach |
| `promoted_evidence` | 1 | evidence whose bytes survive at a different path, so it is invariant at neither |
| `post_reset_admitted` | 37 | later identity-free classified paths the post-reset state may contain and never has to; a member of none of the historical sets above, and of no count in them |
| `post_reset_fixture_admitted` | 27 | later exact evidence representations admitted through a separate exact-path permission; a member of no historical, identity-free, or later-classified set |
| accepted-byte-bound optional rows | 10 | exact alpha.3 site/source rows guarded by accepted raw SHA-256; permission remains optional and owns no input semantics |

**52, 261 and 13 are not one partition of 303.** What partitions the inventory
is 42 + 261: the retired paths that are inventory members, plus the preserved
ones. The other ten retired paths carry no Model signal and were never members.
The 13 required paths are post-reset additions and replacements: twelve did not
exist before the reset, and the thirteenth,
`verify/interfaces/control-plane-compile-check/expected/contract.json`, is the
one existing inventory member whose bytes the reset replaces in place.

Forty-two retired paths are inventory members: the historical Model and
Transaction generation modules `model_v2`--`model_v7` and
`model_transaction_v2`--`model_transaction_v7`, the version-named current owners
`model_v8.rs` and `model_transaction_v8.rs`, the exact-codec host
`crates/eqiora-api/src/codec.rs` with its two v1-only test files, the
compatibility-only wire goldens, the Python `eqiora.compatibility` module and
stub, the live control-v1 schema, the superseded v7 cylinder resource, and the
four signal-bearing staged control-v2 files. The other ten carry no Model signal
and so were never inventory members: the seven staged control-v2 request
fixtures and the three live control-v1 request fixtures. The eight additional
inventory retirements are the displaced specialized Rust, Python, and Studio
FSI lifecycle paths now replaced by the common Model-first Plan/State/Run path.

### Version-named owners retire; unversioned owners replace them

RFC 0083 keeps `v8` in *persisted* names because they identify released bytes,
and makes the public Rust and Python owners unversioned. `model_v8.rs` and
`model_transaction_v8.rs` are source owners, not persisted names, so they retire
with the rest. The reset folds each wrapper together with the surviving current
encoding it delegates to into one unversioned owner —
`crates/eqiora-artifact/src/model_wire.rs` for Model and
`crates/eqiora-artifact/src/model_transaction_wire.rs` for Transaction — which
gives the exact current persisted wire an unversioned product owner without
growing the already large `model.rs` and `model_transaction.rs`.

Their **existence** is required and their **content** is not frozen here: this
case does not own the current encoding, `artifacts.current-model-canonical-identity`
does. They must also not exist before the reset, or the repository is mid-flight
rather than pre-reset.

The registered test root originally imported the version-named current owner.
The implementation rewires only that import to the unversioned current owner.
Rewiring a name is not authoring an oracle; changing any expected value,
tolerance, or frozen set in this case remains unavailable.

### The superseded cylinder becomes oracle input, not a product example

`examples/steady-flow-past-cylinder.model-v7.json` is read by the accepted
oracle `crates/eqiora-artifact/tests/current_model_wire_oracle.rs` as its
superseded specimen. After the reset it is historical evidence, and shipping it
from `examples/` would claim it is still a product example. Its exact bytes move
to
`verify/artifacts/current-model-canonical-identity/expected/historical/steady-flow-past-cylinder.model-v7.json`,
beside the other historical Model specimens, under a frozen 16,798-byte
promotion digest. The unversioned
`examples/steady-flow-past-cylinder.model.json` is the current resource and does
not move.

That is why invariant evidence and promoted evidence are separate lists. A
single `preserved_evidence` list holding both would make the pre state demand a
path the reset removes, or the post state demand one it never creates; splitting
them keeps the frozen pre state and the observed post state each exact.

**A proper nonempty subset missing is a partial transition and fails.** One
retired path removed is refused exactly like forty; one retired compatibility
surface surviving an otherwise complete reset is refused the same way. That is
what makes this a contract rather than a heuristic: the reset cannot land one
seam at a time and call each step green.

The previous sentinel keyed the post-reset state on the deletion of
`crates/eqiora-api/src/codec.rs` while permitting only twelve paths to
disappear. It refused a structurally correct reset — that same file, the
historical modules, the Python compatibility module, and every staged
control-v2 path had to go and were not permitted to.

## Promotion moves bytes, never meaning

Each staged file retires **only** once its exact live target exists carrying the
staged source's frozen digest — the v2 schema at
`schemas/control/compile-v2.schema.json`, the v2 expected contract in place of
the v1 one, the historical v1 schema copy, the seven request fixtures, and the
historical cylinder resource. The retired live `models/accepted-v1.json` and
`schemas/control/compile-v1.schema.json` bytes survive byte-for-byte as
`models/retired-v1.json` and the historical copy, so retiring control v1
preserves the exact request and schema it retires. Nine of the eleven digests
are also frozen independently by the control-v2 lane's own `fixtureDigests`;
this case consumes and re-derives them rather than authoring them.

Three of the promoted paths carry a Model search signal, and the two
unversioned wire owners carry one. The post-reset
predicate admits the 13 required paths on top of the preserved inventory, and
those five are the ones expected to be found by the sweep. An implementation
that needs another new signal-bearing path, or that cannot retire a listed one,
stops and returns the delta to this oracle. Widening a set here to fit an
implementation choice is not an available move.

## Later signal-bearing paths are admitted by exact path, never required

The transition is history; later work is admitted only as optional permission.
The original sixteen identity-free rows cover the Python trajectory and
Result surfaces, the Cartesian Q1 snapshot owner, RFC 0085 standalone-solid
surfaces, subprocess-provider surfaces, non-box transient oracle, and private
MCP adapter. Ten accepted alpha.3 site rows append in exact order:

- Cylinder gallery, control-v2 reference, MCP reference, and three generated
  Python reference pages, all `current-owner-assertion`;
- Gallery publication checker and pressure producer, plus the publication
  fixture helper and predicate test, all `non-fixture-search-hit`.

Every one carries its recorded ordered search signals and zero same-line
Model-derived identity occurrences. `post_reset_admitted` therefore contains
exactly 37 rows after eleven exact common Plan/FSI/compile source and test admissions. A row is absent before the reset, optional after it, belongs to
no historical, required, promotion, classified, fixture, or oracle-exclusion
set, and grants no sibling, stem, suffix, directory, generated-page, or tool
family permission.

The ten site rows additionally bind the raw SHA-256 of their already accepted
complete bytes. The same one-time read derives signal order, occurrence count,
and digest. Absence remains accepted; changed bytes with the same signal and
count fail the digest gate and return to an independent oracle successor. The
digest is a composition guard, not semantic ownership of the site prose, API,
science, producer, publication predicate, or test.

## Exact later fixture bytes use a separate permission

The first 26 fixture rows retain their exact RFC 0085, subprocess-provider,
Python-package, MCP, Stokes, collocated, and semantic-impact shapes and their
aggregate 120 occurrences. The current typed-compilation identity fixture is
the twenty-seventh row, with five exact compile-only identities. The aggregate
is therefore 125. The retired fixed-mesh Cylinder publication grants no
replacement fixture admission.

This permission is also containment-only and exact-path-only. Empty,
singleton, all, and all-but-one subsets of both admission vectors remain valid.
Deletion and wrong-path mutants cover every new row; representative Gallery,
Python, interface, publication, tool, and test neighbors remain unclassified;
category drift fails the ordered-row oracle; and inert byte drift reaches the
digest gate for all ten accepted-byte rows. Omitting, substituting,
uppercasing, malforming, or shortening one accepted digest, or adding a digest
to an older admission, fails without changing product bytes. None of these
checks transfers ownership of publication identities or makes a later site path
part of reset history.

## Later mixed identity evidence uses exact classification

The #114 installed-Python package oracle adds three exact paths that fit neither
admission permission. The sealed alpha.1 classification owns exact aggregate
`3/2/1/8`: three paths, two mixed assertion surfaces, one source-or-package
identity path, and eight identity occurrences. Each historical mixed row
contains the exact four recorded offline Model, offline compilation, typed
Model, and typed compilation
identities once, together with its immutable source, release, and resolution
identities. The stored package release remains a historical exact observation
containing only source/package identities. Treating all three paths as
identity-free would be false, while treating the stored release as a current
Model fixture would also be false.

The present live classification keeps the same exact paths, Model identities,
immutable identities, ordered search signals, and same-line lower-hex signal
counts, but gives the mixed rows live relational maps. The Python surface has
exactly nine lower-hex-64 occurrences, signals `[model_sha256, model_digest]`,
and same-line count zero. The Rust surface has exactly ten occurrences, no
search signal, and same-line count four; it alone retains the alpha.1 offline
compilation identity once as a historical positive. Both surfaces contain each
of the two delegated current compilation values once, and neither retains the
historical typed compilation identity; Python also does not retain the
historical offline compilation identity.

Those current values come from the exact release-owned slots
`offline-model-package.live` at `/compilation_digest` and
the live `packages.typed-compilation-lineage` compile-only authority at
`/package_compilation_sha256`. This case owns
their exact mapping, counts, immutability, and causal stale, substitution,
omission, addition, duplication/collapse, signal, and same-line mutants. It does
not own either exact current value: semantic correctness remains with Role C and
the ordinary producer evidence.

Like both admissions, classification is containment-only: none of the three
paths is required for reset acceptance or joins a historical, required,
promotion, identity-free, or fixture-admitted set. A fourth path gains nothing
from a shared directory, stem, extension, or nearby classification.

## Two consumers the remainder was wrong about

A remainder is only safe while every path it covers really does migrate in place
carrying no identity literal. Two files broke that, in opposite directions, and
both are named explicitly now.

### `moving_spatial_v2_wire.rs` migrates its Model input

The file reads its Model out of the historical fixed-reference CUDA bundle,
decodes it with `ModelEnvelopeV4`, and freezes three digests of what it then
builds — a SpatialState v2, a trajectory segment, and a prefix root. The
remainder covered it and says `identity_literals: 0`. Both halves are false: the
reset rejects those Model bytes, and the file freezes three Model-derived
identities.

It is not a flat-fresh consumer either. Its ULIDs, reference mesh, shear
schedule, snapshot seeds, and Field inventory are all fixed, and its Model comes
from bytes rather than a compilation, so nothing allocates a fresh occurrence. It
is a deterministic replay, and RFC 0083 already says what happens to it:
*spatial authored-field/context projections change only their Model input to the
current typed owner and retain the spatial artifact schemas and replay rules*.

So the input moves to the current Model of the **same semantic program** — the
bridge Model this case already precommits at
[`expected/bridge/fixed-reference-cuda-solve-2d/current-model.json`](expected/bridge/fixed-reference-cuda-solve-2d/current-model.json),
whose Model ULID and semantic revision are the historical ones and whose
generation-v2 fingerprint the bridge above proves equal. Nothing else may be
chosen: a Model the reset produces itself is a different Model, and every
downstream identity would move with it.

`model_input_consumers` in [`expected/transition.json`](expected/transition.json)
freezes the consequence exactly. All three artifacts are committed in **both**
states, because this consumer builds them at run time and has no checked-in
target file to compare against; without the pre-reset side there would be nothing
for the replacement to be a delta *of*. The two states are byte-length identical,
and the replacement is the pre-reset bytes with one 16-entry identity table
applied and nothing else — 87 leaves across the three artifacts, 33 identity
substitutions and the other 54 byte-identical. Coordinates, steps, times,
physical dimensions, Field
and Domain ULIDs, and the reference mesh digest are all on the untouched side.

The three digests the consumer freezes move with them:

| Frozen at | Pre-reset | Replacement |
| --- | --- | --- |
| `state_1.digest()` | `2cb018c9…b218aa` | `40f51f91…c7db38` |
| `decoded_segment.digest()` | `806b8b3d…630e92` | `e6f04b57…8a4d50` |
| `decoded_root.digest()` | `8a9f5359…3c72a8` | `8b2ee5f2…bcb0b3` |

### `realization_v4_wire.rs` keeps its golden and loses its decoder

The v4 golden is a retained separate-family golden and its Model reference stays
opaque — that much the classification already said. What it did not say is that
the file *reconstructs* the golden: it decodes the same historical Model with
`ModelEnvelopeV4` and re-encodes a Realization over it. That route disappears
with the historical decoders, and the two obvious repairs are both wrong.
Handing those bytes to the current Model owner is admitting a historical schema,
and rebuilding the golden over a current Model is relabelling it.

`retained_family_goldens` freezes the third route, which needs no Model decoder
at all: the Realization family is retained, so its own decoder reads the
committed 8,333 bytes, re-encodes them canonically, and reproduces
`ba9efbdb…b5d9e` and the RFC 0008 artifact digest. The Model reference is
compared as a string and never resolved. The golden literal is unchanged; this
amendment adds no replacement for it.

Relabelling gets its own falsifier because nothing inside the artifact can catch
it. A golden whose `model_sha256` is swapped for the current bridge digest keeps
its Model ULID and revision, still decodes, and still *passes*
`validate_model_artifact` against the current Model. The exact bytes are the only
thing that refuses it, so the bytes a relabelled golden would carry are frozen
too.

### The handoff is a list of operations, not a target to hit

Both entries carry a `handoff` with the exact operations and the exact forbidden
moves, so the implementation wires values rather than searching for them. For
`moving_spatial_v2_wire.rs`: repoint `MODEL` at the current bridge Model,
stripping one optional trailing line feed; replace the `ModelEnvelopeV4` import,
the `Resources::model` field type, and the two `ValidatedMovingSpatialContextV2`
type parameters with the unversioned current owner; and substitute the three
digest literals, each of which occurs exactly once. For `realization_v4_wire.rs`:
add the golden as `include_bytes!`, make the frozen-digest test hash and
round-trip those bytes instead of `Fixture::new().envelope()`, and repoint
`MODEL` at the same current Model so the surviving constructive tests keep an
ordinary current-owner path.

Nothing else in either file changes. Every assertion, seed, coordinate, decoder
limit, and rejection case is retained, and no assertion is relaxed. If an
operation cannot be applied as written, the lane stops and returns the delta
here; choosing a different Model, a different target path, or a different digest
is not an available move.

## Every candidate has exactly one fate

`classification.json` names 113 of the 303 candidates in an entry and leaves the
other 190 to the `non-fixture-search-hit` remainder. A remainder is what keeps
the classification complete without listing every path twice, and it is also
where a classification can quietly say the wrong thing: "everything else
migrates in place" stops being true the moment a path the reset *removes* is
left unnamed.

So `dispositions` declares the seven fates an entry may assign — `delete`,
`rename-source`, `delegate`, `migrate`, `preserve-bytes`,
`decompose-by-claim`, and the remainder's `migrate-in-place` — every
path-bearing entry declares exactly one, and no path is named by two entries.
All 42 retired inventory members are named explicitly: fifteen as
compatibility-only deletions (the `v3`--`v7` generation modules, the
exact-codec host with its two v1-only API tests, and the Python compatibility
module with its stub), eight as the displaced application-shaped FSI lifecycle,
two as version-named current owners the reset *renames*,
two as the v2-named source files *decomposed by claim*, and fifteen by the
fixture, delegated, and remaining mixed-claim entries. The remainder therefore
excludes retired paths by construction, and both routes check that it does.

`model_v8.rs` and `model_transaction_v8.rs` are the reason `rename-source`
exists as a fate. They are not compatibility-only — they host the exact current
persisted wire, which survives — so calling them deleted would misstate what
happens to the encoding, and leaving them to the remainder would call a removed
path an in-place migration.

`model_v2.rs` and `model_transaction_v2.rs` are the reason `decompose-by-claim`
reaches product source and not only tests and fixtures. The version in a file
name is the generation the module was born for, not the only one it still serves:
RFC 0083 says in as many words that "the current implementation hosts v8
encoding in `model_v2.rs`", and current v8 delegates to
`ModelEnvelopeV2::from_program_v8`, `from_json_v8`, and `digest_v8` with the
Transaction functions alongside. So their fate is not one verb. The historical
`V2`--`V7` admission and per-generation selection are deleted; the exact current
v8 encoder, decoder, and digest **migrates** to the matching unversioned owner —
`model_v2.rs` to `model_wire.rs`, `model_transaction_v2.rs` to
`model_transaction_wire.rs`. Calling either file compatibility-only would have
authorised deleting the current encoder along with the branch the reset is
actually removing.

The four retired v2/v8 source files therefore reach the same two unversioned
owners by two different routes, and the mapping is per file in both. Both routes
check the `from`/`to` pairs in order rather than as sets, so a reset that sent
Model to `model_transaction_wire.rs` fails instead of passing on membership.

## The oracle excludes its own two executor files, exactly

This case's executable oracle is two files: the integration-test root
`crates/eqiora-artifact/tests/current_model_relational_identity_transition.rs`
and the private support module
`current_model_relational_identity_transition/transition_contract.rs` it
includes with `#[path]`, so the case keeps one Cargo integration-test target.
The split is by responsibility — embedded literals and the identities derived
from them in the root, the repository sweep and the transition contract in the
module — and it is what keeps both files inside the 2,000-line test ceiling
without a debt entry.

Both files spell the tokens the sweep searches for, so both are excluded from
it, by exact path, declared in `search.excluded_paths`. Not by their directory,
not by a suffix rule, and not by anything resembling "tests": every other test
file in the repository, including the sibling oracle
`crates/eqiora-artifact/tests/current_model_wire_oracle.rs`, stays a classified
candidate. Both routes check that the exclusion is exactly those two paths, that
each one really would be found without it, and that a third file added beside
them arrives as an unclassified candidate to be returned here.

## Path existence cannot see a private branch

A file that survives the reset can still hold the branch the reset exists to
delete. `encode_v1`--`encode_v8` and `ensure_v1`--`ensure_v8` live today inside
`model.rs`, `model_transaction.rs`, `model/node.rs`, `model/expression.rs`, and
`model/vocabulary.rs` — every one of which is preserved. A contract that only
counted paths would call that reset complete.

`search.forbidden_product_tokens` closes it: three narrowly frozen
product-source scopes, 102 exact case-sensitive substrings, evaluated **only**
in the post-reset state.

| Scope | Reaches | Forbids |
| --- | --- | --- |
| `rust-product-source` | `src/` of the four `eqiora*` crates and the Studio Tauri host | 90 tokens: the versioned public spellings, the wire DTOs, the generation selectors, the private per-generation encode/admit/derive branches, and the exact v1--v7 Model and Transaction schema strings — 86 of which the pre-reset tree carries |
| `python-product-source` | the `eqiora` package, its stubs, and the shipped Python examples | 6 tokens: the compatibility module and every exact-generation selector — all 6 present today |
| `control-product-source` | the control plane, the Python control host, `studio/src-tauri/src/compile.rs`, and Studio product TS/TSX | 6 tokens: the caller-selected Model generation inputs of control v1 — all 6 present today |

### 98 of the 102 are there to remove; four are prospective

A post-reset-only contract is worth nothing if the tokens were never there in
the first place, so `pre_reset_occurrence` measures that instead of asserting
it. Of the 102, **98 occur** in the files their own scope covers, and **four do
not**: `from_program_v2`, `from_json_v2`, `from_transaction_v2`, and
`digest_v2`. No product source spells those four today. They sit beside
`from_program_v3`--`from_program_v8` and their siblings, which is exactly where
a renamed historical v2 branch would most plausibly reappear while the reset is
being written, so they stay forbidden after it — a prospective guard costs
nothing and catches that. Claiming they occur now would simply be false, and
this case does not.

Both routes freeze that presence and absence exactly — 86 of 90 Rust tokens,
6 of 6 Python, 6 of 6 control — and re-measure it against the working tree, so a
scope that silently stops carrying a token it forbids, or that already carries
one of the four, fails here. **After the reset all 102 are forbidden**, present
ones and prospective ones alike.

Matching is exact substring, deliberately. Recognizing a `#[cfg(test)]` module
or a comment needs a permissive textual parser, and a parser that guesses what a
scope contains is the thing this contract replaces. `*.test.*` and `*.spec.*`
are filename patterns needing no parser, and the single Rust test-only module in
the control scope is excluded by its exact path. That exclusion is scoped to the
control tokens alone: a control test may legitimately name a field it proves the
v2 decoder rejects, while a historical Model spelling in product Rust is a
branch wherever it sits. A reset that adds another test-only module in scope
returns that path here rather than assuming it.

Four tokens are **deliberately permitted** and the contract fails if any is
forbidden: `eqiora.control/v1`, because the v2 decoder's exact rejection
diagnostic may name the protocol it refuses; `eqiora.model-envelope/v8` and
`eqiora.model-transaction-envelope/v8`, because persisted names identify
released bytes; and `model.compile-check/v1`, a command name rather than a Model
generation selector. Only the full old schema string is forbidden, never bare
`vN` text, so `eqiora.realization-envelope/v4` and every retained separately
versioned family are untouched.

Nothing under `verify/`, `crates/*/tests/`, `bindings/python/tests/`, `rfcs/`,
`docs/`, `CHANGELOG.md`, or `schemas/` is scanned. Those hold the negative
corpus and the historical record the reset must keep; forbidding a token there
would delete the evidence that the reset happened.

## What the transition predicate does not claim

For paths it bounds which exist, not what survives inside them: a preserved file
that is emptied still passes, only the eleven promotion targets have frozen
bytes, and the two unversioned wire owners are required by existence alone. It
covers the signal-bearing inventory plus the twenty-two paths named explicitly
outside it — ten retired, twelve added — so deleting some other file that never
carried a Model reference is invisible here and belongs to that file's own
owner.

The token contract bounds tokens, not behaviour: a file that spells no forbidden
token may still hold a historical branch under another name, and a scope that is
empty after the reset passes vacuously. It says nothing about any path outside
its three declared scopes. The independently recorded pre-reset occurrence
freezes exactly which 98 of the 102 tokens existed before implementation; the
observed post-reset checkout must contain none of them.

The registered test now requires the observed repository to be the complete
post-reset state. Synthetic exact-pre-reset, maximal-post-reset, and partial
states remain falsifiers for the same predicate. A preserved path migrated in
place may stop matching, which is admissible and which the predicate allows by
containment rather than equality.

Admission claims less again. It does not require any admitted path to exist and
owns none of their content: only exact path, category, signal order, occurrence
count, and—on the thirteen new rows—accepted bytes are bounded. The exact-B
tree exercises all thirteen byte-bound rows through the live sweep. Synthetic
post-reset states exercise empty, singleton, all, and all-but-one optional
boundaries and every causal mutant through the same byte reader.

The sweep reads checked-in content. Building the Python extension copies example
resources into maturin's staging directory
`bindings/python/python/eqiora/examples`, so a tree that has run the gate holds
an untracked build copy of an already classified Model resource there; that one
exact directory is excluded, like `target` and `__pycache__` beside it. Without
that exclusion the sweep reports its own build output as an unclassified
candidate, which is how this case failed `local_verify.py fast` before the
exclusion existed. Nothing under that path is checked in today, and a file that
became checked in there would be outside this sweep and would need
classification by hand.

## What this case does not own

The control-v2 accepted Model facts belong to the control oracle. The packaged
`steady-flow-past-cylinder` resource and its replacement digest belong to
`artifacts.current-model-canonical-identity`. The fingerprint byte projection
belongs to RFC 0073; this case owns only the *equality relation* between two
freshly observed fingerprints. Every scientific array, coordinate, field, time
value, tolerance, balance, convergence result, source identity, and package
identity is an immutable input here.

The two consumer files above are not owned here either. This case freezes their
Model input, their replacement identities, and the moves that must fail; it does
not own their assertions, their fixture construction, their decoder-limit and
rejection cases, or the wording beside them. It does not reconstruct the
moving-spatial producer: the committed artifacts are the exact bytes that
producer emitted through its already-live current encoder, and the registered
test replays them through the retained spatial decoders rather than rebuilding
the fixture that made them. The Realization and spatial wire families keep their
schema identities and meanings unchanged — the only thing that moves inside them
is a Model reference.

## The four producer classes that carry a Model reference

| Class | Rule | Members |
| --- | --- | --- |
| deterministic | complete precommitted current Model bytes and every permitted downstream identity | packaged DC motor, composed package, offline package, typed-execution lineage, fixed-topology ALE 3D, and the moving-spatial v2 wire consumer whose Model *input* moves |
| flat fresh occurrence | shape, exact-identity inequality, fingerprint equality, same-execution linkage — never an occurrence-dependent literal | control v2, agent-authored change, Model-reference lineage, geometry-to-Model, FSI spatial trajectory, and the digest-relation cases |
| historical recorded execution | untouched bytes plus the semantic bridge; never a relabelled current Run | canonical Cartesian Poisson CUDA, fixed-reference CUDA FSI |
| retained separate-family golden | exact bytes, opaque Model reference, accepted through its own family decoder | Realization v1--v3 goldens, the Realization v4 golden |

The composed-package, offline-package, typed-execution, packaged DC-motor, and
fixed-topology ALE fixtures are deterministic, not flat-fresh. Their
cross-run reproducibility claims are retained; the same-execution linkage
assertions this case adds do not replace them.

## Why the deterministic literals are independent of the implementer

The reset writer is a different lineage and cannot influence these values: the
accepted deterministic producers were replayed here through their already-live
current encoder before the reset existed, and the observed bytes were then
**discarded as authority**. Every committed identity is re-derived from the
committed bytes alone — canonical JSON re-rendered from the wire contract, the
RFC 0008 schema-domain preimage rebuilt by hand, SHA-256 taken with `hashlib`,
and each artifact-reference edge read out of the downstream bytes. See
[`references/`](references/README.md). The implementation installs these exact
values; it may not regenerate or select them.

## Identity-only containment

The sealed deterministic tree is the byte-exact alpha.1 history. Its five Model
artifacts and every downstream artifact still re-derive all 27 recorded
identities, and every superseded Model-epoch identity remains excluded there.
That historical positive runs before the live projection in one invocation.

The five registered live expected artifacts may differ from history only at
this fixed ordered interface: composed compilation (one pointer); offline
compilation, Run, and binding (three); typed compilation, Run, and binding
(three); packaged DC-motor compilation, Run, and binding (three); and ALE Run
provenance (one). Each live value must be a distinct lowercase-hex-64 scalar
with one exact raw occurrence. Replacing those eleven scalar spans with the
same position-specific holes must make the complete historical and live bytes
equal, including key order, whitespace, number spelling, terminal LF, every
Model/package/source/resolution/Realization/trajectory/geometry identity, and
all scientific values. All eleven alpha.1 values being retained is the
intentional exact-parent RED. The transition case does not know or select the
eleven current values; the owning producer cases and release assertions do.

The moving-spatial consumer commits **both** states, so there the identity-only
delta stays observable and is proved as one: the replacement is byte-length
identical, keeps the same leaf set in the same order, changes exactly the frozen
pointer set, moves each changed leaf only to the value the frozen identity table
gives it, leaves every other leaf byte-identical, and is reconstructed from the
pre-reset bytes by applying that table alone.

Those two claims — exact historical/non-release projection and the observed
moving-spatial leaf substitution — not release digest selection, are what this
case asserts here.

## The historical bridge is exact evidence, not prose

For each recorded accelerator bundle the case records the untouched artifact's
raw hash and its RFC 0008 artifact digest — both re-derivable from the bytes
without a product decoder — together with a freshly observed
`SemanticFingerprintGeneration::V2` value. It then commits a current Model
artifact built from that same decoded semantic program and records the same
fields. The fingerprints agree while the schema-domain artifact digests differ.

Source identity is deliberately not a bridge field: the CUDA FSI bundle has no
checked-in source, and source-identity construction is not owned here. The old
Run observed the **historical** artifact; current semantic equivalence is
independently bridged. Neither this case nor any wording derived from it may say
that the current Run was observed.
