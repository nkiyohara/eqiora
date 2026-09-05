# Numeric domains and scalar operations

This target-language catalog defines the bounded scalar operations. Each implementation slice must expose
only the rows it actually supports. The table does not establish current execution coverage.

## Exact integers

`integer` is a signed exact 64-bit value, ranging from -9223372036854775808 through
9223372036854775807. Checked addition, subtraction, multiplication, and negation reject overflow;
they do not wrap or saturate. Decimal integer literal parsing never passes through binary64.
A leading sign is applied before checking the signed literal's range, so the minimum value
is representable without admitting its positive magnitude as an integer value.

```eqiora
parameter particles: integer = 9007199254740993;
let adjacent: integer = particles + 1;
```

`adjacent` is exactly 9007199254740994. Integer arithmetic receives integer operands; there is
no implicit integer/real promotion in an expression. A literal can use a uniquely required
integer context. Without that context ordinary numeric expressions retain the real-scalar
default, so `1/2` in a real value expression is one half, not integer zero.

`quotient(a, b)` truncates toward zero; `remainder(a, b)` satisfies
`a = b*quotient(a,b) + remainder(a,b)` and has the dividend's sign when nonzero.
For example, `quotient(-7, 3) = -2` and `remainder(-7, 3) = -1`.
A zero divisor rejects. Minimum-integer divided by -1 rejects overflow. Ordinary `/` does
not silently select this integer quotient operation; explicitly convert to real for a real ratio.

`to_real(n)` explicitly converts an integer to a dimensionless real scalar. The canonical
binary64 literal/value boundary rounds once to nearest, ties to even, and may lose integer
precision. In particular converting 9007199254740993 produces 9007199254740992; preserving
an exact count requires retaining the integer type. `to_integer(x)` accepts only a finite,
dimensionless real scalar with integral value inside the exact integer range. It does not
round a fractional value or recover precision already lost by a previous conversion.

`index<set>` is a bounded index tied to an exact declared finite index set; it is not an
interchangeable integer from another set. Its checked constructor is `index(set, integer)`.
Physical counts retain their separate species/count contract and nonnegative range; an
integer representation alone supplies neither species identity nor a count-to-amount law.
Runtime counts cannot determine model extent. Integers, indexes, Booleans, and enums have
no ordinary continuous derivative, even if an integer value happens to remain constant.

## Real elementary functions

Except where stated otherwise, the following functions require real dimensionless scalar
arguments and return a real dimensionless scalar. First and second derivatives are admitted
only on the open smooth domain in the table. Value admission at an endpoint does not grant
a derivative there. Typed vectorized evaluation follows the same component domains, without
changing branch decisions or introducing broadcasting.

| Operation | Value domain and result rule | Derivative boundary |
|---|---|---|
| `+`, `-` | Compatible dimensions; preserve dimension | Smooth on finite admitted values |
| `*` | Multiply dimensions | Smooth on finite admitted values |
| `/` | Nonzero denominator; divide dimensions | Singular at zero denominator |
| `math.sqrt(x)` | Real `x >= 0`; halve exact dimension exponents | Reject ordinary derivatives at zero |
| `math.exp(x)`, `math.expm1(x)` | All finite real inputs; reject nonfinite output | Smooth where numerical evaluation is admitted |
| `math.log(x)` | `x > 0` | Zero and negative arguments reject |
| `math.log1p(x)` | `x > -1` | `x <= -1` rejects |
| `math.sin(x)`, `math.cos(x)` | Dimensionless radians | Smooth |
| `math.tan(x)` | Exclude odd multiples of `pi/2` | Poles reject |
| `math.asin(x)`, `math.acos(x)` | `-1 <= x <= 1` | Derivatives require `-1 < x < 1` |
| `math.atan(x)` | All finite real inputs | Smooth |
| `math.atan2(y, x)` | Equal dimensions, not both zero; result in `(-pi, pi]` | Reject at the branch cut and origin |
| `math.sinh(x)`, `math.cosh(x)`, `math.tanh(x)` | All finite real inputs; reject nonfinite output | Smooth where numerical evaluation is admitted |
| `math.asinh(x)` | All finite real inputs | Smooth |
| `math.acosh(x)` | `x >= 1` | Derivatives require `x > 1` |
| `math.atanh(x)` | `-1 < x < 1` | Endpoints and outside reject |

`math.pi` is the dimensionless circle constant. Trigonometric arguments do not infer degrees
from magnitude or a variable name. At a negative `x` with zero `y`, `atan2(y,x)` selects
`pi`; signed numerical zero does not select a second mathematical branch. This value
convention does not make the derivative continuous across the cut.

`expm1(x)` means `exp(x)-1` mathematically and `log1p(x)` means `log(1+x)`, but their numerical
implementations use the appropriate cancellation-resistant kernels. They are not required
to reproduce the rounding error of a literal subtraction or addition in another expression.
Invalid real arguments never silently promote to complex. Complex functions retain their
separately admitted branch and real-linear derivative rules.

## Value powers

Dimension exponents remain exact rational arithmetic. Value powers additionally check numerical
domain. A dimensioned base requires a statically exact reduced rational exponent `p/q`; a
runtime exponent requires a dimensionless base. Rational syntax in a dimensional power must
retain the fraction before numerical rounding, so a literal `1/3` cannot become an approximate
dimension exponent.

An integer power admits a negative real base. Zero with a positive exponent is zero; zero
with a negative exponent rejects; `0^0` rejects. For a rational exponent with denominator
greater than one, the initial real profile requires a nonnegative base. A negative real base
rejects even for an odd denominator: no alternate real-root branch is inferred. To request
a complex principal power, explicitly construct a complex base and use its admitted profile.

A noninteger runtime real exponent requires a positive base. Derivatives divide dimensions
according to the exact exponent and reject singular or unadmitted endpoint behavior; a zero
result alone is not proof that the derivative exists. Polynomial integer powers remain smooth
at zero where their actual derivative formula is defined.

## Memoryless nonsmooth operations

These operations use the same lazy branch owner as `if ... then ... else ...`. Their real
arguments must have compatible dimensions and shapes. No numerical tolerance changes ties.

| Operation | Exact value rule | Ordinary derivative rule |
|---|---|---|
| `math.abs(x)` | `x` for `x >= 0`, otherwise `-x`; preserve dimension | Reject at zero |
| `math.min(a,b)` | Lower value; choose first operand at equality | Reject at a tie unless the admitted expression proof establishes smoothness |
| `math.max(a,b)` | Higher value; choose first operand at equality | Same tie rule |
| `math.clamp(x, lower, upper)` | Require `lower <= upper`; return the bounded value | Reject at switching boundaries; no implicit smoothing |
| `math.sign(x)` | Dimensionless -1, 0, or 1 according to the exact sign | Reject at zero |
| `math.step(x)` | Dimensionless zero for `x < 0`, one for `x >= 0` | Reject at zero |

Complex magnitude and squared magnitude use their explicit complex operations; they do not
inherit real ordering. A smoothness assertion supplied by a caller is not a proof that a tie
can be differentiated. A method needing generalized derivatives or crossing events must
request those separately.

## Error and verification boundaries

All operations retain source-local diagnostics for wrong dimensions/domains, overflow,
nonfinite output, unsupported derivatives, and invalid conversions. Never clip a pole,
replace a singular inverse, add a positivity floor, or silently switch branches to make a
numerical evaluation succeed. Mathematical domain and numerical representability are checked
separately; execution precision is not a new mathematical scalar domain.

Useful independent reference points include `sqrt(4)=2`, `sqrt'(4)=1/4`, `exp(0)=1`,
`expm1(0)=0`, `log(1)=0`, `log1p'(0)=1`, `sin(0)=0`, `cos(0)=1`, and
`atan2(1,1)=pi/4`. Boundary tests must also reach rejected domains and inactive conditional
branches. Small-argument accuracy tests for `expm1` and `log1p` require a separately justified
error bound, not comparison against the cancellation-prone expression they are meant to replace.
