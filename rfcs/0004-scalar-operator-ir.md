# RFC 0004: Scalar Operator IR and CPU conformance path

- Status: Draft implementation
- Authors: Eqiora contributors
- Created: 2026-07-17

## Summary

Canonical expression DAGs lower to a backend-independent scalar SSA Operator
IR with dense symbol slots. The first Rust CPU executor evaluates this IR under
the normative reference activation calendar and numerics, providing exact
trajectory conformance before optimized scheduling and sparse solvers exist.

## Layering

```text
Kernel expression DAG (meaning)
        ↓ lowering
Scalar Operator IR (dense symbol slots + SSA instructions)
        ↓ evaluation
Rust CPU residuals
        ↓ shared reference activation/numerical engine
Trajectory
```

`eqiora-ir` is L2 and depends only on core/schema. It cannot observe graph
mutation or execute a schedule. `eqiora-runtime` is L3 and owns the derived CPU
program. The reference interpreter exposes a narrow conformance hook for
expression backends; its normal `run` API always uses the canonical DAG.

## IR contract

- Symbols are deduplicated in first-use order and addressed by dense slots.
- Every canonical DAG node lowers to one topologically ordered SSA instruction.
- V0 instructions are constant, read, negation, add, subtract, multiply,
  divide, and integer power.
- Residual roots retain source order.
- Evaluation requires exactly one finite scalar per symbol slot and rejects
  non-finite intermediate values.
- Operator IR owns no units because dimensions were proven before lowering;
  symbol order retains typed `SymbolRef` identity.

## Why the scheduler is initially shared

Duplicating exact clock grouping, `Pre`/`Next` microsteps, Newton, and backward
Euler at the same time as introducing Operator IR would make a failed
trajectory ambiguous. V0 varies one layer: residual evaluation. Once this
conforms, a separate runtime scheduler and solver can be introduced and tested
against the same oracle.

This path is therefore not presented as optimized execution or a benchmark.

## Alternatives considered

### Treat the canonical DAG as Operator IR

Would avoid a type, but would provide no lowering boundary and no independent
backend evidence. Rejected.

### Lower directly to Rust closures

Fast to prototype but opaque, unserializable, and difficult to inspect or
target from non-Rust backends. Rejected.

### Dense scalar SSA — selected for v0

Small, deterministic, and sufficient to validate symbol binding and operation
lowering. Tensor, spatial, sparse, device, and memory operations extend the IR
through later RFCs rather than hidden callbacks.

## Verification

- Deduplicate a repeated Field symbol while preserving residual value.
- Reject wrong symbol-input cardinality and non-finite evaluation.
- Compile the thermal sampled-controller from Eqiora source.
- Compare every CPU trajectory sample exactly with the canonical DAG reference
  evaluator under the same run configuration.

## Unresolved questions

- Stable serialization and schema versioning for Operator IR.
- Tensor/shape types and structured memory views.
- Spatial local-operator, gather, and scatter instructions.
- Independent CPU scheduler, sparse nonlinear solve, and code generation.
