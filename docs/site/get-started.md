# Get started

The public alpha supports ordinary-GIL CPython 3.11–3.14 on
manylinux x86-64.

## Install

Create a clean environment and install the exact prerelease:

```bash
python -m venv .venv
. .venv/bin/activate
python -m pip install eqiora==0.1.0a1
```

## Build and run a model

This complete example creates immutable `Field`, `Parameter`, and `Relation`
declarations, closes them into one native `Model`, and runs the shared native
execution lifecycle:

```python
import eqiora

state = eqiora.Field("state", initial=1.0)
rate = eqiora.Parameter(
    "rate",
    value=1.0,
    dimension=eqiora.Dimension(time=-1),
)
decay = eqiora.Relation(
    "decay",
    residual=eqiora.derivative(state) + rate * state,
)
model = eqiora.Model.define("decay", state, rate, decay)

result = eqiora.run(model, end_time=1.0, max_step=0.01)
time = result["state"].time.numpy(copy=False)
values = result["state"].values.numpy(copy=False)

print(eqiora.__version__)
print(model.digest)
print(time[-1], values[-1])
```

The NumPy arrays returned by `copy=False` are lifetime-safe and read-only. Use
`copy=True` when you need an independent writable array; Eqiora does not turn
an impossible zero-copy request into a hidden copy.

## Choose your next path

- [Python guide](python/index.md) covers spatial modeling, structured diagnostics,
  asynchronous runs, cancellation, and framework adapters.
- [Concepts](concepts.md) explains relations, realizations, and evidence.
- [Examples](examples.md) separates readable orientation from capability
  proof.
- [Capabilities](capabilities.md) states the exact supported boundary and
  nonclaims.

Rust users can run the small public-facade orientation examples from a source
checkout:

```bash
git clone https://github.com/nkiyohara/eqiora.git
cd eqiora
cargo run --locked -p eqiora --example quickstart
cargo run --locked -p eqiora --example poisson
```

`quickstart` compiles and runs the decay model above. `poisson` is the spatial
counterpart: it compiles a 2D Poisson model, selects a finite-element
Realization explicitly, runs it on the host CPU, and reports the L2 error
against the exact solution. The [Examples](examples.md) page walks through it
stage by stage.

Before relying on any numerical method or backend, trace its row in the
[capability matrix](capabilities.md) to a registered evidence case.
