# Expected evidence after public recollection

No physical observation is registered in the initial public tree. The first
clean public commit is the only valid source for recollection. The resulting
selected-device observation is accepted only when all of these close
together:

- CPU and CUDA Realizations finalize an exact common CSR/RHS fingerprint;
- the CUDA plan remains `f64`, symmetric-indefinite, identity-preconditioned
  MINRES with `Fast` reduction on one replicated device;
- the generic receipt records that same operator and output, the exact CUDA
  DAG, six transfers without a Jacobi diagonal, and three successful waits;
- an independent serial-host residual replay accepts the recorded vector;
- the unchanged FSI finish accepts its incompressibility, kinematics,
  interface action, and energy evidence;
- the independent CPU oracle agrees in the declared normalized tolerance; and
- canonical Model, coupled Realization v3, and Run v2 bytes and digests replay.

The bounded device and library values identify one selected-device run. They
do not include host identity or private paths, and they do not establish a
portable hardware-compatibility claim.
