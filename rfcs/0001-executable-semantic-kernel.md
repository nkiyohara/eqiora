# RFC 0001: Executable Semantic Kernel v0

- Status: Draft implementation
- Authors: Eqiora contributors
- Created: 2026-07-17

## Summary

An executable Eqiora model is a `ModelView` selecting a validated immutable
snapshot whose Relations contain inspectable residual-expression DAGs and are
activated by explicit continuous, periodic, event, or guard semantics.

## Motivation

Typed IDs and graph topology alone cannot define model behavior. The current
store can represent that a Relation depends on a Field, but not the residual
equation, derivative reference, initial value, clock period, or activation
rule. A language or numerical backend built on that incomplete contract would
invent a second source of semantics.

## Proposed design

`eqiora-schema::kernel` owns typed definitions for all nine Semantic Kernel
node kinds. A closed expression DAG represents residuals:

```text
SymbolRef = field | derivative(field) | pre(field) | next(field)
          | parameter | port | time

ExprNode  = constant | symbol | neg | add | sub | mul | div | pow_integer
```

The state/algebraic/discrete distinction is not a top-level tuple. It is
derived from symbol use and Activation. Clock periods and phases are reduced
non-negative rational seconds so multi-rate coincidence is exact.

Semantic nodes enter the Graph Store only as complete typed `KernelNode`
definitions. The store type-erases IDs only after construction, preserves one
immutable snapshot for execution, and rejects dangling references atomically.

Meaning and numerical approximation remain separate:

- `KernelProgram::from_snapshot` defines structural, unit, clock, and dependency
  validity.
- `eqiora-sem` defines activation and simultaneous-update behavior.
- A reference stepper approximates those equations and must demonstrate
  convergence; its particular integration algorithm is not model meaning.

## Alternatives considered

### Rust closures

Closures are easy to evaluate but cannot be serialized, structurally diffed,
rendered as equations, lowered to AD/IR, or safely supplied by another process.
They are rejected for canonical semantics.

### Recursive expression trees

Trees are simple but duplicate common subexpressions and make stable node-level
diagnostics and later graph transformations awkward. They remain suitable as a
parser AST, not the canonical residual representation.

### Typed expression DAG — selected

The DAG is inspectable, deterministic, serializable, and operator-native. It
costs a small arena/index layer but best supports validation, AD, lowering,
source round-trip, and semantic diff.

## Compatibility and migration

This is a pre-alpha breaking change. Raw `AddNode` remains for non-semantic
Graph Federation records; Semantic Kernel nodes require `DefineKernelNode`.
Wire representation is not stable until schema fixtures and drift checks land.

## Verification

- Reject empty or cyclic/forward-referencing expression DAGs.
- Reject initial values whose dimensions differ from their Field definition.
- Reject zero-period ClockDomains.
- Reject raw creation of a Semantic Kernel node.
- Preserve transaction atomicity and snapshot isolation.
- Require exact agreement between Relation symbols and `DependsOn` edges.
- Require exactly one Activation per Relation and one periodic ClockDomain per
  periodic Activation.
- Accept causal signal fanout only with one output and one or more inputs.
- Execute a thermal plant plus sampled controller without special node kinds.

## Security, safety, and governance

Expression nodes are a closed vocabulary and never contain native function
pointers or executable source. Deserializers will enforce size/depth limits
before allocation. New expression primitives or kernel semantics require an
RFC and conformance evidence.

## Unresolved questions

- Exact tensor/shape vocabulary beyond scalar v0.
- Public schema generator after the Rust experimental types stabilize.
- Tensor/shape-aware structural analysis beyond square scalar systems.
- Total ordering of simultaneous external events beyond periodic clocks.
