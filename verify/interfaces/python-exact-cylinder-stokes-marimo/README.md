# Exact-cylinder steady-Stokes Marimo composition

This case owns one application-level composition and no scientific meaning.
The canonical file is fixed at
[`examples/python/exact_cylinder_stokes_marimo.py`](../../../examples/python/exact_cylinder_stokes_marimo.py).
It authors the exact Geometry, resolves a typed Gmsh Mesh, compiles the
installed component-only `.eqi` source with that Geometry, resolves the root
Plan, executes it directly, obtains the common Result, and presents the
caller-owned pressure Figure in one Marimo flow.

## Independent positive path

The evidence first checks the exact canonical source as one public
composition. A narrow AST inventory requires exactly one direct
`eqiora.run(...)` call and no `eqiora.submit(...)` or `.result()` call. After
that positive source passes, adding a second direct `eqiora.run(stokes_plan)`
must fail only this inventory. This makes
no claim about aliases or dynamically constructed calls. The source also has
one `plot_scalar_field(...)` and one
`importlib.resources.files(eqiora)` occurrence. This is not a generic Python
import, file-access, or isolation policy. It freezes no digest, physical
observation, tolerance, or image baseline.

The candidate profile then installs one non-editable Linux x86-64 CPython 3.13
wheel with its existing exact `notebook` and `matplotlib` extras. It copies
only the application into a dedicated clean positive consumer and requires
its complete recursive inventory to be exactly that one regular file, with no
directory or symlink member. It launches exact Marimo 0.23.16 under
`python -I`, with the resolved script target exactly equal to the copied file
in that consumer. The browser waits for the application
readiness marker only after observing live rows named by the public carrier
types: Geometry, MeshPlan, Mesh, Model, Plan, and Result. It requires a
decoded nonempty pressure Figure. It does not compare scientific scalars or
pixels.

## Causal falsifiers

Only after that live browser positive succeeds, the same installed CPython
3.13 environment executes one exact precommitted wrong app under `python -I`.
That app substitutes
`from examples.python.exact_cylinder_stokes import solve` for the installed
composition. Its separate clean negative consumer has the exact recursive
inventory
`exact_cylinder_stokes_marimo_repository_helper_mutant.py` and nothing else.
The expected outcome is specifically
`ModuleNotFoundError: No module named 'examples'`; an unrelated denial or an
unexpectedly resolved helper rejects. The import fails before the mutant can
execute Eqiora, so the one ordinary positive remains the only execution.

This is one exact plausible wrong application, not a generic claim about
arbitrary Python imports, file reads, editable environments, or security
isolation. It uses simple clean staging rather than an OS sandbox, public
loader, application protocol, or new durable receipt.

## Authority and non-claims

The exact-Geometry dossier remains the only registered predecessor. Typed
Gmsh meshing, root Plan execution, common Result projection, and Matplotlib
rendering are ordinary product coverage here: this case fixes only the
checked-in application composition and its clean installed-candidate Marimo
reachability. It freezes no mesh count, digest, scientific scalar, tolerance,
or image baseline. It does not claim transient
flow, the cylinder-wake benchmark, drag/lift/Strouhal/Reynolds quantities,
trajectory or animation, a shared viewer, image or pixel identity, JupyterLab
or Studio parity, a general notebook workflow, performance, or production
scale.

After implementation and registration, run:

```console
cargo run --locked -p eqiora-verify -- run \
  --case interfaces.python-exact-cylinder-stokes-marimo
```
