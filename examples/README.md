# Examples

Examples are small orientation paths for users. They favor readability and do
not establish support claims. Reproducible claim/non-claim contracts live under
[`verify/`](../verify/) and are indexed by `eqiora-verify`.

Run them from the repository root:

```bash
cargo run --locked -p eqiora --example quickstart
cargo run --locked -p eqiora --example poisson
```

| Example | Source | What it shows |
| --- | --- | --- |
| `quickstart` | [`decay.eqi`](decay.eqi) | Compile one scalar decay model and run it through the reference lifecycle. |
| `poisson` | [`packages/org.example.poisson`](../packages/org.example.poisson/) | Compile a 2D Poisson model, select a Realization explicitly, run it on the host CPU, and report the L2 error against the exact solution. |

Each example keeps the Model, the Realization, and the Run visibly separate.
Selecting a mesh, a discretization, a solver, or a placement is always something
the caller writes down; there is no default Realization to fall back on.

The [Poisson walkthrough](../docs/site/examples.md) explains what each stage
means and what the reported evidence does and does not establish.
