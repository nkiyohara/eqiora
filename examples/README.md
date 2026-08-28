# Examples

Examples are small orientation paths for users. They favor readability and do
not establish support claims. Reproducible claim/non-claim contracts live under
[`verify/`](../verify/) and are indexed by `eqiora-verify`.

Run them from the repository root:

```bash
cargo run --locked -p eqiora --example quickstart
python examples/python/exact_cylinder_geometry.py
python examples/python/exact_cylinder_mesh.py
python examples/python/exact_cylinder_stokes.py
python examples/python/exact_cylinder_stokes.py \
  --pressure-png exact-cylinder-pressure.png
marimo run examples/python/exact_cylinder_stokes_marimo.py
jupyter lab examples/python/exact_cylinder_stokes_jupyter.ipynb
python examples/python/fixed_reference_fsi.py
```

| Example | Source | What it shows |
| --- | --- | --- |
| `quickstart` | [`decay.eqi`](decay.eqi) | Compile one scalar decay model and run it through the common root Plan lifecycle. |
| `poisson` | [`packages/org.example.poisson`](../packages/org.example.poisson/) | Compile a 2D Poisson model and exercise its verification-only native reference solve. |
| `exact-cylinder-geometry` | [`python/exact_cylinder_geometry.py`](python/exact_cylinder_geometry.py) | From an installed `eqiora` package, author the exact rectangle-with-one-circular-hole identity and inspect its fixed-role named selections. |
| `exact-cylinder-mesh` | [`python/exact_cylinder_mesh.py`](python/exact_cylinder_mesh.py) | From an installed `eqiora` package, explicitly realize the exact cylinder source as the bounded error-controlled chordal reference mesh and inspect Rust-derived selection counts. |
| `exact-cylinder-stokes` | [`python/exact_cylinder_stokes.py`](python/exact_cylinder_stokes.py) | From an installed `eqiora` package, define the sole concrete Geometry in Python, compile it with the shipped equations-only `.eqi` Component, resolve the common MINI/P1 and linear-solve policies, inspect immutable pressure, solver, force, and flux evidence, and optionally save the pressure through `eqiora[gmsh,matplotlib]`. |
| `exact-cylinder-stokes-marimo` | [`python/exact_cylinder_stokes_marimo.py`](python/exact_cylinder_stokes_marimo.py) | In Marimo, compose the same Python-authored Geometry, installed `.eqi` Component, Mesh, common root Plan, and direct `run` Result, then inspect their live identities and caller-owned pressure Figure. |
| `exact-cylinder-stokes-jupyter` | [`python/exact_cylinder_stokes_jupyter.ipynb`](python/exact_cylinder_stokes_jupyter.ipynb) | In Jupyter, compose the same public Geometry, installed `.eqi` Component, Mesh, common root Plan, direct `run` Result, identity summary, and caller-owned pressure Figure without a rich-Mesh widget. |
| `fixed-reference-fsi` | [`python/fixed_reference_fsi.py`](python/fixed_reference_fsi.py) | Author the adjacent Geometry in Python, compile the equations-only FSI Component, scope MINI/P1 and P1 to exact Model Domains, initialize four exact Fields, and run the common root Plan/State/Run lifecycle. |
| `steady-flow-past-cylinder` | [`steady-flow-past-cylinder.eqi`](steady-flow-past-cylinder.eqi), [exact geometry](steady-flow-past-cylinder.geometry.json) | Python supplies the sole concrete Geometry to the equations-only `.eqi`, then uses the common root Plan lifecycle. The retained JSON artifacts serve historical verification only. |

General examples keep the Model, typed numerical Plan, and Run visibly separate;
their mesh, discretization, solver, and placement choices remain explicit. The
exact-cylinder Stokes example instead names one closed reference application
whose complete configuration is frozen behind that narrow operation. It is not
a default Realization for other fluid problems.

The [Poisson walkthrough](../docs/site/examples.md) explains what each stage
means and what the reported evidence does and does not establish.

The cylinder source and Python-authored exact geometry form one checked
workflow. Its 50-chord mesh is an error-controlled realization of the exact
circle; the example does not claim curved
finite elements, Navier--Stokes flow, a drag coefficient, vortex shedding,
mesh convergence, or a benchmark comparison.
