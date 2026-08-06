# Independent acceptance oracle

The exact ordinary canonical source is `natural.eqi` byte-for-byte. Formatting
either ordinary fixture must produce:

```eqiora
model NaturalEquation {
  field lhs: 1 = 3;
  parameter rhs: 1 = 2;
  relation balance continuous {
    lhs = rhs;
  }
}
```

The independently frozen structural fingerprint digest is
`e7662b36983484f8385eb4d5f595f86c87d6aed3e4c6276c569049dc033e9cc5`.
It is semantic comparison evidence, not an artifact identity.

The underflow witness is `1e-400`, an accepted finite spelling whose binary64
value compares equal to zero. Bare `0`, `-0`, and `1e-400` remain the legacy
sentinel and format as `= 0`. Parenthesized or explicit-residual zero and
underflow retain `Sub(x, Number(0))` and format as `x = (0);`.
`Sub(x, Neg(Number(0)))` formats as `x = (-0);`. Extra parentheses,
underflow spelling preservation, Neg collapse, computed/name/call zero
folding, and omitted escaping are rejecting mutants.

For the exact inline parser-range source, the Relation range is `[46,95)`.
Its first natural root, lhs, and parenthesized rhs ranges are `[70,81)`,
`[70,73)`, and `[76,81)`; its second root, lhs, and rhs ranges are
`[83,92)`, `[83,86)`, and `[89,92)`. The semicolon and next statement are
excluded. Reformatting owns new offsets rather than preserving stale ones.

Natural static failures are compared with the existing explicit-residual
public route. Intrinsic-name and dimension errors retain their existing
source-span presence. Flat shape, frame, nominal-support, and Relation-root
support errors retain graph-path diagnostics with no source span. Inventing a
whole-equation downstream span is a rejecting mutant.

Fresh source, package, and native compilation keeps exact independent
occurrence and artifact lineage. Cross-route canonical Model bytes, artifact
references, digests, source digests, package identities, releases, locks, and
compilation records are not equality expectations.
