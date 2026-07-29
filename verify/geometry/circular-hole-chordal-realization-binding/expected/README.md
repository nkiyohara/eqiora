# Expected values

The non-implementing oracle is
[`../oracle/binding_oracle.py`](../oracle/binding_oracle.py), SHA-256
`1d920610df68b7256bda2f9186978aaec4df11dc4546bd11d6e4b3192ccb83db`. It freezes
every expected value in
[`binding-contract.json`](binding-contract.json) and reports 73 checks with 0
failures. An ordinary run re-derives that fixture and compares it byte-for-byte,
so the file cannot drift away from the derivation that produced it.

These values are frozen ahead of implementation. None is derived from
production output, and none may be tuned or relaxed by the implementing lane: an
implementer who believes a value is wrong returns the proof rather than
adjusting the value.

## Exactly derived

| Quantity | Value |
| --- | --- |
| exact source canonical bytes | 511 |
| exact source identity | `b00123472a596e8289820cabaee20d52cdf81b5572fa9ce58ff17cdaa00046d9` |
| allowance scale | `2.2` m |
| evaluation allowance | `6.252776074688882e-14` m, exactly `2476979795053773 / 2^95` |
| effective budget | `0.00009999999993747225` m |
| accepted segment count | 50 |
| `sagitta(49)` | `1.0273036248318289955797595210037224856637053318839e-4` m |
| `sagitta(50)` | `9.8663578586421902383159656827472333154739014922844e-5` m |
| ideal area deficit at n = 50 | `2.0654536205467760336685969666957589060533063430286e-5` m² |
| ideal perimeter deficit at n = 50 | `2.0666771241244346537321549979462280729278040417922e-4` m |

The four high-precision values are reproduced here from this oracle's own
`pi`/`sin`/`cos`/`asin` kernel, independently of the sibling chordal case's
oracle, and agree with it to every digit either freezes. The segment count is
derived twice — once by monotone search over `sagitta(n)`, which uses no inverse
function, and once by RFC 0082's stable half-angle inverse — and the two routes
are required to agree.

## Deliberately not expected values

Three bound resource digests and three measured metrics are **not** frozen as
constants, because published contracts do not determine them; the case README
gives the proof per field. The oracle reports them under
`not_derivable_from_published_contracts` rather than inventing a value.

The 765-byte envelope in `encoding_witness` is therefore an **encoding witness**
with three declared slots, not a prediction. It is marked
`is_dfg_realization_prediction: false`. Wiring its digest
`e44e8371f2bb8e9f878696de7efffe0ea2f3714bc0354b376b03344310f71775` as a positive
oracle for the real DFG chain would be a false positive, and is forbidden.

The reusable expected value is the function `canonical_envelope(values)` in the
oracle: it takes the thirteen real field values an implementation produced and
independently derives the canonical bytes and identity those values must have.

`boundary_error_bound_m`, `circle_area_deficit_m2`, and
`circle_perimeter_deficit_m` are frozen only as ideal high-precision values plus
the binary64 spelling of those ideals. RFC 0082 stores measured quantities
there, which may differ in the last places.
