# Studio controlled-run lifecycle verification

This case verifies the first controlled reference-run slice shared by the
semantic interpreter, public application service, and Studio. It proves a
cooperative accepted-boundary contract; it does not turn presentation progress
into model semantics or admit a partial trajectory as a result.

## Contract under test

```text
immutable document + accepted ReferenceRunPlan
                         ↓
controlled semantic interpreter
  initial accepted state / accepted time, tick, or event boundary
                         ↓ bounded observer
Continue ────────────────┴────────────────────────── Cancel
   ↓ complete interval                                ↓
complete owned result + evidence       accepted cancellation boundary only
                         ↓
          typed terminal ReferenceRunOutcome
```

Observation never occurs inside expression evaluation, Newton iteration,
event localization, or an atomic activation commit. The initial observation
follows consistency solving and any zero-time periodic activation. Later
observations follow fully accepted nonterminal steps. Cancellation therefore
identifies a coherent model time and accepted-step count but produces no
partial `Trajectory`, result series, or successful-run evidence.

Studio bridge v5 assigns a UUID to each run and owns at most one active run per
native session. An exact-identity cancellation request sets a Rust atomic flag.
The blocking execution worker observes that flag only through the shared
accepted-boundary callback. A Studio-local adapter emits the first progress
value, coalesces later values to at most one every 100 ms, and forces an
emission when it observes cancellation. Channel progress is advisory and may
be dropped; the completed, cancelled, or diagnostic command response is the
only terminal authority.

The frontend reducer independently checks protocol, request ID, run ID, end
time, and monotone model-time/step progress. Run lifecycle and retained result
data are separate. Starting or cancelling a successor does not erase the last
completed result, and a cancelled successor cannot replace it with partial
samples. The run panel presents the accepted plan, model-time progress,
accepted-step count, cancellation state, and the prior immutable evidence with
explicit status text rather than color alone.

## Falsifying cases

- the public controlled-run observer sees accepted step counts
  `[0, 1, 2, 3]`, cancels at step three, and reports model time `0.3` for the
  reference decay case;
- an observer that always continues produces the same series and plan evidence
  as the uninterrupted public API;
- the semantic interpreter never calls an observer from a Newton iteration,
  event-localization trial, expression evaluation, or partial activation
  commit;
- a second native run cannot replace an active run, and another run ID cannot
  set its cancellation flag;
- malformed UUIDs, run/outcome identity drift, plan drift, non-finite or
  out-of-bounds progress, and cancellation/end-time inconsistency fail runtime
  schema validation;
- obsolete request IDs, misrouted run IDs, regressing model time, and
  regressing accepted-step counts cannot update frontend state;
- cancellation returns a typed accepted boundary and no partial result or
  successful-run evidence;
- a cancelled successor leaves the preceding completed 41-sample trajectory
  and its immutable evidence visible;
- the progressbar and polite status copy are exposed to assistive technology,
  and cancellation is available through both the run panel and searchable
  command path;
- the accepted plan compacts during execution without hiding adapter,
  placement, integrator, progress, or the safe-point action;
- the safe-point button remains wholly inside the run panel at the 1440×900
  reference layout; and
- the cancelled-successor state with retained completed evidence produces no
  serious or critical automated WCAG 2.2 findings, while the transient running
  state independently proves progressbar semantics and complete safe-point
  button bounds; definition-list structure and 24-pixel target size remain in
  the automated gate.

## Commands

```bash
cargo test -p eqiora-sem -p eqiora-api --locked
cargo clippy -p eqiora-sem -p eqiora-api --all-targets --locked -- -D warnings

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

The repository-wide gates remain authoritative for dependency and feature
composition:

```bash
cargo test --workspace --locked
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

## Nonclaims

This case is not evidence for hard real-time cancellation latency,
solver-inner-loop preemption, pause/resume, checkpoint/restart from a cancelled
boundary, concurrent or remote runs, cancellation of production time/spatial
backends, Python async/cancellation, operating-system process termination, or
persistent run-queue recovery. Those consumers may reuse the accepted-boundary
shape only after defining and falsifying their own safe points and terminal
evidence.
