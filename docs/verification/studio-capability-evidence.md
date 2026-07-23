# Studio accepted-plan and evidence verification

This case verifies the second Eqiora Studio projection slice. It does not make
the WebView an execution authority.

## Contract under test

```text
editable model-time strings
        ↓ bounded validation
native ReferenceRunPlan preview
        ↓ exact versioned plan key
native key replay + detached execution
        ↓ successful result only
typed run evidence
        ↓ bounded projections
trajectory / semantic table / evidence inspector
```

The shared Rust application service owns the plan. Studio bridge v5 projects
the adapter and version, host placement, integration and nonlinear methods,
tolerances, event controls, and safety limits. Submission is accepted only if
the reconstructed plan has the previewed key. The semantic reference
interpreter records `SemanticOracle` acceptance and explicitly has no
independent optimized verifier.

Workspace layout uses `eqiora.studio.workspace/v1`, not the model or run wire.
The decoder checks version, model digest, node count, finite bounded
coordinates, and JSON byte count. Reconciliation prunes missing canonical IDs
and assigns deterministic positions to new IDs. Saves are coalesced while a
view node moves.

The application shell independently uses one closed typed registry for the
implemented Relations, scalar-elliptic, and bounded-CAD workflows. Accepted
document projection and exact CAD Model identity are its only applicability
inputs. Navigation, header actions, command-palette entries, focus targets,
typed disabled reasons, existing protocol-owned projection budgets, and
semantic alternatives are projections of that registry. Registry message
keys and fallback English are presentation-only.

## Falsifying cases

- malformed or resource-excessive run input never reaches native execution;
- an obsolete asynchronous plan preview cannot replace a newer request;
- editing an admitted input invalidates the plan without deleting completed
  evidence;
- a forged or stale plan key is rejected before execution;
- reference evidence cannot claim an independent verifier in the runtime
  schema;
- another model digest, unknown workspace version, malformed JSON, excessive
  coordinate, or unavailable storage fails without changing canonical state;
- 10,000 samples with narrow extrema remain ordered and retain both extrema
  and endpoints within the fixed chart budget;
- the semantic-table projection stays within its independent fixed row budget;
- compile, accepted run, reflow, and focus navigation remain available through
  the command palette without dragging;
- workflow and command identities are unique; a CAD projection for another
  Model cannot enable Geometry, and an inapplicable Geometry request falls
  back without changing the accepted document;
- header, navigation, and palette actions resolve the same availability and
  reason, and a hidden-workflow focus command cannot execute;
- every workflow declares a bounded projection and a keyboard-accessible
  semantic alternative, and every registry-owned message key resolves;
- the scalar control result remains summary-only and Field transfer begins
  only through an explicit action on the exact accepted Model/run/plan;
- a foreign descriptor, invalid chunk sequence or size, non-finite value, or
  final count/range drift fails before a complete Field reaches the renderer;
- the 2D Field table, Cartesian raster, and inspector retain one shared
  keyboard/pointer selection without exceeding the declared value budget;
- native modal focus is restored on Escape, disabled commands expose a reason,
  and serious/critical automated WCAG 2.2 findings fail the interaction test;
- primary actions remain visible at the supported minimum shell size.

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

Core plan/evidence contracts remain in the normal workspace gates:

```bash
cargo test --workspace --locked
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

## Nonclaims

This case is not evidence for general spatial Realization or device selection,
connection/topology transactions, mesh rendering, unstructured/nonuniform or
3D/vector/tensor Field rendering, GPU zero-copy, portable project files, an
independent verifier for the semantic oracle, or unbounded result transfer.
The bounded 2D scalar projection and explicit data plane have their own
registered case,
[`interfaces.studio-scalar-field-view`](../../verify/interfaces/studio-scalar-field-view/README.md).
The registry is not dynamic plugin discovery or whole-product localization.
Typed scalar value transactions and the controlled-run lifecycle have their
own separate verification cases.
