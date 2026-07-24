# Eqiora Studio

Eqiora Studio is an accessible desktop projection of canonical Eqiora models.
It is a client of the public Rust facade, not a second model implementation.
Source compilation, transaction-wire replay, atomic commit, artifact identity,
and reference execution remain in `eqiora-api`.

The implemented vertical slices provide:

- an Eqiora source editor with a keyboard compile command;
- a coordinated semantic outline, relation view, and inspector;
- workspace-only graph layout with keyboard and pointer repositioning, stored
  through a separate bounded `eqiora.studio.workspace/v1` envelope;
- structured diagnostics that return keyboard users to the exact source span;
- editable run configuration with local field errors and bounded request
  validation before bridge allocation;
- native accepted-plan preview with exact preview-to-run key replay;
- detached reference execution, typed producer/placement/numerical evidence,
  and a bounded accessible trajectory table/chart;
- accepted-step progress and exact-run cancellation through the shared
  controlled-execution API, with no partial-result admission;
- distinct completed/cancelled terminal outcomes that retain the preceding
  completed evidence while a successor is running or cancelled;
- a dedicated scalar-elliptic Realization surface with allocation-free FEM/FVM
  capability preview, exact content-addressed replay, and bounded serial/Rayon
  host placement;
- independently checked true-residual and continuous-balance evidence without
  putting bulk mesh/field arrays in the control response;
- an explicit bounded 2D scalar Field view whose separate data plane retains
  exact Model/run/Realization identity, transfers fixed little-endian `f64`
  chunks from a two-entry session cache, and synchronizes a keyboard-accessible
  value table, inspector, and Cartesian raster;
- coherent-SI scalar edits for `Field` and `Parameter` nodes through exact
  shared transaction preview, optimistic preconditions, and atomic commit;
- bounded immutable revision navigation with keyboard undo/redo and explicit
  source-basis versus canonical-child provenance;
- one closed typed registry for the current Relations, scalar-elliptic, and
  bounded-CAD workflows, including shared navigation, command availability,
  focus targets, projection budgets, and semantic alternatives;
- a searchable native-modal command palette for every primary operation and
  secondary example fixture, backed by the same resolved command state;
- a typed presentation-message catalog for registry-owned labels,
  descriptions, and disabled reasons; and
- a least-privilege Tauri shell with no filesystem, shell, or remote-content
  permission.

## Boundaries

```text
React components
      ↓ pure reducer
canonical document state ── separate ── workspace layout/selection
      ↓ Zod-validated bridge DTO v5
Tauri command adapter
      ↓ public `eqiora` facade only
shared application service → canonical model / owned run result
                                  ↓ explicit user action
                 field-view DTO v1 + bounded raw chunks
```

The Studio bridge remains `eqiora.studio.bridge/v5`; that version owns the
WebView DTO shape and is independent of the canonical Model wire. Compile/check
ordinary authoring selects the shared current Model wire v6, while the closed
control decoder continues to admit explicit v1--v6 requests and responses.
Extending the Model wire alone does not advance the bridge protocol.

The protocol modules under `src/` validate every value crossing the WebView
boundary at runtime. Rust applies the same byte, time-step, cache, and protocol
limits before canonical operations. Unknown future kernel nodes or semantic
edges fail closed as `ST0003`; transport, resource, and plan-replay faults use
the Studio-local `ST0001`--`ST0007` range and are never confused with stable
kernel diagnostic codes.

The native runtime, not React, resolves a `ReferenceRunPlan`. The frontend may
present its adapter, placement, methods, tolerances, and bounds only after a
runtime-validated bridge-v5 response. Run submission carries the returned exact
plan key; the native adapter reconstructs the plan and rejects key drift before
starting work. Completed results state that the reference interpreter is the
semantic oracle and do not invent an independent verifier.

Every submitted run has an exact UUID and Studio owns at most one active native
run per session. Progress is accepted only for that identity and crosses an
advisory channel coalesced at accepted semantic boundaries. Cancellation sets
a Rust atomic token; the interpreter observes it outside Newton, event
localization, expression evaluation, and atomic activation commits. The typed
cancelled outcome retains the last accepted model time and step count and
contains no partial trajectory. Completion, cancellation, and diagnostics
return through the command response rather than being inferred from progress
delivery.

For a canonically lowered scalar elliptic model, the same bridge exposes a
separate Realization workflow rather than overloading model-time controls.
Editable method, cells-per-axis, and workers form an independent Realization
revision. The shared Rust application service bounds the implied mesh and
field shape before allocation, resolves a coherent FEM or FVM plan, and binds
it to model and `RealizationEnvelopeV1` identity. Submission replays the exact
plan key. The control result projects only bounded field summary,
assembly/solve producer topology, independent true-residual verification, and
continuous balance. Spatial work is atomic in this slice, so Studio displays
indeterminate status and offers no fabricated progress or cancellation
control.

A completed two-dimensional scalar result may then be opened by an explicit
**View field** action. `eqiora-api` has already fixed the Field and Domain
identities, dimension, coherent-SI bounds, association, logical shape, and
last-axis-fastest order in the accepted plan. The independent
`eqiora.studio.field-view/v1` protocol opens only that exact
Model/run/Realization identity and transfers at most 250,000 finite host
`f64` values in fixed 4,096-value little-endian chunks. The client publishes a
complete array to the renderer only after shape, chunk order/size, extrema,
and identity checks succeed. The native session retains at most two accepted
Fields. No coordinate/connectivity array, durable Field artifact, implicit
CPU/GPU copy, or partial rendering is implied.

The same boundary resolves a `ValueEditPlan` for one finite scalar-valued
`Field` or `Parameter`. The preview exposes before/after quantities and the
identity of the exact shared model-transaction envelope. Commit carries only
the accepted plan key and request identity; native code reconstructs and
replays the plan with revision and previous-value preconditions, then returns
an immutable child document plus typed lineage evidence. The source text is
retained as its basis rather than silently rewritten. UI undo/redo selects one
of at most 24 retained child documents; the native session cache is separately
bounded to 32 documents and prunes the same abandoned forward branch.

Kernel source spans use UTF-8 byte offsets. The DOM editor uses UTF-16 indices,
so `src/source-span.ts` owns the explicit, tested projection between them.
Diagnostics retain the source revision they describe and become navigable only
while that source still matches the editor. Completed results similarly remain
visible after a source or semantic run-input change, but are labelled as
preceding evidence rather than appearing current.

The browser development view is explicitly labelled **Browser preview**. It
exists for deterministic interaction and visual testing; it does not parse or
execute Eqiora semantics. Only the Tauri shell calls the native adapter.

`src/application.ts` is the closed application-shell registry. It does not
discover packages or plugins and does not inspect source text or component
trees to infer applicability. Relations comes from an accepted document,
scalar elliptic comes from the native-advertised document workflow, and CAD
requires an exact current-Model projection. If Geometry becomes inapplicable,
the shell falls back to Relations without mutating canonical or run state.
Header controls and the command palette use the same typed availability and
disabled-reason keys. Secondary example fixtures live in the palette so they
do not compete with the primary modeling task in the permanent header.

Each workflow declares a protocol-owned bounded projection and an accessible
semantic alternative. The scalar spatial control response remains
summary-only; its field workflow separately declares an explicit owned-host
copy, fixed chunk size, value limit, and semantic-table alternative. Registry
text is resolved through `src/messages.ts`, whose keys and fallback English
are presentation-only and never enter semantic identity or IPC.

## Develop and verify

Use Node.js 24 LTS and the repository Rust toolchain:

```bash
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

`npm run dev` opens the browser projection. `npm run tauri dev` launches the
canonical native path. Playwright verifies the 1440×900 design, the minimum
shell size, keyboard selection/compile/run and diagnostic-recovery paths,
visible focus, command search/focus restoration, bounded input and series
projection, stale-evidence provenance, accepted-step progress, cancellation,
retained prior evidence, spatial Realization preview/replay/evidence,
explicit 2D Field opening, table/raster keyboard selection, unobstructed
primary actions, and viewport containment. Exact-pinned
`@axe-core/playwright` scans primary, validation, diagnostic, and cancelled-
successor states with retained completed evidence for WCAG 2.2 A/AA serious or
critical violations without exclusions. Running-state checks independently
assert its progressbar and fully visible safe-point action. Automated scanning
complements rather than replaces keyboard and inclusive manual testing.

The accepted architecture and accessibility contract is
[RFC 0016](../rfcs/0016-studio-accessible-projection.md). The reproducible
accepted-plan, workspace, bounded-result, command, and accessibility cases are
listed in
[`docs/verification/studio-capability-evidence.md`](../docs/verification/studio-capability-evidence.md).
Typed transaction, conflict, immutable-lineage, and source-basis cases are in
[`docs/verification/studio-typed-value-edit.md`](../docs/verification/studio-typed-value-edit.md).
Controlled execution, exact run routing, coalesced progress, cancellation, and
retained-result cases are in
[`docs/verification/studio-run-lifecycle.md`](../docs/verification/studio-run-lifecycle.md).
Spatial FEM/FVM capability resolution, host placement, exact replay, and
independent evidence are specified in
[`docs/verification/studio-realization-capability.md`](../docs/verification/studio-realization-capability.md).
The application-owned Field projection and bounded explicit data-plane case is
registered in
[`verify/interfaces/studio-scalar-field-view`](../verify/interfaces/studio-scalar-field-view/README.md).
