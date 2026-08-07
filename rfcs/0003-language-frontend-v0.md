# RFC 0003: Eqiora Language frontend v0

- Status: Draft implementation
- Authors: Eqiora contributors
- Created: 2026-07-17

## Summary

Eqiora Language v0 begins as a small declaration grammar for scalar fields,
parameters, typed Ports, exact periodic clocks, implicit Relations,
Connections, and model boundaries. A byte-lossless lexer, recovering parser,
source AST, and idempotent formatter form the frontend; semantic lowering is a
separate compiler-layer operation.

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

## V0 grammar sketch

```eqiora
model thermal {
  field temperature: K = 293;
  field command: 1 = 0;
  parameter tau: s = 10;
  port control_out: signal output 1;
  port control_in: signal input 1;

  clock control = periodic(period = 1 / 10, phase = 0 / 1);

  relation plant continuous {
    derivative(temperature) = control_in;
  }

  relation controller periodic(control) {
    control_out - next(command) = 0;
  }

  connect signal control_out -> control_in;
  boundary control_in, control_out;
}
```

Declaration values are coherent SI scalars. Dimension expressions use the SI
base symbols `kg`, `m`, `s`, `A`, `K`, `mol`, and `cd`, the dimensionless
literal `1`, multiplication, division, and integer powers. Exact clock
fractions denote seconds. Compiler lowering, not parsing, resolves names,
validates dimensions, and mints or recovers persistent graph identities.

## Parser and formatter contract

- Every source byte belongs to one token, including whitespace, line comments,
  invalid source fragments, and EOF.
- Keywords are interpreted by the parser rather than hard-coded in the lexer.
- Diagnostics carry stable codes and UTF-8 byte spans.
- Recovery synchronizes at declarations and model boundaries so tools can use
  a partial AST.
- Numeric literals must be finite `f64`; clocks use unsigned `u64/u64` syntax.
- A Relation statement may spell its ordered residual naturally as `lhs = rhs;`;
  it lowers directly to the existing `lhs - rhs` residual structure. The
  legacy exact signed numeric-zero form (`lhs = 0;`, including finite literals
  that round to binary64 zero) continues to retain `lhs` as the residual.
- The formatter emits one style and formatting canonical output is idempotent.
  A top-level subtraction formats as natural equality with each side in an
  independent expression context. Exact zero and negative-zero right operands
  use `(0)` and `(-0)` so reparsing cannot collide with the legacy sentinel.
- V0 comment tokens are lossless, but comment attachment to formatted AST
  nodes is deferred rather than guessed.

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
- Lower the thermal sampled-controller source through Transaction,
  `KernelProgram`, and the reference evaluator to the expected trajectory.

## Unresolved questions

- Stable source-anchor to graph-ID mapping for incremental recompilation.
- Block comments, documentation comments, and comment attachment.
- User-defined units and affine units.
- Modules, packages, imports, generic shapes, domains, and spatial operators.
