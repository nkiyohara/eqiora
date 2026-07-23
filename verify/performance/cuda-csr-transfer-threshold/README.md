# Cold one-shot CUDA CSR transfer threshold

This case asks one deliberately narrow performance question: on one recorded
host/GPU/driver/library environment, at which measured 2D five-point problem
size, if any, does one nonresident CUDA CSR action become faster than the
serial host action after including setup, allocation, index conversion, all
host-to-device transfers, cuSPARSE SpMV, synchronization, and the output
device-to-host transfer?

The protocol is fixed before collecting observations:

- refinements are `64, 128, 256, 512, 1024, 2048, 4096`, with `n^2` rows and
  `5 n^2 - 4 n` sorted CSR nonzeros;
- the process and input matrix are warm, but every CUDA sample constructs
  fresh adapter resources, descriptors, workspace, allocations, and transfers;
- the host action reuses its output allocation;
- one independent serial reference is computed before warm-up; both timed
  actions are accepted against it only after their respective measurements;
- two warm-ups precede nine recorded repetitions at each size;
- timed CPU-first and CUDA-first order alternates by repetition;
- every CUDA sample must pass the independent host oracle under the recorded
  tolerance;
- medians are recomputed from raw repetitions; and
- a durable crossing is the first size at which the CUDA median is lower and
  remains lower for every larger measured size, with at least one larger size.

The collector and independent replay are implemented. The case remains
`implemented`, rather than `verified`, until it is collected from a clean
public source commit and the resulting observation is registered. This avoids
presenting an observation whose source provenance cannot be resolved in the
public repository.

## Running the collector

Run from a clean commit and restrict CUDA visibility to one device. The output
path must not exist; the collector publishes it by one directory rename only
after every sample succeeds.

```bash
CUDA_VISIBLE_DEVICES=<physical-index> \
taskset -c <one-cpu> \
cargo run --release -p eqiora-backend-cuda \
  --features cuda-runtime \
  --example cuda_csr_transfer_threshold -- <new-output-directory>
```

## Environment contract and nonclaims

The environment observation binds the clean source commit and records the
compiler, operating-system release, CPU model, affinity count, frequency
policy, CUDA library versions, device model and memory, numeric system load,
numeric GPU operating counters, and compute-process count. It deliberately
does not persist a hostname, device selector, PCI address, GPU UUID, process
identifier, process name, or raw command output.

After public collection, the replay test must accept the raw repetitions and
environment before the case can return to `verified`. Any resulting threshold
is local to that CPU, GPU, interconnect, driver, library, affinity, and
power/thermal state. The case does not measure resident or amortized execution,
pinned memory, memory pools, preprocessing, multiple GPUs, solver convergence,
energy, or reproducibility across devices. It never selects a production
`Realization` backend and does not establish a universal threshold.
