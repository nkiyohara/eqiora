# Python exact-cylinder steady-Stokes root Plan

This case verifies the installed Python model-first path:

```python
geometry = eqiora.GeometryGraph(2)
fluid_box = geometry.rectangle("fluid_box", origin=(0.0, 0.0), size=(2.2, 0.41))
cylinder = geometry.circle("cylinder", center=(0.2, 0.2), radius=0.05)
domain = geometry.subtract("fluid", fluid_box, cylinder)
geometry = geometry.build(domain)

model = eqiora.compile(
    path=component_source,
    geometry=geometry,
    parameters={...},
)
mesh_provider = eqiora.meshing.ReferenceMesher(...)
mesh_plan = eqiora.meshing.resolve(geometry, mesh_provider)
mesh = eqiora.meshing.generate(geometry, plan=mesh_plan)
plan = eqiora.resolve(
    model,
    mesh=mesh,
    spatial=eqiora.fem.MiniP1(),
    solver=eqiora.solve.Linear(...),
    scaling=None,
)
result = eqiora.run(plan)
pressure = result.output(plan.field("pressure"))
observation = eqiora.fluid.steady_stokes_evidence(result)
```

The `.eqi` file owns equations, fields, parameters, and abstract supports. Python
is the sole concrete geometry authority. The root resolver derives physics from
the Model and accepts both a fresh compile and its replay through the same
semantics. The Plan owns the exact field selector used to read the common
`FieldOutput`; the native execution output owns the exact Plan pairing used to
construct the steady-Stokes observation.

Scientific values, tolerances, and falsifiers remain owned by
[`fluid.exact-circular-hole-stokes-2d-gmsh`](../../fluid/exact-circular-hole-stokes-2d-gmsh/README.md).
This interface case checks installed-package composition, common output
ownership, the finite flux and momentum relationships, fresh/replay resolver
equivalence, foreign-field and cross-physics rejection, and absence of the
displaced `SteadyStokes` intent, specialized Plan, and `fluid.resolve` path.

It does not claim a general fluid formulation, arbitrary meshes, velocity or
bubble projection, drag or lift, visualization, convergence, transient flow,
FSI, or cross-platform mesh-byte identity. The executable boundary is recorded
in [`case.toml`](case.toml).
