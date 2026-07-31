# Canonical Cartesian Poisson CUDA handoff

The first canonical spatial CUDA solver gate uses the public `eqiora` facade
from source model through run provenance. The v2 execution evidence adds the
crate-private graph-bound adapter seam without re-exporting raw admission as a
curated facade API:

```text
canonical 2D Poisson revision
  -> exact Q1 FEM or cell-centered TPFA Realization
  -> portable typed graph + opaque finalized CSR / RHS / properties / SolverPlan
  -> pre-device-allocation device / QueueSlot binding
  -> graph-bound CUDA Jacobi-CG with Fast reductions
  -> seven typed transfers + three waited CUDA-event fences
  -> native serial-host true-residual acceptance
  -> independent serial-host receipt replay
  -> immutable complete-host-output receipt over one nine-step DAG
  -> method-native reconstruction
  -> analytic + balance + reference-CPU comparison
  -> current ModelEnvelope + RealizationEnvelopeV1 + RunManifestV2
```

The call site intersects three independently owned capability sources:

- the canonical lowerer admits exactly generated Cartesian 2D, Q1 FEM or
  cell-centered TPFA, replicated `f64`;
- the discovered device must admit `Float64`, CSR SpMV, dense level-1 vector
  actions, and an asynchronous queue; and
- the solver admits CG, identity or Jacobi, and `Fast` only.

No CUDA-specific solve function is added to numerics. The generic CUDA solver
does not know Cartesian, Q1, TPFA, or canonical lowerer scope. The finalized
problem retains the resolved CUDA target and independently rejects producer
topology, plan, shape, or residual contradictions before reconstruction.

## Physical numerical contract

Both methods use a fixed 16-by-16 mesh. The device result must satisfy:

- independently recomputed host residual under the exact `SolverPlan`;
- continuous L2 error below `2e-3` against
  `sin(pi x) sin(pi y)`;
- relative global balance below `2e-11`; and
- every method-native unknown and balance scalar within
  `2e-12 + 2e-12 * |reference|` of the same model/method executed by the
  reproducible reference CPU backend.

This is tolerance conformance, not an accidental CPU/CUDA bit-identity claim.
The CPU execution is an oracle only; a CUDA failure never falls back to it.

`CudaLinearSolveEvidence` supplies the selected device name, actual compute
capability, driver version, cuSPARSE/cuBLAS versions, cudarc version, and
binding-toolkit ABI. The physical test constructs `ExecutionTopologyV1::Cuda`
and `ExecutionProvenanceV1` from those observed values, then validates a
`RunManifestV2` against the exact model and Realization artifacts. No device or
library value is invented by the test.

## Evidence status

The machine-readable case is
[`numerics.canonical-cartesian-poisson-cuda`](../../verify/numerics/canonical-cartesian-poisson-cuda/case.toml).
The implementation test checks public feature shape and fail-closed admission
without device allocation; live execution remains ignored unless selected by
the operator.

The case is `verified` by a privacy-safe physical observation collected from
clean public source commit
`5696f62ed84eba5457e2ff99f40fd2080c808d69`. Replay pins that exact source
identity and rejects a substituted commit. The observation remains evidence of
one bounded run, not hardware attestation or a portable support claim.

Because compiler v0 intentionally mints fresh graph IDs, replay does not
pretend that two raw compiler outputs are byte-stable. A separate bounded
source-identity observation drives complete bijective alpha-renaming of named
declarations and relation activations. Every node/reference ULID must be mapped
exactly once before normalized Model bytes/digest, then Realization and Run
bytes/digests, are compared exactly.

The recorded bundle's Model v1 bytes remain immutable historical evidence.
The current runtime does not decode or relabel them. Replay verifies their raw
hash and recorded artifact digest, then uses the separately committed current
Model bridge for semantic comparison and for newly reconstructed Realization
and Run lineage. The bridge changes only the Model artifact epoch; the CUDA
solution, residual, balance, transfer, fence, and receipt oracles are unchanged.

The physical collector obtains each `WaitedCompletion` only by successfully
waiting a real CUDA event. Host replay cannot repeat that physical fact: it
uses bounded synthetic successful fences to reconstruct the typed trace, then
re-finalizes each method and validates the exact `QueueSlot` and materialized
`QueueId`, seven transfers, three fence sequences, exact `+1` solution
generation, operator/output fingerprints, and fixed nine-step receipt DAG.
The synthetic fences are structural witnesses, never re-attestation.

Replay reconstructs the recorded candidate through
the solver-native serial verifier, requires a second independent serial-host
residual replay in the receipt, finishes the method-native field, and repeats
analytic L2, balance, and same-model/method reference-CPU comparisons.

Run the physical gate with:

```bash
CUDA_VISIBLE_DEVICES=<physical-index> \
EQIORA_CUDA_DEVICE=0 \
cargo test -p eqiora --features cuda \
  --test canonical_cartesian_poisson_cuda \
  canonical_plane_poisson_runs_through_q1_and_tpfa_on_cuda \
  -- --ignored --exact
```

The receipt and graph-bound admission token are in-memory and non-durable; the
raw seam is not a curated facade or general public execution API. The claim
also excludes arbitrary initial values, free-memory reservation, persistent
residency, multiple streams/queues, GPU assembly, matrix-free kernels, pinned
transfers, memory pools, ILU/IC/AMG, reproducible device reductions,
FSI/MINRES, performance or scale, multiple GPUs, MPI plus CUDA, general PDE
finalization, checkpoint/restart, and real-time scheduling.
