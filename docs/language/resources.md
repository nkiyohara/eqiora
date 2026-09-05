# Source resources and diagnostics

This is the proposed converged frontend profile, not a claim that the current parser already
enforces every bound. Feature implementations must enforce it before exposing the corresponding
grammar through source, Python authoring, or exact decoding. It extends existing compiler
limits without introducing another evaluator or registry.

## Bounded input and expansion

| Resource | Maximum |
|---|---|
| Raw UTF-8 source per module | 1,048,576 bytes |
| Lossless tokens per module, including trivia and EOF | 262,144 |
| Nested expression/type/body delimiters | 256 |
| Identifier/path-segment bytes | 4,096 |
| Qualified-name segments | 256 |
| Decimal numeric token bytes | 256 |
| Top-level declarations per module | 65,536 |
| Members in one container | 65,536 |
| Total elaborated members and expression nodes per source unit, independently | 1,000,000 each |
| Equality roots in one Relation | 65,536 |
| Named bindings in one instance | 65,536 |
| Elements in one expanded array, tensor, finite space, or product | 65,536 |
| Spatial tensor rank in the initial profile | 4 |
| Diagnostics returned for one module | 256 |

The existing identity owner's tighter applicable limits still apply to name totals, boundary
memberships, canonical encoding, and intermediate encoding work. Source/resource admission
does not authorize lifting those ceilings. New kinds count toward existing totals rather
than obtaining another independent million-node allowance.

Check raw bytes before token allocation, nesting before recursive descent, and products before
expansion. Counts include rejected input work where applicable: malformed tokens or a failed
branch cannot reset a budget. A depth-limited diagnostic must not recursively format the same
overdeep tree. Limits are inclusive; the first excess rejects without publishing a Model.

Dimension exponents use reduced rational values with numerator magnitude and positive denominator
at most 2,147,483,647. Zero has denominator one. Reject a zero denominator and normalize signs
before comparison. Cross-cancel and use checked exact intermediates for addition, multiplication,
and reduction; an out-of-range reduced result rejects. This never uses approximate equality or
a floating intermediate. A syntactically oversized rational token rejects before normalization.
Exact clock scheduling retains its own rational-time and scheduling-work limits; it does not
borrow floating quantity conversion or the dimension-exponent range.

## Diagnostic decisions

Use the existing diagnostic code and UTF-8 byte-range owners. The following messages define
the conditions for the source profile; names/ranges are structured context, not reason to
mint a new code for each specimen.

| Condition | Code | Message |
|---|---|---|
| Invalid source token | `EQ0601` | `invalid token` |
| Missing declaration terminator | `EQ0602` | `expected ';' after declaration` |
| Unknown declaration initializer | `EQ0602` | `unknown initialization belongs in an initial block` |
| Inapplicable declaration clause | `EQ0602` | `clause is not allowed on this declaration` |
| Missing explicit import alias | `EQ0602` | `qualified import requires 'as'` |
| Chained ordered predicate | `EQ0602` | `comparison chaining requires explicit Boolean composition` |
| Unresolved name or member | `EQ0603` | `unresolved name` |
| Duplicate binding | `EQ0603` | `duplicate binding` |
| Cyclic definition | `EQ0603` | `cyclic definition` |
| Wrong physical dimension | `EQ0603` | `incompatible dimensions` |
| Wrong nominal support/frame/basis/clock | `EQ0603` | `incompatible binding identity` |
| Invalid unit island | `EQ0603` | `invalid unit expression` |
| Nonpositive dimension denominator | `EQ0603` | `dimension denominator must be positive` |
| Resource excess during parsing | `EQ0602` | `source resource limit exceeded` |
| Resource excess during typed elaboration | `EQ0603` | `elaboration resource limit exceeded` |
| Typed operation without implementation admission | `EQ0001` | `requested mathematical profile is not implemented` |

For a resource diagnostic, include resource name, inclusive limit, and observed count where
bounded counting provides it. For a duplicate, identify both declarations; for a mismatch,
identify actual and expected typed roles. Distinguish invalid syntax/type from a valid request
that the selected execution profile cannot implement. Lowering and numerical failures retain
their existing phase-specific codes rather than being disguised as parser errors.

The primary range is the smallest offending token or clause. A missing terminator points to
the next significant token; end-of-file is its empty byte range. If the diagnostic limit is
reached, the final slot reports the diagnostic resource limit and analysis stops. A truncated
diagnostic list cannot be treated as a successfully compiled module.

## Focused parser and formatter probes

Use existing frontend tests, not a second parser framework. Each case below is a small source
probe for the implementation slice that admits its constructs.

- Reject `state memory: V = 0 [V];`, then recover a later complete `parameter gain: 1 = 2;`.
  Do not silently reinterpret the initializer as a numerical guess.
- Reject the wrong type in `parameter delay: s = 10 [V];`. Distinguish that error from a
  malformed `10 [s` unit island and from an unresolved indexing base in `samples[2]`.
- Recover after a missing semicolon before another declaration without consuming that
  declaration. Nested type/notation delimiters cannot synchronize at an unrelated inner brace.
- Round-trip `-x^2`, `x^-2`, `(x^y)^z`, and both sides of `a-b = c-d;` with their exact
  parsed grouping. Formatting a literal-zero equality needs no sentinel-escaping parentheses.
- Round-trip `complex<V>` and `if a < b then a else b` without treating value comparisons
  as generic type arguments. Reject `a < b < c`.
- Preserve attached `///` docs and ordinary comments around a declaration's type, support,
  and activation. Formatting canonical output a second time must be byte-identical.
- Test each finite bound at its inclusive limit and one over it where a focused construction
  can reach the gate. A huge input rejected at an earlier unrelated gate does not test the
  intended elaboration bound.

These checks test the actual changed frontend paths. A Markdown link check establishes only
document integrity; it does not establish parser acceptance, type correctness, numerical
accuracy, or source/Python parity for these target specimens.
