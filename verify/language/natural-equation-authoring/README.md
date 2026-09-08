# Uniform equation authoring

Every Relation statement retains ordered authored `lhs` and `rhs`, including
zero. The compiler checks both operands using the shared mathematical types
and exact spatial support while retaining both sides in the Semantic Model. The checked numerical
projection constructs ordered residuals for numerical execution. A bare
literal zero can inherit the other operand's mathematical type. An explicitly
typed zero retains its dimension, shape, frame and array roles. The neutral
rule `lhs - zero → lhs` applies only when the checked result has exactly the
left operand's complete type and support; it cannot discard complex promotion.

This case first compiles an ordinary nonzero natural equation, then checks
independently enumerated authored sides and projected numerical residuals, each source form's
own canonical roundtrip, UTF-8 byte ranges, sixteen fixed zero/precedence rows,
actual source falsifiers, source-local zero/underflow denials, and public
structural comparison across direct, exact-package and native construction.

```bash
mise run fast -- --case language.natural-equation-authoring
mise run affected -- --case language.natural-equation-authoring
```

The fixture corpus has four fields at most, inputs at most 4 KiB, projected
DAGs at most 32 nodes, per-document formatter output at most 1 KiB, and each
direct canonical Model vector at most 256 KiB. These bound this executable
witness, not product performance or allocation. Product expression depth
and activation/type boundaries have focused owner tests.

The authored equations `a = b` and `a - b = 0` have distinct structural
identity even when their checked numerical residuals agree. Exact-package
and native routes share identity only when they retain the same ordered
equation sides. Side and equation order remain structural identity even when
equation permutation preserves a
simultaneous mathematical solution. This case does not claim general algebraic
equivalence or a new execution backend.
