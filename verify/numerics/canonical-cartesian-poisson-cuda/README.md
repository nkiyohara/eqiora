# Canonical Cartesian Poisson CUDA handoff

This case fixes one canonical two-dimensional Poisson revision and realizes it
independently as continuous Q1 FEM and orthogonal cell-centered TPFA. Both
paths stop at the same opaque finalized algebraic handoff. The optional CUDA
adapter consumes only its CSR, right-hand side, asserted properties, and sole
`SolverPlan`; the numerical layer has no CUDA-specific solve entry point.

For this bounded slice, the equation-aware finalizer also regenerates the
portable typed Realization DAG. A deployment binding fixes one device and one
logical queue slot and admits the exact CUDA CG/Jacobi/`Fast` tuple before any
device allocation. The resulting run is graph-bound, uses the admitted
implicit-zero initial state, and yields a complete host-visible output plus an
immutable in-memory execution receipt.

The implementation covers:

- exact lowerer/runtime/solver capability intersection at the call site;
- pre-device-allocation binding to one selected device and one logical queue
  slot;
- `Fast` Jacobi-CG with no CPU fallback;
- a typed transfer trace and successfully waited CUDA fences;
- a fixed execution DAG from input transfer through complete host-output
  acceptance;
- solver-native serial-host true-residual verification and a separate
  serial-host residual replay;
- method-native L2 error and global balance; and
- comparison with the same model and method on the reproducible reference CPU.

The public-alpha tree carries a privacy-bounded physical observation collected
from clean public source commit
`5696f62ed84eba5457e2ff99f40fd2080c808d69`. Portable host replay pins that
exact commit, reconstructs both method paths, and accepts the recorded
selected-device execution. The case is therefore `verified` for this bounded
observation, without turning one device run into a general CUDA support claim.

The recorded Model, Realization, and Run remain one immutable historical
bundle: the Model uses the retired v1 schema and the downstream artifacts keep
its historical digest. The current decoder never reads or relabels those Model
bytes. A separately precommitted current Model bridge has the same independently
derived generation-v2 structural fingerprint and supplies the semantic program
for host reconstruction. Current Realization and Run lineage is derived
separately and deliberately differs from the recorded lineage; no current Run
is claimed as a device observation.

The collector writes a new directory atomically. Its environment schema records
only the clean source commit, release profile, non-identifying runtime context,
the single Eqiora device ordinal, device name and capabilities, memory size,
CUDA library versions, numeric system load, and counts of other compute
processes. It never persists the host name, the raw device selector, PCI or
UUID identity, process identifiers, or process text.

Collect from a clean public source commit on a compatible CUDA 12 system:

```text
CUDA_VISIBLE_DEVICES=<physical-index> \
cargo run --release -p eqiora --features cuda \
  --example canonical_cartesian_poisson_cuda_collect -- <new-output-directory>
```

After visibility is narrowed, Eqiora ordinal zero names the selected device.
The collector refuses a dirty source tree, an existing output path, more than
one visible selector, or another compute process on the selected device. It
stages canonical Model, Realization, and Run bytes with bounded
solution/environment observations, then publishes the complete directory by
one rename.

For any future replacement of the registered observation:

1. confirm that `observations/environment.json` names the exact clean public
   commit used to build the collector;
2. review the directory with the public-release hygiene checker;
3. update the replay's registered-source constant to that exact commit;
4. retain only the capabilities proved by the replacement observation; and
5. run the portable replay and the full verification boundary.

The physical collector obtains each fence record only from a successful wait
on a real CUDA event. Ordinary host replay cannot repeat that physical fact: it
reconstructs a bounded synthetic successful fence solely to replay the typed
acceptance contract, then checks the recorded queue order, transfers, waited
fence sequences, solution generations, fingerprints, and receipt DAG. The
observation is evidence of one selected run, not hardware attestation or a
performance claim.

Compiler v0 deliberately mints fresh graph IDs on each source compile. The
collector records the raw compiler model identity and the complete lexically
ordered source-name-to-typed-ID map separately from the canonical Model
artifact. Replay constructs a complete bijective alpha-renaming before it
compares canonical Model bytes and digests.

The receipt and graph-bound admission token remain in-memory and non-durable.
The raw graph execution seam is not a curated facade or general public
execution API. This slice also does not claim arbitrary initial guesses,
free-memory reservation, GPU assembly, matrix-free kernels, ILU/IC/AMG,
reproducible CUDA reductions, pinned transfers, memory pooling, multiple
queues, scale, multi-GPU or MPI+CUDA execution, FSI/MINRES CUDA, general PDE
finalization, or real-time scheduling.
