# Acceptance

For one, two, and four MPI ranks with the same number of distinct physical
CUDA devices on one host:

- accepted reduced owner-row shards are the only rank-local matrix source;
- each process observes exactly one CUDA device at ordinal zero and all live
  physical UUIDs are distinct;
- MPI retains reproducible identity-preconditioned `f64` MINRES, halo,
  reductions, explicit-index gather, and every-rank host acceptance;
- each rank uploads its row offsets, column indices, and coefficients once;
- every local action advances dense input/output generations monotonically and
  retains successful waits for input, sparse action, and host-visible output;
- the complete operator agrees exactly with the CPU reference operator;
- the unchanged FSI finish accepts both paths; and
- normalized algebraic and physical values satisfy
  `|a-b| <= 2e-10 + 2e-10 max(|a|, |b|)`.

Different rank counts need not produce bit-identical solution values or equal
iteration counts. Missing launcher/device/runtime prerequisites and every
contract contradiction are explicit failures, not skipped evidence.
