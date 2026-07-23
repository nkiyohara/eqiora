# General implicit DAE reference verification

`time.general-implicit-dae` verifies the smallest canonical path that cannot
enter Eqiora's constant first-order projection. Its residual is

```text
(1 + z) (x_dot + x) = 0
z - x^2 = 0
```

with exact solution `x(t) = exp(-t)` and `z(t) = exp(-2t)`. Because the
derivative coefficient `1 + z` depends on state, `FirstOrderProgram` rejects
the Relation and `GeneralImplicitProgram` retains the full residual plus its
analytic state/derivative JVP.

The explicit variable partition is `(x: differential, z: algebraic)`.
Consistent initialization holds `x = 1` and `z_dot = 0` fixed while solving
for `x_dot = -1` and `z = 1`. This is the semi-explicit index-one
initialization mode documented for IDA, not an inference that every admitted
residual has index one.

## Frozen evidence

| Steps | Step | Terminal L2 error | Observed order | Constraint error |
|---:|---:|---:|---:|---:|
| 10 | 1.0000e-1 | 2.2116138367007383e-2 | — | 2.7755575615628914e-17 |
| 20 | 5.0000e-2 | 1.1234336075507990e-2 | 0.977184635941 | 0.0 |
| 40 | 2.5000e-2 | 5.6626276671572370e-3 | 0.988371289924 | 0.0 |
| 80 | 1.2500e-2 | 2.8428606953024140e-3 | 0.994128265510 | 0.0 |

The deterministic implicit-Euler oracle must show monotonically decreasing
terminal error, observed order above `0.9`, and terminal algebraic-constraint
error below `1e-12`. The machine-readable source is
[`../../verify/time/general-implicit-dae/expected/convergence.csv`](../../verify/time/general-implicit-dae/expected/convergence.csv).

The same evidence target also checks `x_dot^2 - 1 = 0`. The Relation is
classified as nonlinear in its derivative, and a provided consistent pair
with `x_dot = +1` advances along `x(t) = t`. The opposite branch is equally
valid; the test ensures branch choice remains explicit problem data rather
than a lowering heuristic.

The case round-trips three distinct provenance roles: the replayable general
lowering witness, supplied versus accepted initial-pair envelopes, and the run
manifest that links both to `ImplicitEuler`, backend identity, plan, and output
digests. Decoder dimension limits, partition drift, accepted-pair digest drift,
and explicit-RK admission are rejected.

## Claim boundary

This proves one state-dependent-coefficient, semi-explicit index-one DAE with
a dense reference Newton solve and one explicitly selected branch of a scalar
nonlinear-derivative residual. It does not establish automatic branch
selection, a production IDA adapter, adaptive BDF, arbitrary-index or
structural-index analysis, sparse nonlinear algebra, checkpoint/restart
lineage, DAE sensitivities, or hybrid DAE event semantics.
