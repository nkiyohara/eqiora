# Converged language: core rules

Working specification of the adopted converged language direction.
This draft describes the target language, not the grammar currently accepted by the compiler.
The complete declaration table, resource profile, and engineering and mathematical specimens
must be reviewed together before this specification is complete. Delivered behavior remains
indexed in the [capability matrix](../capability-matrix.md).

Specimens: [resistor divider](divider.md), [heated body](heated-body.md),
[sampled state](sampled-state.md), [data-backed property](data-backed-property.md),
[finite-state mathematics](finite-state.md), [harmonic RC response](harmonic-rc.md).
The [1D wavefunction](wavefunction.md) covers stationary normalization and complex time evolution.
The [Maxwell cavity](maxwell.md) covers vector evolution and oriented boundary traces.
The [stochastic specimen](stochastic.md) specifies explicit calculus and noise identity.
The [phase-separation specimen](phase-separation.md) specifies functional variations and mixed dynamics.
The [free-streaming specimen](free-streaming.md) closes a bounded position–velocity transport problem.
The [ion-transport specimen](ion-transport.md) combines species identity, molar flux, and Poisson coupling.

The [calculus and branching rules](calculus.md) include the foundation audit's explicit
partials, continuous time, second-order oscillator, and piecewise constitutive examples.
The [coordinate and measure rules](coordinates.md) specify product supports and partial integrals.
The [tensor and local-map rules](tensors.md) include the rank-four constitutive specimen.
The [numeric catalog](numeric-catalog.md) defines exact integers and bounded scalar operations.
The [unit catalog](units.md) fixes admitted symbols, prefixes, and structural dimension aliases.

## Source and modules

A source file is UTF-8. Identifiers are case-sensitive. Whitespace separates tokens but does
not terminate a statement. Braces delimit bodies, semicolons terminate statements, and commas
separate signature entries and arguments. There is no implicit multiplication.

Identifiers match `[A-Za-z_][A-Za-z0-9_]*`; Unicode remains available in documentation and
notation. Decimal tokens require a digit before any fractional point and digits after it,
with an optional `e`/`E` exponent and signed integer exponent value. Signs remain operators.
The [grammar productions](grammar.md) collect the shared syntax; specialized child rules
are defined on their linked owner pages.

`//` introduces a line comment; `///` attaches documentation to a declaration. Declaration
notation uses `@{...}` immediately after the declaration name. Its contents are a bounded
notation AST, not executable source or arbitrary TeX. Neither documentation nor notation
introduces a mathematical value or changes name resolution.

Module identity is the exact package identity plus portable relative source path. Source
does not declare or rename its own module. Imports name the full target and require an
explicit local `as` binding. A local import name never changes the imported declaration's
identity. There are no wildcard imports, textual includes, ambient package aliases, inherited
equations, or equation overrides. Standard packages use the same manifest and exact lock
resolution as other packages.

The parser retains source ranges and invalid fragments. Compilation publishes no partial
Model when a source, type, or binding error remains.

## Declaration heads

The shared order is:

```text
kind name [@{notation}] [: type] [on support] [at activation]
```

Brackets here mean optional grammar, not literal source brackets. Optional syntax does not
make every clause legal for every kind. The tables are closed; specialized children are typed
constructs, not arbitrary attributes.

| Kind | Type | `on` | `at` | Definition |
|---|---|---|---|---|
| `parameter` | Required | No | No | `= expression` in a body; optional default in a signature |
| `variable` | Required | Optional | Optional | No declaration initializer |
| `state` | Required | Optional | Optional | No declaration initializer |
| `let` | Optional assertion | Optional assertion | Optional assertion | Required `= expression` |
| `input` | Required | Optional | Optional | External causal input in a signature |
| `output` | Required | Optional | Optional | Owned causal output in a signature; equations in the body |
| `port` | Required connector type | Optional | Optional | Owned connector occurrence; role laws come from its connector |
| `clock` | `periodic` for a requirement | No | No | Signature requirement or concrete `= periodic(...)` in an owning scope |
| `support` | Required support contract | No | No | Signature requirement or exact derived product/boundary in a body |
| `observable` | Required | Optional assertion | Optional assertion | Derived `= expression`; no solve unknown |
| `test` | Required | Inferred from trial field | Inferred from trial field | `for field`, with an optional `zero_on` boundary restriction |
| `property` requirement | Required exact contract reference | No | No | Signature requirement bound to an exact release |
| `coordinate` | Required coordinate dimension | Required | No | `from` exact coordinate factor; no initializer |
| `space` | None | No | No | `= orthonormal(...)` or `= product(...)` |
| `noise` | Required admitted noise contract | No in the scalar profile | No | Exact process-channel declaration |
| `amplitude` | Required complex value type | Inherited/asserted | No | Form child `for` an exact original value |

| Container or definition | Header after name/notation | Contents |
|---|---|---|
| `component`, `model` | Required parenthesized signature, including `()` | Braced private body |
| `connector` | No value type or support/activation clauses | Named typed member roles |
| `operator` | Parenthesized input signature, `: result-type` | `= expression;` |
| `property contract` | Parenthesized input signature, `: result-type` | Closed contract children |
| `property release` | `: qualified-contract` | Closed definition, validity, provenance, and license children |
| `relation` | Optional `on`, then optional `at` | Braced simultaneous equalities |
| Conservation `law` | Required `on support` | `storage`, `flux`, `source` children |
| Stochastic `law` | `for state` | `calculus`, `drift`, `diffusion` children |
| `form` | `for` exact Law or Relation | Typed tests, relations, or selected reduction children |
| `instance` | `: qualified-component(named-bindings)` | Semicolon; no overriding body |

Notation always follows the declared name, before its signature or type. Instance support
and clock requirements are named bindings; `on` or `at` does not override a component body.
Omitting an instance argument list is not another canonical spelling: use `Ground()`.

A property requirement binds a release, not the scalar obtained by evaluating that release.
The [property declaration rules](properties.md) define its contract and release children.
The contract owns the independent-variable signature and result type; each call supplies its
named inputs. State-dependent input does not make a pure property call mutable. Support and
activation come from the typed call arguments, subject to the contract; the release cannot
perform sampling, history updates, or support conversion implicitly.

`relation name [on support] [at activation] { ... }` contains simultaneous equations.
`law name on support { ... }` contains one physical `flux`, one `source`, and optional
`storage` expression, each terminated by a semicolon. Omitting storage means a steady balance,
not inferred zero initial energy. Its fixed-domain convention is
`derivative(storage) + div(flux) = source`; moving-domain transport needs its own admitted
contract. `form name for law { ... }` owns mathematical trial/test roles and equations,
not mesh or solver configuration. These are closed typed children, not string-valued attributes.

Signature `variable` and `state` entries borrow exact external occurrences. They do not allocate
private unknowns or transfer ownership of state initialization and updates. Signature `input`
entries require a compatible driver; signature `output` entries expose values defined by the
body. A `port` exposes its connector's members and participates in typed connection equations.
An `on` or `at` clause on a port must satisfy that connector's admitted support and activation
contract. It cannot change its member roles.

An omitted `on` on a declared unknown means a lumped value, not an inferred spatial field.
An omitted `at` on a declared unknown means continuous time, permanently. `on` and `at` are
independent: a clocked spatial state and a continuous lumped state are both meaningful.
A parameter is fixed mathematical input; a spatially varying constitutive expression belongs
in a typed property, operator, or derived expression, not an implicitly mutable parameter.

A `let` derives its support and activation from its expression. Written clauses assert those
derived facts; they cannot sample, hold, broadcast, or relocate the expression. Parameter-only
aliases are static. A runtime-dependent alias cannot define an array extent, clock period,
package identity, or other static requirement.

```eqiora
parameter viscosity @{\mu}: Pa * s = 1.002e-3 [Pa * s];
variable pressure @{p}: Pa on fluid;
state temperature @{T}: K on solid;
state memory: V at control;
initial { memory = 0 [V]; }
```

`variable` introduces an algebraic unknown; `state` owns evolution or history. An `initial`
block contains simultaneous mathematical initialization equations. It is neither an ordered
assignment program nor a collection of solver guesses. Restart consumes accepted State and
does not execute initialization again. Numerical guesses, scales, time steps, and output
schedules belong to the numerical/execution interface.

Clock identity is nominal and exact. Two clocks with equal periods are not interchangeable.
An activation must resolve to an admitted typed clock or event. Crossing activation boundaries
requires an explicit admitted transition such as sample or hold.

`clock control = periodic(10 [ms], phase = 0 [s]);` creates a clock in its owning scope.
The first argument is a positive exact period; the optional nonnegative exact phase defaults
to zero. Ticks occur at `phase + k * period` for nonnegative integer `k`, measured from fresh
initialization's time origin. A delayed first tick uses an explicit positive phase. Initialization
precedes a tick at zero. `period(control)` is an exact static time quantity. A signature entry
`clock tick: periodic` borrows a supplied clock and does not create a second schedule.

At a tick, `pre(state)` reads committed pre-tick state and `next(state)` denotes the candidate
post-tick state. All same-clock update equations are solved together and committed atomically.
A rejected attempt changes no accepted state. An owned state with no admitted update retains
its committed value; it does not become zero or acquire another owner's update. Clocked inputs
and outputs exist at ticks, not as implicit continuous held signals.

The target has no source `field`, `field slot`, `as continuum`, postfix `shape`, explicit
`continuous`, or initialized unknown declaration. Kernel Fields and continuum semantics retain
their existing owners.

## Mathematical types

| Source type | Meaning |
|---|---|
| `V` | Real scalar with voltage dimension |
| `1` | Dimensionless real scalar |
| `m^(-1/2)` | Real scalar with an exact rational dimension exponent |
| `complex<V>` | Complex scalar with voltage dimension |
| `vector<V, 3>` | Three-component spatial vector |
| `tensor<complex<Pa>, 3, 3>` | Rank-two spatial tensor with complex pressure components |
| `array<V, 3>` | Three indexed voltage channels, not a spatial vector |

There is no redundant `real<...>` constructor. A vector or tensor contains a scalar type;
an array contains a checked element type and exact static extent. Spatial frame information
may be inferred only from a unique exact support contract. Without that context a frame
requirement must be supplied explicitly; an extent alone cannot select a frame.

Finite component spaces and maps use the [finite-space grammar](finite-spaces.md).
They are distinct from both arrays and
spatial vectors. Equal size does not permit substitution between two bases, between bases
and spatial frames, or between a basis and a coordinate domain.

Array indexes are zero-based exact integers. An extent is a positive static integer; zero-sized
arrays are rejected. Indexing preserves element type and does not choose a component basis or
perform a coordinate transformation. Expansion and element-count limits are checked before
allocation or elaboration. There is no implicit broadcasting, reshaping, or basis conversion.

Dimensions use exact reduced rational exponents of the SI base dimensions. Dimension aliases
are structural: they neither scale a value nor introduce nominal quantity identity. Rational
normalization has a positive denominator, coprime numerator and denominator, and one zero.
Precision, storage layout, and backend do not enter a mathematical scalar type.

Addition and equality require compatible dimensions, shapes, frames, supports, and activation.
Real-to-complex embedding preserves dimension and value. Complex-to-real conversion requires
an explicit mathematical projection. Ordered predicates require real scalars; complex values
can be compared for equality but not ordered.

A bare literal zero can take the scalar domain, dimension, and shape uniquely required by its
context. For example, `voltage = 0;` uses a voltage zero. An unconstrained zero is dimensionless
real scalar zero. Contextual zero does not create a frame, support, clock, or basis conversion.
A nonzero dimensionless number never acquires units from context.

## Numbers, units, and brackets

The sign of a number is an expression operator, not part of its numeric token. Decimal and
decimal-exponent literals have exact source values before quantity normalization. A number
token followed by `[` starts a quantity island, even across whitespace. Inside the island,
identifiers resolve in the unit catalog, independently of value names.

```eqiora
10 [ms]
210 [GPa]
998.2 [kg / m^3]
```

By contrast, `samples[2]` indexes a value, and `[a, b]` is an array literal in expression
position. A numeric literal cannot be indexed: `10[2]` is an invalid quantity island, not an
alternative spelling of indexing. Multiplication must be written explicitly.

Dimension and unit expressions use `*`, `/`, parentheses, and `^` with an exact exponent:

```text
exponent = signed-integer | "(" signed-integer "/" positive-integer ")"
```

Thus `m^(-1/2)` is a dimension, while `x^(1/2)` in a value expression is numerical exponentiation.
Dimension exponent arithmetic never passes through floating point. Reduced exponents determine
dimension equality; spelling remains source provenance.

Type constructors consume `<` and `>` only in type position. In value position they are ordered
predicates. Calls have no value-position angle-bracket specialization. This separates
`complex<V>` from `a < b` without symbol-table-dependent tokenization.

Scaled units convert through one compiler-owned catalog. Exact decimal input and exact
multiplicative scale compose before a single binary64 rounding boundary for a canonical
numerical literal. Overflow, nonfinite results, and nonzero input rounded to zero are rejected.
Exact clocks retain rational time instead. A rational power of a scaled unit is accepted only
when its scale root is exact; otherwise it is rejected rather than represented by a guessed
rational scale. Affine input units require the separate absolute/difference quantity contract.

## Expressions and equations

From strongest to weakest binding:

| Operation | Associativity |
|---|---|
| Parenthesized expression; call, member access, indexing | Postfix, left to right |
| `^` | Right |
| Unary `+`, `-` | Prefix; power binds inside its operand |
| `*`, `/` | Left |
| `+`, `-` | Left |
| `<`, `<=`, `>`, `>=`, `==`, `!=` | Non-associative |
| `not` | Prefix Boolean negation, below comparisons |
| `and` | Left, short-circuit |
| `or` | Left, short-circuit |
| `if predicate then value else value` | Right-nested conditional expression |

Power admits a signed right operand. Therefore `-x^2` means `-(x^2)`, `x^-2` means
`x^(-2)`, and `x^y^z` means `x^(y^z)`. Parentheses are required for chained predicates;
`a < b < c` is rejected rather than interpreted as either a conjunction or numeric coercion.

`=` is not an expression operator. In a Relation or `initial` block, `lhs = rhs;` introduces
one simultaneous equality. In a parameter default, alias definition, or named argument, the
enclosing typed construct determines its meaning. `==` produces a Boolean predicate and does
not introduce a physical equation. Booleans do not implicitly become dimensionless numbers.

Every Relation equality lowers from both operands; literal zero has no parser sentinel role.
The system is not evaluated as assignment statements. Retaining operand and equation order
for exact identity does not create imperative execution order.

`math.i` is the dimensionless imaginary unit. `math.complex(real_part, imaginary_part)` constructs
a complex scalar from real operands with equal dimensions. It is the canonical explicit
construction; neither bare `i`/`j` suffixes nor implicit imaginary-part removal are admitted.
For example, `math.complex(2 [V], 3 [V])` has type `complex<V>`.

Dimension legality does not establish a numerical domain. Real square roots require nonnegative
arguments; real logarithms require positive dimensionless arguments. Complex principal roots
and logarithms use argument in `(-pi, pi]`; the logarithm is undefined at zero. A request for a
derivative on a branch cut or singularity must satisfy the specific derivative contract, not
inherit admission from value evaluation. A dimensional power needs a statically exact rational
exponent; a runtime exponent requires a dimensionless base.

`inner(a, b)` conjugates its first argument. Plain contraction does not. Transpose and adjoint
are distinct, and a metric-dependent adjoint must retain that metric. Operators are pure:
evaluation cannot perform I/O, execute a host callback, mutate history, or advance a random
stream. Unsupported value or derivative operations fail explicitly at their common owner.

## Scope and resolution

Component and Model signatures own their public requirements. Their bodies are private but
inspectable by tools. Register the full signature before resolving defaults or types; textual
order is not evaluation order. Register body declaration names before resolving expressions.
This permits forward references, not recursive expansion or cyclic definitions.

Reject duplicate names within one scope, cyclic aliases or defaults, recursive operator calls,
and recursive component expansion. A simultaneous equation dependency is not a definition
cycle: coupled algebraic unknowns are intentional. Whether an equation system can be executed
is a later Formulation/Realization admission decision.

Instance arguments use `name = value`. The signature determines whether that value binds a
parameter, support, clock, property, port, or reference; callers do not repeat the category.
Each required external requirement is bound exactly once; only requirements with declared
defaults may be omitted. An exposed owned port or output is created by the occurrence rather
than supplied as an external requirement. Binding borrowed state preserves its identity and
owner instead of allocating another state.
Private members cannot be imported or bound as an exposed interface.

Signature names and body names share one declaration namespace within a container; a body
cannot shadow a signature requirement. An operator's formal scope is separate from the caller.
Substitution retains binding identity, not just the spelling of a formal or an import alias.

A pure scalar operator evaluates pointwise when its arguments carry one common exact support
and activation. This is the scalar operator's lifted application, not a second field evaluator.
Static scalar coefficients can be constant functions in that context; a runtime value on a
different support or clock cannot. Scalar-to-vector broadcasting, implicit product-support
construction, and hidden sample/hold remain forbidden. The result retains the checked common
support/activation independently of its scalar dimension.

## Diagnostics, recovery, and formatting

Use the existing [diagnostic registry](../diagnostics.md): `EQ0601` for invalid tokens,
`EQ0602` for grammar errors, and `EQ0603` for unresolved names or static types. Do not reuse a
code for a different condition. Diagnostics identify the smallest offending UTF-8 byte range;
duplicate bindings also identify the original binding. An end-of-file error uses the empty
range at end of input.

Recovery must make progress, respect nested delimiters, and preserve later declarations.
A malformed unit island or type constructor must not consume the next complete declaration.
The existing lossless lexer and recovering parser remain the owners; no new parser framework
is needed. The [resource and diagnostic profile](resources.md) specifies finite bounds and
focused rejection/recovery examples.

Canonical formatting must preserve parsed mathematical structure, attached documentation,
and ordinary comments. It must be idempotent and preserve binding through parentheses, including
unary minus, power, and both equality operands. Source locations and whitespace may change;
the compiler-owned authoring projection must not. Invalid source can support diagnostics and
partial editor analysis, but must not be presented as a successfully canonicalized Model.
