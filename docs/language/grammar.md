# Shared grammar productions

This grammar accompanies the [core decision tables](core.md). Bracketed productions are
optional, braces repeat, quoted text is literal source, and `|` selects alternatives.
Applicability, scope, and type rules narrow these productions; parsing a head does not make
every clause legal. Specialized constructs remain closed at their named owner pages.

```text
qualified-name = identifier {"." identifier}
import = "import" qualified-name "as" identifier ";"
annotation = "@{" notation-ast "}"
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
instance = "instance" identifier [annotation] ":" qualified-name "("
           [named-argument {"," named-argument}] ")" ";"
named-argument = identifier "=" expression-or-exact-reference
initial = "initial" "{" {equation} "}"
relation = "relation" identifier [annotation] ["on" qualified-name]
           ["at" qualified-name] "{" {equation} "}"
equation = expression "=" expression ";"
connection = "connect" qualified-name "->" qualified-name ";"
           | "connect" qualified-name "," qualified-name {"," qualified-name} ";"
           | "connect" "periodic" qualified-name "," qualified-name ";"
operator = ["public"] "operator" identifier [annotation] signature ":" type "=" expression ";"
```

The operator signature contains only admitted pure input/static requirements. Component/Model
signatures admit the roles in the core table. A reference argument retains its typed role;
the broad grammar above does not convert a support, clock, property, or state into a numeric
expression. No repeated argument-category words or positional instance bindings are admitted.

```text
scalar-type = dimension | "complex" "<" dimension ">"
type = scalar-type | "bool" | "integer" | "index" "<" qualified-name ">"
     | "vector" "<" scalar-type "," static-extent ">"
     | "tensor" "<" scalar-type "," static-extent "," static-extent
       {"," static-extent} ">"
     | "array" "<" type "," static-extent ">"
     | coordinate-type | map-type | qualified-nominal-type
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
primary = quantity | number | qualified-name | "true" | "false"
        | "(" expression ")" | "[" [expression {"," expression}] "]"
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
