# Frozen expected observations

Both positive files format to the exact 187-byte `models/natural.eqi` bytes.
Their ordered root vector, excluding ranges, is:

```text
[
  Sub(Name(a), Name(b)),
  Sub(Sub(Name(a), Sub(Name(b), Name(c))), Name(d)),
  Sub(Neg(Name(a)), Neg(Name(b))),
]
```

The natural ranges are:

```text
Relation(balance)@106..184
Sub(Name(a)@140..141, Name(b)@144..145)@140..145
Sub(Sub(Name(a)@151..152,
        Sub(Name(b)@156..157, Name(c)@160..161)@155..162)@151..162,
    Name(d)@165..166)@151..166
Sub(Neg(Name(a)@173..174)@172..174,
    Neg(Name(b)@178..179)@177..179)@172..179
```

The explicit ranges are:

```text
Relation(balance)@106..196
Sub(Name(a)@140..141, Name(b)@144..145)@140..145
Sub(Sub(Name(a)@155..156,
        Sub(Name(b)@160..161, Name(c)@164..165)@159..166)@155..166,
    Name(d)@169..170)@155..170
Sub(Neg(Name(a)@181..182)@180..182,
    Neg(Name(b)@186..187)@185..187)@180..187
```

Natural equations, explicit residuals, locked packages, and native construction
have equal structural fingerprints. Their independently enumerated expression
trees above define the expected meaning; this language case does not pin the
comparison codec's digest bytes.

## Fixed statement table

`U` is exactly `1e-324`.

| Input | First root | Golden | Relation / root 0 / root 1 |
| --- | --- | --- | --- |
| `x = 0;` | `Name(x)` | `x = 0;` | `80..129 / 108..109 / 119..124` |
| `x = -0;` | `Name(x)` | `x = 0;` | `80..130 / 108..109 / 120..125` |
| `x = 1e-324;` | `Name(x)` | `x = 0;` | `80..134 / 108..109 / 124..129` |
| `x = (0);` | `Sub(Name(x), Number(+0))` | `x = (0);` | `80..131 / 108..115 / 121..126` |
| `x = (-0);` | `Sub(Name(x), Neg(Number(+0)))` | `x = (-0);` | `80..132 / 108..116 / 122..127` |
| `x = (1e-324);` | `Sub(Name(x), Number(+0))` | `x = (0);` | `80..136 / 108..120 / 126..131` |
| `x - 0 = 0;` | `Sub(Name(x), Number(+0))` | `x = (0);` | `80..133 / 108..113 / 123..128` |
| `x - (-0) = 0;` | `Sub(Name(x), Neg(Number(+0)))` | `x = (-0);` | `80..136 / 108..116 / 126..131` |
| `x - 1e-324 = 0;` | `Sub(Name(x), Number(+0))` | `x = (0);` | `80..138 / 108..118 / 128..133` |
| `x = ((0));` | `Sub(Name(x), Number(+0))` | `x = (0);` | `80..133 / 108..117 / 123..128` |
| `x = 0 * y;` | `Sub(Name(x), Mul(Number(+0), Name(y)))` | `x = 0 * y;` | `80..133 / 108..117 / 123..128` |
| `x = -(-0);` | `Sub(Name(x), Neg(Neg(Number(+0))))` | `x = --0;` | `80..133 / 108..117 / 123..128` |
| `x - y = z;` | `Sub(Sub(Name(x), Name(y)), Name(z))` | `x - y = z;` | `80..133 / 108..117 / 123..128` |
| `x = y - z;` | `Sub(Name(x), Sub(Name(y), Name(z)))` | `x = y - z;` | `80..133 / 108..117 / 123..128` |
| `-x = -y;` | `Sub(Neg(Name(x)), Neg(Name(y)))` | `-x = -y;` | `80..131 / 108..115 / 121..126` |
| `x - (y - z) = x;` | `Sub(Sub(Name(x), Sub(Name(y), Name(z))), Name(x))` | `x - (y - z) = x;` | `80..139 / 108..123 / 129..134` |

## Diagnostic table

| Pair | Count | Code/message | Graph path | Source span |
| --- | ---: | --- | --- | --- |
| missing lhs | 1 | `EQ0603`, exact unresolved `missing` | none | `diagnostic.eqi:52..59` |
| missing rhs | 1 | `EQ0603`, exact unresolved `missing` | none | `diagnostic.eqi:58..65` |
| missing both | 1, lhs only | `EQ0603`, exact unresolved `left_missing` | none | `diagnostic.eqi:34..46` |
| dimension | 1 | `EQ0603`, exact `[L]`/`[T]` mismatch | none | `diagnostic.eqi:79..97` |
| shape | 1 | `EQ0304`, incompatible-type prefix | `semantic.Relation.Relation:*…expression.2` | none |
| frame | 1 | `EQ0304`, incompatible-type prefix | same predicate | none |
| nominal support | 1 | `EQ0302`, incompatible-support prefix | same predicate | none |
| root support | 1 | `EQ0302`, residual/scope predicate | same predicate | none |

The malformed 96-byte source produces exactly two ordered `EQ0602`
diagnostics: `expected \`;\` after residual` at `88..89`, then the declaration
expectation at `95..96`, both in `malformed.eqi`.

## Ordered one-field mutants

The normal comparator first accepts each frozen observation and then rejects
exactly these 35 single-field clones, in order:

| # | Family | Sole changed field | Rejected value |
| ---: | --- | --- | --- |
| 1 | operator | `root.operator` | `Mul` |
| 2 | dropped rhs | `root.right` | absent |
| 3 | swapped operands | `root.ordered_operands` | `[b,a]` |
| 4 | sign normalization | `root.right` | `Name(b)` |
| 5 | operand order | `inner.ordered_operands` | `[c,b]` |
| 6 | root order | `roots` | swap 0 and 1 |
| 7 | addition | `root.operator` | `Add` |
| 8 | left precedence | `root.tree` | right-nested subtraction |
| 9 | right precedence | `root.tree` | left-nested subtraction |
| 10 | reassociation | `root.tree` | flattened left association |
| 11 | side swap bytes | `formatted_statement` | `b = a;` |
| 12 | sentinel distinction | `root.tree` | `Sub(x,0)` |
| 13 | zero escape omission | `formatted_statement` | `x = 0;` |
| 14 | negative-zero escape omission | `formatted_statement` | `x = -0;` |
| 15 | Neg collapse | `root.tree` | `Sub(x,0)` |
| 16 | underflow preservation | `formatted_statement` | `x = (1e-324);` |
| 17 | extra grouping | `formatted_statement` | `x = ((0));` |
| 18 | overbroad zero folding | `root.tree` | `Name(x)` |
| 19 | overbroad grouping | `formatted_statement` | `x = (0 * y);` |
| 20 | double-Neg collapse | `root.tree` | `Sub(x,0)` |
| 21 | root-start drift | `range.start` | `141` |
| 22 | lhs-only root | `range.end` | `141` |
| 23 | excluded RHS grouping | `range.end` | `114` |
| 24 | semicolon inclusion | `range.end` | `116` |
| 25 | next-statement drift | `range.end` | `121` |
| 26 | stale formatted offset | `range.start` | `155` |
| 27 | optional-span manufacture | `source_span` | `Some(205..220)` |
| 28 | optional-span erasure | `source_span` | none |
| 29 | identity overclaim | `required_equalities` | exact identity fields added |
| 30 | structural denial | `structurally_equivalent` | false |
| 31 | package overclaim | `comparison_kind` | exact artifact |
| 32 | native overclaim | `comparison_kind` | exact artifact |
| 33 | dimensionful positive-zero sentinel loss | `root.tree` | `Sub(force,0)` |
| 34 | dimensionful negative-zero sentinel loss | `root.tree` | `Sub(force,Neg(0))` |
| 35 | dimensionful underflow-zero sentinel loss | `root.tree` | `Sub(force,0)` |

The package source identity is
`org.eqiora.oracle.NaturalEquation@1.0.0`, path `models/natural.eqi`, role
`ModelSource`, entry `natural_equation_oracle`, with no dependencies. The
native declaration order is fields `a=4`, `b=3`, `c=2`, `d=1`, then Relation
`balance` with the same three ordered explicit residuals.
