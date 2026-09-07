# Independently derived equality observations

`models/natural.eqi` retains the ordered pairs `(a,b)`, `(a-(b-c),d)`,
and `(-a,-b)`. `models/explicit-residual.eqi` retains each complete residual
on the left and a literal zero on the right. Each file formats to its own
bytes, not to the other file. Both checked residual vectors are:

```text
Sub(a,b)
Sub(Sub(a,Sub(b,c)),d)
Sub(Neg(a),Neg(b))
```

The sixteen fixed rows distinguish authored sides from checked roots:

| Authored statement | Checked root | Canonical statement |
| --- | --- | --- |
| `x = 0` | `x` | `x = 0` |
| `x = (0)` | `x` | `x = 0` |
| `x = ((0))` | `x` | `x = 0` |
| `x = -0` | `x` | `x = -0` |
| `x = (-0)` | `x` | `x = -0` |
| `x = -(-0)` | `x` | `x = --0` |
| `x = 0e-999` | `x` | `x = 0` |
| `0 = x` | `Sub(0,x)` | unchanged |
| `x - 0 = 0` | `Sub(x,0)` | unchanged |
| `x = 0*y` | `Sub(x,Mul(0,y))` | `x = 0 * y` |
| `x = y-y` | `Sub(x,Sub(y,y))` | `x = y - y` |
| `x = zero` | `Sub(x,Parameter(zero))` | unchanged |
| `x-y = z` | `Sub(Sub(x,y),z)` | `x - y = z` |
| `x = y-z` | `Sub(x,Sub(y,z))` | `x = y - z` |
| `-x = -y` | `Sub(Neg(x),Neg(y))` | unchanged |
| `x-(y-z) = x` | `Sub(Sub(x,Sub(y,z)),x)` | `x - (y - z) = x` |

Source ranges are independently located in each fixed UTF-8/CRLF input:
equality begins at its lhs, ends after the complete rhs, and excludes the
semicolon. Parentheses remain within the authored operand range. No expected
offset is read from parser output.

Bare zero works for a dimensionful lhs. Explicit matching-unit zero works;
wrong or unknown units reject before normalization. `0*missing` still resolves
its operand; `0*1[s]` still has time dimension. Decimal nonzero underflow
(`1e-324`, including sign/grouping) rejects at numeric admission. Unit conversion
nonzero underflow (`5e-324[mm]`) rejects at normalization. Exact decimal zero
with a large exponent remains zero; the least positive representable subnormal
(`5e-324`, binary64 bits `1`) remains a nonzero operand.

Eight actual source mutations independently change multiplication/addition,
dropped rhs, swapped operands, sign, inner operand order, reassociation, or
root order. Every mutated model is compiled and compared through the public
ordered structural fingerprint, rather than mutating an observation clone.

The unchanged mathematical permutation oracle is `x+y=3, x-y=1`: adding and
subtracting gives `(x,y)=(2,1)`. At `(4,-1)` the ordered residual vector is
`(0,4)`, or `(4,0)` after swapping equations. This justifies solution invariance,
not identical ordered fingerprints. The focused simultaneity test owns execution.

The exact package is `org.eqiora.oracle.NaturalEquation@1.0.0`, entry
`models.natural`, source `src/models/natural.eqi`, no dependencies. Native fields
are `a=4,b=3,c=2,d=1` followed by the same three explicit mathematical residuals.
Only their checked structural meaning is compared; fresh lineage is not equated.
