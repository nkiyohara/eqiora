# Initial unit and dimension catalog

This catalog supplies the units used by the [language specimens](core.md). Type
dimensions, scaled input units, and output presentation remain separate. The compiler owns
one catalog shared by source and Python `eqiora.units`; no root-level unit aliases
or package-specific unit evaluator are introduced.

## Coherent symbols

The base symbols are `kg`, `m`, `s`, `A`, `K`, `mol`, and `cd`; `1` is dimensionless.
The following derived symbols have exact coherent definitions. Each is valid as a dimension
name and as an input unit with scale one.

| Symbol | Exact definition |
|---|---|
| `Hz` | `1/s` |
| `N` | `kg*m/s^2` |
| `Pa` | `N/m^2` |
| `J` | `N*m` |
| `W` | `J/s` |
| `C` | `A*s` |
| `V` | `W/A` |
| `Ohm` | `V/A` |
| `S` | `A/V` |
| `F` | `C/V` |
| `H` | `V*s/A` |
| `Wb` | `V*s` |
| `T` | `Wb/m^2` |

Unit symbols are case-sensitive: `S` is conductance, `s` is time, and `T` is magnetic flux
density. `Ohm` is the canonical ASCII spelling; the catalog does not add a second Unicode
ohm spelling. `g` is an input-only mass unit with exact scale 1/1000 kg. It is not a second
base dimension. No angle unit converts degrees implicitly; trigonometry consumes dimensionless
radian values and frequency roles require their explicit cyclic/angular conversion.

## Declaration initializers

A bare numeric initializer inherits the coherent unit of an explicitly dimension-typed
declaration. These declarations therefore denote the same value:

```eqiora
parameter density: kg / m ^ 3 = 1;
parameter explicit_density: kg / m ^ 3 = 1[kg / m ^ 3];
```

Use explicit input units for conversions: `parameter density: kg / m ^ 3 = 1[g / cm ^ 3];`
denotes 1000 kg/m³. An incompatible input unit is rejected. Contextual units apply only to
numeric declaration initializers, not arbitrary expressions or instance arguments.
The existing contextual-zero rule is unchanged.

## Prefixes and conversion

A numeric Parameter default uses its explicitly declared dimension's coherent unit.
For example, `parameter pressure: Pa = 2;` and
`parameter pressure: Pa = 2[Pa];` give the same value. An explicit input unit such
as `parameter pressure: Pa = 2[kPa];` gives 2000 Pa. Unknowns have no declaration
initializer: use `state pressure: Pa; initial { pressure = 2[kPa]; }` for a
mathematical initial condition. Its nonzero expression requires explicit units.

The initial prefix set is closed:

| Prefix | Scale |
|---|---|
| `n` | 1/1,000,000,000 |
| `u` | 1/1,000,000 |
| `m` | 1/1,000 |
| `c` | 1/100 |
| `k` | 1,000 |
| `M` | 1,000,000 |
| `G` | 1,000,000,000 |

One prefix may precede `m`, `s`, `A`, `K`, `mol`, `cd`, `g`, or any derived unit above.
No prefix may precede `kg`, a dimension alias, or an already prefixed unit. A bare coherent
symbol is recognized first; otherwise split one prefix from an admitted unit and reject if
the result is unknown or ambiguous. Prefix stacking, alternate case, and the micro sign are
not accepted aliases. Thus `ms`, `kOhm`, `GPa`, and `uF` are admitted; `mkg`, `kkOhm`, and
`KOhm` are rejected.

Prefixed symbols are input units, not dimension names. `parameter resistance: Ohm = 1 [kOhm];`
is valid source; a type `kOhm` cannot encode a scale into its dimension. A quantity's
product/division/power composes dimensions and scales exactly before the canonical numerical
rounding boundary. Non-exact rational scale roots reject in the initial profile.

Python constructs the same source with `eqiora.lang.quantity(1, units.Ohm.prefixed("k"))`.
It does not evaluate unit scales independently. A quantity retains its exact decimal
coefficient and exponent until the compiler composes the unit's decimal power. The resulting
decimal is rounded once to binary64 (nearest, ties to even). Neither the input number nor
the scale must be separately representable: `1e-400[km ^ 100]` denotes `1e-100[m ^ 100]`.
Nonfinite final values and nonzero values rounding to zero reject; representable subnormals
remain valid, and zero is canonicalized to positive zero. `0.1[nm]` therefore agrees with
`1e-10[m]` without an intermediate rounded multiplication.

Decimal tokens are bounded to 256 bytes before normalization. Their explicit and normalized
decimal exponents must fit signed 64-bit integers; composed unit scale powers retain their
signed 32-bit bound. Source identity preserves the exact normalized decimal value and unit
expression, while the Model stores the resulting binary64 value. Native finite values enter
source authoring through their shortest scientific decimal spelling; Python `Decimal` input
preserves an intentionally exact decimal value. Bare numerical expressions retain their
ordinary numerical evaluation boundary.

For independent conversions, 10 ms is 1/100 s, 1 kOhm is 1000 Ohm, 210 GPa is
210,000,000,000 Pa, and 1 uF is 1/1,000,000 F. Exact clocks retain the 1/100-second rational;
they do not recover it from a rounded numerical literal. Concrete clocks use
`periodic(10[ms], phase = 0[s])`; phase may be omitted. Exact literal arithmetic with
`+`, `-`, `*`, and `/` admits nonterminating rational seconds such as `periodic(1[s] / 3)`.
Every exact clock expression node has a reduced numerator magnitude and positive denominator
bounded by `u64`; dimension arithmetic and expression nesting keep their existing bounds.
The final period must be positive and the phase nonnegative, both dimensioned as time.
These checks do not evaluate value references or numerical operators and never pass through
binary64. This input boundary does not add clock-interface or multiclock scheduling semantics. Case or namespace collisions with
ordinary values do not affect these conversions.

Affine Celsius/Fahrenheit symbols are not admitted by this initial multiplicative catalog.
Absolute/difference quantity semantics must precede their admission; an offset cannot be
approximated by a scale or inferred from a property-input name.

## Exact dimension powers

Dimension exponents are reduced rational numbers. Write an integer directly or a fraction
in parentheses: `m ^ -1`, `m ^ (-1 / 2)`, or `Hz ^ (-1 / 2)`. Equivalent fractions such as
`m ^ (2 / 4)` and `m ^ (1 / 2)` denote the same dimension. Numerator magnitude and positive
denominator are bounded by 2,147,483,647, including the raw source tokens before reduction.

For a real constant wave amplitude on an interval, `(m ^ (-1 / 2)) ^ 2 * m` is dimensionless.
Likewise, `Hz ^ (-1 / 2)` equals `s ^ (1 / 2)`. `math.sqrt(area)` has dimension length when
`area` has dimension `m ^ 2`; real evaluation requires a nonnegative value, and its derivative
at zero is rejected. Dimension algebra does not choose a numerical branch for value powers.

## Structural aliases

`dimension name = dimension-expression;` defines a structural dimension alias. Top-level
exports may use `public`; notation follows the name. Aliases accept no `on`, `at`, numeric
scale, offset, or nominal quantity tag. The complete declaration scope is registered before
resolving alias dependencies; duplicate names, cycles, and resource excess reject.

Dimension aliases resolve in type expressions, independently of ordinary value names.
Input-unit expressions use only the closed unit catalog, so an alias is not a way to install
a hidden unit conversion. For example, a declared `dimension Energy = J;` can type an
operator result while its literal is still written `2 [J]`, not `2 [Energy]`.
Aliasing a dimension does not erase a connector, species, basis, or support's nominal identity.

Python uses the same structural dimension values and exact module resolver:

```python
from eqiora import Dimension, Module, ValueType

units = Module("units", package="org.example.mechanics")
units.dimension("Speed", Dimension(length=1, time=-1))
consumer = Module("main")
mechanics = consumer.import_module("mechanics", units)
speed_type = ValueType.real(mechanics.dimension("Speed"))
```

`Module.dimension` exports a public alias; `ModuleRef.dimension` admits one public
alias from its exact attached module, including parsed `.eqi` modules. The returned
`Dimension` remains structural and accepts ordinary `ValueType` construction. Emission
retains the alias declaration and explicit module import, while value types may use their
coherent SI expansion. Private or unknown aliases and invalid dependency cycles reject.

Presentation chooses a compatible output unit without changing the Model value. A plot label
cannot redefine a quantity or turn Hz into angular frequency. New catalog entries require
the shared source/Python/type checks rather than consumer-local string parsing.
