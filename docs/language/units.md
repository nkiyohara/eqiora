# Initial unit and dimension catalog

This target catalog supplies the units used by the [language specimens](core.md). Type
dimensions, scaled input units, and output presentation remain separate. The compiler owns
one catalog shared by source and explicit Python `eqiora.units`; no root-level unit aliases
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

## Prefixes and conversion

The initial prefix set is closed:

| Prefix | Scale |
|---|---|
| `n` | 1/1,000,000,000 |
| `u` | 1/1,000,000 |
| `m` | 1/1,000 |
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
is valid target source; a type `kOhm` cannot encode a scale into its dimension. A quantity's
product/division/power composes dimensions and scales exactly before the canonical numerical
rounding boundary. Non-exact rational scale roots reject in the initial profile.

For independent conversions, 10 ms is 1/100 s, 1 kOhm is 1000 Ohm, 210 GPa is
210,000,000,000 Pa, and 1 uF is 1/1,000,000 F. Exact clocks retain the 1/100-second rational;
they do not recover it from a rounded numerical literal. Case or namespace collisions with
ordinary values do not affect these conversions.

Affine Celsius/Fahrenheit symbols are not admitted by this initial multiplicative catalog.
Absolute/difference quantity semantics must precede their admission; an offset cannot be
approximated by a scale or inferred from a property-input name.

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

Presentation chooses a compatible output unit without changing the Model value. A plot label
cannot redefine a quantity or turn Hz into angular frequency. New catalog entries require
the shared source/Python/type checks rather than consumer-local string parsing.
