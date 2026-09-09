# Shared grammar productions

This grammar accompanies the [core decision tables](core.md). Bracketed productions are
optional, braces repeat, quoted text is literal source, and `|` selects alternatives.
Applicability, scope, and type rules narrow these productions; parsing a head does not make
every clause legal. Specialized constructs remain closed at their named owner pages.

```text
qualified-name = identifier {"." identifier}
import = "import" qualified-name "as" identifier ";"
annotation = "@{" notation-ast "}"
dimension-declaration = ["public"] "dimension" identifier [annotation] "=" dimension ";"
value-head = value-kind identifier [annotation] [":" type] ["on" qualified-name]
             ["at" qualified-name]
value-kind = "parameter" | "variable" | "state" | "let" | "input" | "output"
           | "port" | "observable" | "property"
signature = "(" [signature-entry {"," signature-entry}] ")"
signature-entry = value-head ["=" expression] | support-requirement | clock-requirement
support-requirement = "support" identifier [annotation] ":" support-contract
clock-requirement = "clock" identifier [annotation] ":" "periodic"
container = ["public"] ("component" | "model") identifier [annotation] signature body
body = "{" {body-item} "}"
value-declaration = value-head ["=" expression] ";"
event-declaration = "event" identifier "=" "crossing" "(" expression ","
                    "direction" "=" crossing-direction ")" ";"
crossing-direction = "any" | "rising" | "falling"
indexset-declaration = "indexset" identifier [annotation] "=" "range" "(" static-extent ")" ";"
index-family = "[" identifier "in" qualified-name "]"
instance = "instance" identifier [annotation] [index-family] ":" qualified-name "("
           [named-argument {"," named-argument}] ")" ";"
named-argument = identifier "=" expression-or-exact-reference
initial = "initial" "{" {equation} "}"
boundary-family = "[" identifier "in" qualified-name "]"
relation = "relation" identifier [annotation] [index-family] ["on" qualified-name]
           ["at" qualified-name] "{" {equation} "}"
equation = expression "=" expression ";"
scalar-connector = ["public"] "connector" identifier "{"
                   "across" identifier ":" type ";"
                   "through" identifier ":" type ";" "}"
field-connector = ["public"] "connector" identifier "{"
                  "trace" identifier ":" dimension ";"
                  "flux" identifier ":" dimension ";"
                  "shape" connector-shape ";" "frame" connector-frame ";"
                  "pairing" "euclidean_boundary_duality" ";"
                  "orientation" "parent_outward" ";" "}"
connector-port = "port" identifier ":" qualified-name ["over" qualified-name] ";"
connector-shape = "scalar" | "spatial_vector" | "[" positive-integer {"," positive-integer} "]"
connector-frame = "invariant" | "spatial"
connection = "connect" [index-family] expression "->" expression {"," expression} ";"
           | "connect" [index-family] expression "," expression {"," expression} ";"
           | "connect" "periodic" qualified-name "," qualified-name ";"
record = ["public"] "record" identifier [annotation] "{" record-member {"," record-member} [","] "}"
record-member = identifier [annotation] ":" type
operator = ["public"] "operator" identifier [annotation] signature ":" type "=" expression ";"
```

The operator signature contains only admitted pure input/static requirements. Component/Model
signatures admit the roles in the core table. A reference argument retains its typed role;
the broad grammar above does not convert a support, clock, property, or state into a numeric
expression. No repeated argument-category words or positional instance bindings are admitted.

A Relation family ranges over an exact IndexSet or a Component's complete-exterior
requirement. A boundary family follows notation and precedes `on`; `on` must name the
bound boundary member. An indexed Relation retains its ordinary support and activation.
The [core family rules](core.md#boundary-relation-families) define its scope and expansion.
An instance's index-family instead ranges over an exact finite index set. Its bound index
cannot substitute for a boundary member. The [numeric catalog](numeric-catalog.md) defines
its static extent, nominal identity and indexed member references.

Private Model/Component events follow the [crossing-event profile](events.md). Named
Relation activations resolve to their declared clock or event; the parser does not classify
a name as periodic. Event declarations infer the guard type from its expression.

Imports precede declarations. Library files may contain declarations without a Model; an
execution entry selects a Model explicitly. Top-level declaration order does not control
resolution, and a component cannot redefine an imported equation. The existing package resolver
retains module-cycle and closure admission rather than running source as import-time code.

```text
scalar-type = dimension | "complex" "<" dimension ">"
type = scalar-type | "bool" | "integer" | "index" "<" qualified-name ">"
     | "vector" "<" scalar-type "," static-extent ">"
     | "tensor" "<" scalar-type "," static-extent "," static-extent
       {"," static-extent} ">"
     | "array" "<" type "," static-extent ">"
     | coordinate-type | map-type | "counts" "<" qualified-name ">" | qualified-nominal-type
dimension = dimension-product
dimension-product = dimension-power {("*" | "/") dimension-power}
dimension-power = dimension-atom ["^" dimension-exponent]
dimension-atom = "1" | qualified-dimension-name | "(" dimension ")"
dimension-exponent = signed-integer | "(" signed-integer "/" positive-integer ")"
quantity = number "[" unit-expression "]"
```

Unit expressions use the same product/power syntax but resolve through the unit catalog,
not the dimension-alias or value namespaces. Static extents are exact bounded positive
integer expressions; a numeric expression depending on runtime state is not an extent.
The parser commits to a quantity island when a numeric token is followed by `[`; it cannot
fall back to numeric indexing if the unit expression is invalid. In type arguments, static
integer expressions exclude comparison/Boolean operators, so a closing `>` is unambiguous.

```text
expression = conditional
conditional = "if" expression "then" expression "else" conditional | disjunction
disjunction = conjunction {"or" conjunction}
conjunction = negation {"and" negation}
negation = "not" negation | comparison
comparison = additive [("<" | "<=" | ">" | ">=" | "==" | "!=") additive]
additive = multiplicative {("+" | "-") multiplicative}
multiplicative = signed-power {("*" | "/") signed-power}
signed-power = ("+" | "-") signed-power | power
power = postfix ["^" signed-power]
postfix = primary {call-arguments | "[" expression "]" | "." identifier}
primary = quantity | number | qualified-name | "true" | "false" | finite-reduction
        | "(" expression ")" | "[" [expression {"," expression}] "]"
finite-reduction = ("sum" | "product" | "min" | "max") "(" expression "," "over" "="
                   "(" identifier "in" qualified-name ")" ")"
call-arguments = "(" [argument {"," argument}] ")"
argument = expression | named-argument | typed-structural-argument
```

Structural arguments such as `holding`, contraction axes, coordinate assignments, and finite
sum binders use their operation's exact grammar; they do not introduce a general assignment
expression, runtime tuple metaprogramming, or user-extensible keyword bag. Ordinary calls
cannot mix positional and named binding styles or bind one formal twice. Pure authored
operators use named inputs; structural compiler operators use their documented positional
and named roles. The recursive power production retains right associativity and signed exponents.

See [properties](properties.md), [finite spaces](finite-spaces.md), [coordinates](coordinates.md),
[calculus](calculus.md), [harmonic forms](harmonic-rc.md), [eigenpairs](wavefunction.md),
[variations](phase-separation.md), and [stochastic Laws](stochastic.md) for specialized children.
The [resource profile](resources.md) applies before recursion or expansion.

Conditional value expressions use the [scalar branch profile](conditionals.md). `if`, `then`
and `else` delimit values; both branches are statically checked and runtime evaluation selects
only one. Parenthesize a conditional used as a tighter arithmetic or `not` operand.

Typed operator definitions use the [operator profile](operators.md):

```text
operator-declaration = ["public"] "operator" name "(" formal {"," formal} ")"
                       ":" operator-type "=" expression ";"
formal = "input" name ":" operator-type
operator-type = "scalar" | "spatial" "[" integer "]" | value-type
named-call = qualified-name "(" name "=" expression {"," name "=" expression} ")"
```

Named and positional arguments cannot mix. Builtins retain their specified positional or
named roles; user-defined operator calls use formal names. Exact body purity, dimensions,
lexical composition and resource limits are compiler obligations.
