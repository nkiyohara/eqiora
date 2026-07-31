# Examples

Examples are small orientation paths for users. They favor readability and do
not establish support claims. Reproducible claim/non-claim contracts live under
[`verify/`](../verify/) and are indexed by `eqiora-verify`.

Run them from the repository root:

```bash
cargo run --locked -p eqiora --example quickstart
cargo run --locked -p eqiora --example poisson
python examples/python/exact_cylinder_geometry.py
python examples/python/exact_cylinder_mesh.py
python examples/python/exact_cylinder_stokes.py
python examples/python/exact_cylinder_stokes.py \
  --pressure-png exact-cylinder-pressure.png
```

| Example | Source | What it shows |
| --- | --- | --- |
| `quickstart` | [`decay.eqi`](decay.eqi) | Compile one scalar decay model and run it through the reference lifecycle. |
| `poisson` | [`packages/org.example.poisson`](../packages/org.example.poisson/) | Compile a 2D Poisson model, select a Realization explicitly, run it on the host CPU, and report the L2 error against the exact solution. |
| `exact-cylinder-geometry` | [`python/exact_cylinder_geometry.py`](python/exact_cylinder_geometry.py) | From an installed `eqiora` package, author the exact rectangle-with-one-circular-hole identity and inspect its fixed-role named selections. |
| `exact-cylinder-mesh` | [`python/exact_cylinder_mesh.py`](python/exact_cylinder_mesh.py) | From an installed `eqiora` package, explicitly realize the exact cylinder source as the bounded error-controlled chordal reference mesh and inspect Rust-derived selection counts. |
| `exact-cylinder-stokes` | [`python/exact_cylinder_stokes.py`](python/exact_cylinder_stokes.py) | From an installed `eqiora` package, load its shipped exact Model artifact, execute the accepted exact-cylinder steady-Stokes composition, inspect immutable pressure, solver, force, and flux evidence, and optionally save its accepted P1 pressure through `eqiora[matplotlib]` without a repository-local runtime input. |
| `steady-flow-past-cylinder` | [`steady-flow-past-cylinder.eqi`](steady-flow-past-cylinder.eqi), [exact geometry](steady-flow-past-cylinder.geometry.json), [current Model](steady-flow-past-cylinder.model.json) | In native Studio, replay one immutable exact rectangle-minus-circle Model, realize its error-controlled affine mesh, execute the accepted steady Stokes path, and inspect the pressure field with reaction and balance evidence. |

General examples keep the Model, the Realization, and the Run visibly separate;
their mesh, discretization, solver, and placement choices remain explicit. The
exact-cylinder Stokes example instead names one closed reference application
whose complete configuration is frozen behind that narrow operation. It is not
a default Realization for other fluid problems.

The [Poisson walkthrough](../docs/site/examples.md) explains what each stage
means and what the reported evidence does and does not establish.

The cylinder source, exact geometry, and canonical Model are one checked
example set rather than three interchangeable inputs. Open it with **Run
cylinder demo** in native Studio. Its 50-chord mesh is an error-controlled
realization of the retained exact circle; the example does not claim curved
finite elements, Navier--Stokes flow, a drag coefficient, vortex shedding,
mesh convergence, or a benchmark comparison.
