# Conditional scalar values

`if condition then value else value` is a memoryless value expression. The condition
must be Boolean. Both branches pass static checks for complete value type, dimensions,
shape, nominal support and activation; evaluating a value demands only its selected branch.
A missing branch is an error. Comparisons use exact mathematical endpoints, without a
solver tolerance or an automatically generated event.

The concrete real scalar operator profile includes guarded square roots and piecewise laws:

```eqiora
operator guarded_root(input area: V^2): V =
  if area >= 0[V^2] then math.sqrt(area) else 0[V];

operator conductance(input voltage: V, input forward: S, input reverse: S): A =
  if voltage >= 0[V] then forward * voltage else reverse * voltage;
```

The guarded root returns 2 V at 4 V² and 0 V at -1 V². Its inactive square root is not
evaluated. With forward conductance 2 S and reverse conductance 0.5 S, voltages
`[-4, 0, 3] V` yield `[-2, 0, 6] A`. Bounded batches apply the same scalar evaluator to each
row with fresh memoization and aggregate work/output limits; they do not evaluate both
branches and select afterward.

Python `q.if_else(condition, then_value, else_value)` authors this expression. Use existing
`q.quantity(value, unit)` for typed thresholds and branch constants. Python builds both
branch expressions; portable numerical evaluation selects one. All three operands retain
Source and formal ownership, so an inactive branch cannot capture a foreign declaration.

`math.abs`, binary `math.min`/`math.max`, `math.clamp`, `math.sign` and `math.step` use the
same conditional owner and the exact endpoints in the [numeric catalog](numeric-catalog.md).
Binary extrema retain their first argument on a tie. Finite IndexSet reductions lower
through the same comparison and selection nodes, preserving ordinal order and exact
integer values; their source reduction syntax remains distinct. Clamp admits equal bounds but rejects an inverted interval before
evaluating its value. This domain requirement is retained in the mathematical expression.

The native `ScalarOperatorIr::linearize_typed` binds a derivative calculation to one finite
real scalar input point in `symbols()` order. It preserves live input dependencies, demands
only the active path and returns the existing point-bound linearization. Conductance slopes
are 0.5 S below zero and 2 S above zero. At the switching point the ordinary derivative
rejects. The guarded-root slopes are 0.25/V at 4 V² and 0/V at -1 V²; its switching point
also rejects. An active square root at zero cannot supply an ordinary derivative.

The bounded derivative profile conservatively rejects demanded comparison boundaries,
including clamp's equal-bound case and either endpoint. It neither differentiates Boolean
predicates nor accepts a user assertion of smoothness. Equal branch values alone do not
prove a differentiable join. Generalized derivatives and smooth-join proofs are separate
capabilities. Existing symbolic first/second `partial` graphs retain their polynomial
profile; this point-bound entry does not make a branch-specific derivative graph valid at
another point.

A piecewise value creates no history, hysteresis, reset or crossing localization. An explicit
event uses the existing event owner and its admitted Run profile. General nonsmooth solves,
complex ordering, analytic catalogs beyond this profile and automatic event insertion are
not established by these scalar value and derivative paths.
