# Input Model

There is no checked Model artifact or duplicate Geometry. The application
reads `eqiora/examples/steady-flow-past-cylinder.eqi` through
`importlib.resources.files(eqiora)` and compiles it with the caller-authored
Geometry. The resulting Model, Plan, and Result identities are observed only
relationally; this dossier freezes no digest or Gmsh output.
