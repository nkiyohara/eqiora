# Python Model-first common ODE lifecycle

This case compiles one scalar decay Model, resolves it through the public root
no-Mesh Plan with exact Field-bound Tsitouras 5(4) tolerances, constructs the
Model-owned initial State, and executes through the ordinary Run lifecycle.
The expected values are derived independently as `exp(-t)` at `t = 0.1 s` and
`0.2 s`; implementation output is not used as an oracle.

The same resolver accepts exact artifact replay and retains the actual Model
digest. Exact `FieldRef` selects the immutable Series, and synchronous and
awaited access return the same once-materialized Result occurrence.

This does not claim shaped state, BDF, backward Euler on the common ODE arm,
DAE, events, sensitivities, string/name result lookup, step-count schedules,
controller-history continuation across restart, mid-run accepted-boundary
cancellation, another backend, or another placement.

Run the registered evidence:

```bash
cargo test --locked -p eqiora-python --test python_run_lifecycle
cargo run --locked -p eqiora-verify -- run --case interfaces.python-common-ode-lifecycle
```
