# RFC 0003: Eqiora Language frontend v0

- Status: Frontend architecture; grammar defined by the converged language specification
- Authors: Eqiora contributors
- Created: 2026-07-17

## Summary

A byte-lossless lexer, recovering parser, source AST, and idempotent formatter
form the frontend; semantic lowering is a separate compiler-layer operation.
The [converged language specification](../docs/language/core.md) owns source
grammar and semantic decisions. This RFC describes the frontend architecture.

## Layer boundary

```text
source bytes → lossless tokens → recursive source AST → typed compiler lowering
                                                    → graph Transaction
```

`eqiora-lang` depends only on L0 core diagnostics. It does not know graph
stores, transactions, or reference execution. This avoids a same-layer cycle
between the language frontend and Graph Federation. The recursive AST is a
source representation only; canonical residuals remain expression DAGs.

`eqiora-compiler` is the L2 bridge. It resolves one model in declaration and
expression passes, checks SI exponent arithmetic, creates complete typed
kernel definitions, derives exact `DependsOn`/`HasPort` topology, and returns
an uncommitted `Transaction`. Only the caller may atomically commit it.

Compiler lowering, not parsing, resolves names, validates dimensions, and
mints or recovers persistent graph identities. Quantity conversion and exact
clock semantics follow the common language specification.

## Parser and formatter contract

- Every source byte belongs to one token, including whitespace, line comments,
  invalid source fragments, and EOF.
- Keywords are interpreted by the parser rather than hard-coded in the lexer.
- Diagnostics carry stable codes and UTF-8 byte spans.
- Recovery synchronizes at declarations and model boundaries so tools can use
  a partial AST.
- The formatter emits one style and formatting canonical output is idempotent.
- Expression grouping, equation meaning, and comment attachment follow the
  converged parser/formatter contract rather than a second grammar here.

## Alternatives considered

### Parser generator immediately

Useful once the grammar is broad, but it introduces a dependency and recovery
model before the semantic surface is stable. Deferred; the public AST and
diagnostics, not the parser implementation, are the contract.

### Parse directly into kernel nodes

Shorter initially, but conflates source recovery, name resolution, persistent
identity, graph transactions, and semantics. Rejected.

### Small recursive-descent/Pratt frontend — selected

Auditable, dependency-free, and sufficient to settle the first grammar. Pratt
precedence makes expressions explicit while preserving a clean later lowering
to the inspectable DAG.

## Verification

- Reconstruct input exactly from lexer tokens.
- Recover a later valid declaration after an invalid item.
- Parse continuous and periodic Relation declarations.
- Format, parse again, and prove canonical formatting is idempotent.
- Reject dimensionally invalid residuals with a source span.
- Lower admitted source through Transaction, `KernelProgram`, and the reference
  evaluator to the independently expected trajectory.

## Unresolved questions

- Stable source-anchor to graph-ID mapping for incremental recompilation.
- Block comments, documentation comments, and comment attachment.
- User-defined units and affine units.
- Modules, packages, imports, generic shapes, domains, and spatial operators.
