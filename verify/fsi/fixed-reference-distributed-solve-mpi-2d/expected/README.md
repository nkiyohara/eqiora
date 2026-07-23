# Acceptance

At one, two, and four physical MPI ranks on one host:

- the exact accepted RFC 0060 assembly receipt retains both reduced and full
  target identities;
- accepted reduced owner-row payloads are the sole source of rank-local CSR
  and RHS storage; the distinct complete CSR is only a verifier;
- solver row ownership exactly equals the accepted reduced-target ownership;
- every solver ghost and halo transfer is derived from an off-owner shard
  column;
- the complete reduced identity equals the transport-independent CPU path,
  while the full target remains a pressure-row-continuity lineage witness;
- MPI admits only symmetric-indefinite, identity-preconditioned,
  reproducible `f64` MINRES;
- explicit global owner indices and values reconstruct one complete candidate
  with no missing or duplicate entry;
- every rank independently reapplies the complete CSR, accepts the true
  residual, and agrees exact output plus the domain-separated execution
  summary;
- the unchanged FSI finish accepts residual, incompressibility, kinematics,
  interface velocity/action, pressure closure, and energy balance; and
- CPU/MPI dimensionless algebraic coefficients and exact-support physical
  Fields normalized as velocity/`U`, pressure/`P`, and displacement/`L`
  satisfy `|a-b| <= 2e-10 + 2e-10 max(|a|, |b|)`; and
- all ranks agree a domain-separated summary of observed MPI
  implementation/library, standard, `mpi-rs`, thread support, topology, and
  reduction provenance.

The tolerance applies after each physical Field is divided by its exact
Realization scale. No dimensionless absolute tolerance is applied directly to
mixed SI quantities. Solution bits and iteration counts need not match across
different rank counts; Model/operator meaning and the bounded physical result
must match.

A one-rank forged solver-owner authority must yield a common synchronized
diagnostic without a hang, and the subsequent authoritative solve must pass.
Binder drift is covered directly. Gather/reacceptance and post-admission
MINRES local-action faults remain RFC 0058 generic MPI prerequisites; mesh and
route faults remain RFC 0060 prerequisites. They are not overclaimed as direct
injections in this case.
