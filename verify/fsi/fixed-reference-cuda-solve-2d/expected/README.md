# Expected evidence

`current-model-bridge.json` is the current canonical Model consumed by the
host replay of the retained CUDA observation. The historical Model in
`artifacts/model.json` remains intentionally rejected by the current decoder.

The registered selected-device observation comes from the first clean public
source commit. It is accepted only when all of these close together:

- CPU and CUDA Realizations finalize an exact common CSR/RHS fingerprint;
- the CUDA plan remains `f64`, symmetric-indefinite, identity-preconditioned
  MINRES with `Fast` reduction on one replicated device;
- the generic receipt records that same operator and output, the exact CUDA
  DAG, six transfers without a Jacobi diagonal, and three successful waits;
- an independent serial-host residual replay accepts the recorded vector;
- the unchanged FSI finish accepts its incompressibility, kinematics,
  interface action, and energy evidence;
- the independent CPU oracle agrees in the declared normalized tolerance; and
- the recorded historical Model, coupled Realization v3, and Run v2 bytes and
  digests remain linked and unchanged;
- a separately frozen current Model has the same independently derived
  generation-v2 structural fingerprint, while its newly derived Realization
  and Run lineage is not claimed as the recorded device observation.

The bounded device and library values identify one selected-device run. They
do not include host identity or private paths, and they do not establish a
portable hardware-compatibility claim.

Those closures are not all of one kind. The recorded coefficients, identities,
report, receipt, environment, physical finish, and historical canonical
artifact bytes are fixed by the pinned collection and must remain unchanged.
The current Model bridge is a separate semantic relation, never a relabelling.
The CPU-oracle
closure is different: its `conformance` figures are the expected values of a
comparison the replay performs live, re-solving the oracle from the current
tree, so a change to the reference solver moves them without any device,
operator, or acceptance rule changing. Refreshing them to the values that tree
now produces is a restatement of the same closure, not a relaxation of it: the
accepted bound stays the precommitted `2e-10 + 2e-10 max(|a|, |b|)`, it is
never refitted to a refreshed figure, and the decoder rejects any recorded
figure whose scaled error exceeds one.
