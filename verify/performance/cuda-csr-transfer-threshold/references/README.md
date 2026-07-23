# Reference method

One independent `CsrMatrix::multiply` reference is computed before warm-up.
The performance comparison uses the allocation-free serial
`CsrMatrix::multiply_into` wall time and the outer CUDA adapter-call wall time
minus its separately recorded reference comparison. The CUDA value therefore
includes admission, index conversion, device discovery, context and stream
creation, allocation, descriptors, transfers, synchronization, version and
evidence construction, and resource teardown. The raw phase sum, reference
comparison, and full verified-call wall time remain separately replayable.

Timed CPU-first and CUDA-first calls alternate without an intervening host
matrix action. The timed host output and every CUDA output are independently
accepted against the same precomputed reference.

The protocol follows NVIDIA's guidance to include transfers in a
transfer-sensitive decision and to synchronize asynchronous device work before
using CPU wall-clock timing:

- [CUDA C++ Best Practices Guide](https://docs.nvidia.com/cuda/cuda-c-best-practices-guide/)
- [cuSPARSE Generic API](https://docs.nvidia.com/cuda/cusparse/)

`cuda_transfer_threshold_evidence` is deliberately host-only. After public
collection it parses the raw CSV independently of the collector, reconstructs
matrix and transfer sizes, requires all nine alternating repetitions, checks
every oracle field, recomputes medians, and rejects a forged durable-crossing
summary. Until then, the test is explicitly ignored and the case is not
registered as verified evidence.
