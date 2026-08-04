# Model fixture

The public Rust evidence embeds the frozen one-dimensional Model source next
to its construction code so the Model and foreign-Model mutant remain visible
at the exact execution call site. It is not duplicated as an `.eqi` fixture in
this directory.

The bounded Model has one interval `[0, 1]`, a dimensionless scalar potential,
Parameters `source_scale`, `diffusion`, and `boundary_offset`, the relation
`-div(diffusion * grad(potential)) - source_scale = 0`, and equal essential
trace conditions at both endpoints.
