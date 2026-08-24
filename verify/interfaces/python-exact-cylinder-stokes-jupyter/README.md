# Exact-cylinder steady-Stokes Jupyter composition

This case owns one Jupyter presentation of the already accepted exact-cylinder
steady-Stokes application and no scientific meaning. The canonical notebook is
[`examples/python/exact_cylinder_stokes_jupyter.ipynb`](../../../examples/python/exact_cylinder_stokes_jupyter.ipynb).
It uses the same installed package resource and critical public-call inventory
as the accepted Marimo application: exact Geometry, explicit Mesh request and
plan, accepted Mesh, current Model replay, typed steady-Stokes intent and plan,
one submitted Run, common Result, typed evidence, and caller-owned pressure
Figure.

## Positive path and parity

The source check first parses the notebook as nbformat 4, requires the exact
ordered cell identities, and rejects stored outputs or execution counts. Each
critical Eqiora operation occurs exactly once in both the notebook code and
the accepted Marimo source. Both surfaces contain exactly one direct
`eqiora.submit(...)`, one `.result()`, and no direct `eqiora.run(...)`; this is
an exact check for these two sources, not a general alias or dynamic-call
analysis.

The candidate profile copies only the notebook into a clean consumer and
launches exact JupyterLab 4.6.2 through the installed CPython 3.13 environment.
The browser runs every cell and waits for readiness only after observing the
public carrier type names, runtime-equal Run and Result identities, the typed
pressure/force/flux unit labels, exactly one decoded nonempty pressure PNG, no
stderr output, and loopback-only traffic. It compares no scientific scalar or
pixel.

The existing Marimo ordinary positive and clean repository-helper mutant run
before this Jupyter positive in the same frozen candidate profile. The mutant
therefore continues to prove that neither presentation silently depends on the
repository example helper; this case adds no duplicate falsifier or second
execution authority.

## Authority and non-claims

The predecessor Geometry, Mesh, Result, steady-Stokes, and plotting cases
remain sole authorities for numerical and physical meaning. This case claims
only one checked-in Jupyter composition and its parity with the checked-in
Marimo composition. It does not claim transient flow, the cylinder-wake
benchmark, drag, lift, Strouhal or Reynolds quantities, trajectories,
animation, arbitrary notebooks, saved output replay, pixels, performance,
Studio parity, or production scale.

After implementation and registration, run:

```console
cargo run --locked -p eqiora-verify -- run \
  --case interfaces.python-exact-cylinder-stokes-jupyter
```
