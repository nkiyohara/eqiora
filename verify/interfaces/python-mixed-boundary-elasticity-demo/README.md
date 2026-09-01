# Installed Python mixed-boundary elasticity root Plan

This case verifies the installed Python model-first structural path:

```python
graph = eqiora.geometry.GeometryGraph()
rectangle = graph.rectangle(x_bounds=(0.0, 1.0), y_bounds=(0.0, 1.0))
geometry = graph.build(
    rectangle,
    named_topology={
        "body": rectangle.region,
        "x_lower": rectangle.boundaries[0],
        "x_upper": rectangle.boundaries[1],
        "y_lower": rectangle.boundaries[2],
        "y_upper": rectangle.boundaries[3],
    },
)

model = eqiora.compile(
    path=component_source,
    geometry=geometry,
    parameters={...},
)
mesh_provider = eqiora.meshing.CartesianMesher(cells=(16, 16))
mesh_plan = eqiora.meshing.resolve(geometry, mesh_provider)
mesh = eqiora.meshing.generate(mesh_plan)
plan = eqiora.resolve(
    model,
    mesh=mesh,
    spatial=eqiora.fem.Q1(),
    solve=eqiora.solve.Linear(...),
)
result = eqiora.run(plan)
displacement = plan.field
output = result.output(displacement)
observation = eqiora.solid.linear_elasticity_evidence(result)
figure = eqiora.matplotlib.plot_deformed_field(
    result,
    field=displacement,
    scale=1.0,
)
```

The `.eqi` file owns equations, fields, parameters, and abstract supports.
Python owns the sole concrete rectangle and mesh. Physics comes from the Model;
the root resolver receives typed spatial and solver policies. The common Result
is read only with the exact Plan-owned field selector, while the native run
output preserves the exact Plan pairing used to construct the structural
observation.

Scientific values and tolerances remain unchanged and are owned by
[`solid.mixed-boundary-elasticity-2d`](../../solid/mixed-boundary-elasticity-2d/README.md).
This case checks the installed composition, common displacement output,
reaction/body-force/bounds relationships, foreign-field and cross-physics
rejection, fail-closed policy selection, and the headless caller-owned plot.
It also checks physical absence of the displaced `LinearElasticity` intent,
specialized Plan and resolver, and `plot_displacement` shim.

It does not claim stress, strain or traction recovery, convergence, other mesh
families, 3D, nonlinear or dynamic structure, FSI, interactive viewing, exact
pixels, or scientific validation from rendering. The executable boundary is
recorded in [`case.toml`](case.toml).
