# Studio typed value transaction verification

This case verifies Studio's first canonical editing slice. It proves that a
small ergonomic interaction is still the same optimistic, typed graph
transaction used by other Eqiora clients; it does not make React or Tauri a
second model implementation.

## Contract under test

```text
selected immutable document + finite coherent-SI scalar
                         ↓ native preview
current ModelTransactionEnvelope
  RevisionIs(base) + ValueEquals(target, before) + SetValue(after)
                         ↓ exact value-edit plan key
native reconstruction + atomic graph commit
                         ↓
immutable child document + typed lineage evidence
                         ↓
bounded UI revision navigation
```

The editable vocabulary is intentionally one revision-local scalar on a
`Field` or `Parameter`. The replacement retains the canonical physical
dimension. `ValueEditPlan` owns base digest, base revision, stable target,
before/after quantities, exact current transaction digest, and a
domain-separated preview-to-commit key. `ModelDocument::commit_value_edit`
canonicalizes and decodes the stored transaction again through the current
Transaction owner, checks its digest, clones the base graph store, performs one
atomic commit, rebuilds the typed kernel program, and returns a current child.
The base document remains usable and unchanged.

Studio bridge v5 treats both directions as untrusted. Zod validates every
frontend response; Rust checks protocol, document cache membership, target,
numeric finiteness, request bounds, and exact plan key. The result schema also
requires the projected document digest/revision to equal its edit evidence.
`ST0006` denotes a value-edit plan whose exact identity cannot be replayed.

The frontend lineage stores at most 24 immutable projections; the native
session stores at most 32 documents along the active lineage. Undo/redo selects
retained entries and does not create inverse transactions. A commit after undo
branches from the selected base and prunes the abandoned forward path in both
the UI and native session. The editor labels the original text as the source
basis after a child exists; recompiling starts a new root instead of silently
inventing source text.

## Falsifying cases

- empty, non-finite, overlong, and no-op input cannot produce a plan;
- a relation, activation, missing target, or scalar-less entity cannot be
  retargeted as a value edit;
- preview cannot change the physical dimension;
- a stale base revision or changed previous value fails its optimistic
  precondition instead of being replayed onto newer state;
- a forged plan key or transaction identity fails before mutation;
- a historical or otherwise unsupported Transaction schema fails before
  mutation;
- commit returns revision 2 for the first child while the base remains
  revision 1 and retains its previous value;
- an obsolete asynchronous preview cannot replace a newer input, selection,
  or document;
- undo/redo changes the selected immutable projection, and commit after undo
  creates a branch instead of retaining an invalid forward path;
- model layout follows stable canonical IDs but never enters edit evidence;
- recompiling the source basis replaces the transaction lineage explicitly;
- run evidence for another canonical digest is visibly stale after editing;
- inspector commit, command-palette commit, and keyboard undo/redo expose the
  same reducer actions without requiring a canvas drag; and
- the typed editor, transaction preview, lineage controls, focus treatment,
  minimum shell, and WCAG 2.2 automated gate remain usable at supported sizes.

## Commands

```bash
cd studio
npm ci
npm run check
npm test
npm run build
npm run test:e2e
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo clippy --manifest-path src-tauri/Cargo.toml --locked --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml --locked
npm run tauri build -- --no-bundle
```

The public application-service contract remains in the normal core gates:

```bash
cargo test -p eqiora-api --locked
cargo clippy -p eqiora-api --all-targets -- -D warnings
cargo test --workspace --locked
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

## Nonclaims

This case is not evidence for connection/topology edits, vector/tensor or
array values, non-coherent unit entry, a portable transaction-history
artifact, lossless source rewriting, collaboration, CRDT/OT conflict
resolution, or unbounded history. Each requires its own typed consumer and
falsifiable verification case.
