# Python semantic-reference Run lifecycle verification

This case verifies one bounded Python lifecycle over the existing semantic
reference interpreter. `run(...)` and `submit(...).result()` cross the same
native execution path. A submitted `Run` exposes a typed finite status history,
one coalesced progress slot, exact Model and reference-plan identity, explicit
cooperative cancellation, and one immutable completed Result.

The registered Rust/PyO3 target loads the actual public Python wrapper and
proves completed, cancelled, and execution-failed terminal branches; exact
cancellation evidence and `EQ0506`; repeated and terminal cancellation
behavior; zero-duration progress semantics; once-only Result materialization;
sync/await parity; ordinary-GIL release; asyncio Task-cancellation independence;
and distinct progress publications no faster than the throttling policy. The
ordinary all-target package gate separately exercises invalid-transition,
waiter, worker-panic, and abandoned-materialization internals. The
installed-wheel companion adds Result/array lifetime independence and clean
process exit after dropping a live Run.

Cancellation is a request, not a retroactive label. It is observed only at the
reference interpreter's accepted boundaries. If completion or failure wins
after the final boundary, `Cancelling` may validly terminate as `Completed` or
`Failed`; no completed result is fabricated as cancelled. `progress` is not a
terminal counter and may be `None` for a zero-duration run.

This case does not call `ReferenceRunPlan` a portable Realization. It does not
claim typed graph-shaped Realization/deployment provenance, a persisted Run
manifest, production/MPI/CUDA cancellation, timeout taxonomy, callbacks or
notification streams, pause/restart, free-threaded CPython, or subinterpreter
shutdown.

Run the registered Rust evidence:

```bash
cargo test --locked -p eqiora-python --test python_run_lifecycle
cargo run --locked -p eqiora-verify -- run --case interfaces.python-reference-run-lifecycle
```

The installed-package companion is
`bindings/python/tests/test_run_lifecycle.py` and runs through the ordinary
isolated-wheel verification gate.
