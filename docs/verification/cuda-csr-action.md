# CUDA CSR action verification

The first device conformance case proves one deliberately narrow path:

```text
validated Eqiora CsrMatrix
    -> checked signed-64 CUDA indices
    -> explicit matrix/vector H2D plans
    -> cuSPARSE Generic API CSR SpMV
    -> explicit D2H plan
    -> independent host CsrMatrix action
    -> tolerance-gated evidence
```

It does not lower new model meaning and does not introduce a CUDA matrix into
the Semantic Model or stable wire. cudarc owns the selected context, one
nonblocking stream, and typed allocations. A private adapter module loads only
the cuSPARSE 12 Generic API functions it executes; the library, handle,
matrix/vector descriptors, and queried external workspace remain alive until
the stream is synchronized.

## Falsifying fixture

The hardware test assembles the SPD matrix and action

```text
[ 4 -1 ] [1] = [2]
[-1  3 ] [2]   [5]
```

through the ordinary L2 assembly contract. It requires an explicit physical
device selection, executes deterministic CSR SpMV, and checks:

- exact returned values for this integer-exact fixture;
- runtime/device identity and live driver/cuSPARSE versions;
- matrix, input, and output transfer byte counts;
- input completion before action completion before output completion;
- retained workspace size;
- explicit host-oracle absolute/relative tolerance;
- setup, H2D, action, D2H, verification, and total wall times.

Invalid input shape is tested without loading CUDA, so non-hardware CI also
proves failure occurs before runtime discovery or allocation.

## Running the physical gate

The test is ignored by default because ordinary CI runners must not imply
hardware support. On a machine with compatible NVIDIA driver and cuSPARSE 12:

```bash
CUDA_VISIBLE_DEVICES=<physical-index> \
EQIORA_CUDA_DEVICE=0 \
cargo test -p eqiora-backend-cuda --features cuda-runtime \
  --test contract_boundary \
  cuda_runtime::physical_cuda_csr_action_matches_the_host_oracle \
  -- --ignored --exact
```

After `CUDA_VISIBLE_DEVICES` narrows visibility, Eqiora ordinal zero refers to
that selected device. Absence or incompatibility is a typed capability
failure; the test never falls back to CPU and relabels the result as CUDA.

## Nonclaims and related solver gate

This action evidence alone does not claim a sparse solve, GPU assembly,
matrix-free execution, cross-device bit identity, pinned transfers, multi-GPU
execution, CUDA graphs, or real-time scheduling. The separately falsifiable
[CUDA Krylov verification](cuda-krylov-solvers.md) keeps this same
CSR/queue/transfer seam and proves the first CG/BiCGSTAB solver slice.
