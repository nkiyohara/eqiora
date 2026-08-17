# Exact-cylinder steady-Stokes Marimo composition

This case owns one application-level composition and no scientific meaning.
The canonical file is fixed at
[`examples/python/exact_cylinder_stokes_marimo.py`](../../../examples/python/exact_cylinder_stokes_marimo.py).
It authors the already accepted exact Geometry, resolves the accepted bounded
Mesh, reads the existing Model from the installed `eqiora` package, resolves
the typed steady-Stokes Plan, submits exactly one Run, obtains the common
Result, and presents the existing typed evidence and caller-owned pressure
Figure in one Marimo flow.

## Independent positive path

The evidence first admits the checked-in source as one public composition. It
requires exactly one `eqiora.submit(...)`, one `.result()`, one
`steady_stokes_evidence(...)`, and one `plot_scalar_field(...)` call. It
requires `importlib.resources.files(eqiora)` and admits only absolute imports
from `eqiora`, `eqiora.matplotlib`, `importlib.resources`, and `marimo`. It
freezes no digest, physical observation, tolerance, or image baseline.

The candidate profile then installs one non-editable Linux x86-64 CPython 3.13
wheel with its existing exact `notebook` and `matplotlib` extras. It copies
only the application into a clean consumer directory and launches exact
Marimo 0.23.16 under `python -I`. The browser waits for the application
readiness marker only after observing live rows named by the public carrier
types: Geometry, MeshPlan, Mesh, Model, SteadyStokesPlan, Run, Result, and
SteadyStokesEvidence. It compares only the Run and Result identity
relationally, checks quantity unit labels, and requires a decoded nonempty
pressure Figure. It does not compare scientific scalars or pixels.

## Causal falsifiers

Only after the ordinary source is admitted, the source oracle applies three
same-shaped mutations. A repository-sentinel read and an editable-source
`sys.path` insertion are rejected before Marimo or a Run can exist. A second
`submit` is rejected by the one-Run predicate. The real candidate launch adds
the complementary causal boundary: its working directory contains the exact
copied application, not the extracted repository tree or its sentinel, and
`-I` prevents an ambient editable import from satisfying `import eqiora`.

This uses simple clean staging rather than introducing an OS sandbox, public
loader, application protocol, or new durable receipt. The candidate source
and installed wheel remain protected by the existing distribution-candidate
authority.

## Authority and non-claims

The five predecessor cases remain sole authorities for exact Geometry, Mesh,
physics, solver behavior, lineage, pressure, force, flux, and plotting
meaning. This case claims only the checked-in application composition and its
clean installed-candidate Marimo reachability. It does not claim transient
flow, the cylinder-wake benchmark, drag/lift/Strouhal/Reynolds quantities,
trajectory or animation, a shared viewer, image or pixel identity, JupyterLab
or Studio parity, a general notebook workflow, performance, or production
scale.

After implementation and registration, run:

```console
cargo run --locked -p eqiora-verify -- run \
  --case interfaces.python-exact-cylinder-stokes-marimo
```
