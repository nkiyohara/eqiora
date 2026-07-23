# Expected observations

- Equal requests previewed against one exact Model produce equal Realization
  bytes and digests without assembling or solving a numerical system.
- FEM and FVM requests produce distinct accepted Realization identities and
  execute the same bounded one-dimensional constant-source Poisson meaning.
- Field location/count/range, continuous balance, and independently verified
  linear residual summaries satisfy the existing scalar-elliptic acceptance
  rules.
- FEM exposes all values in canonical vertex order, including essential
  endpoints; FVM exposes primary cell values in canonical cell order rather
  than reconstruction vertices. The immutable zero-copy NumPy projection
  survives destruction of its originating Result.
- Asymmetric affine Fields in one through three dimensions follow the declared
  logical shape and canonical row-major Cartesian order, with the last physical
  axis varying fastest.
- The Run v2 manifest names the exact Model, semantic revision, Realization,
  host-serial topology, adapter/backend, and reproducible reduction policy.
- Exact-profile replay reproduces Run identity; foreign or tampered Model and
  Realization linkage fails before result admission.
- Blocking, submitted, and awaitable scalar-elliptic execution share one
  native lifecycle. Progress follows the exact plan-replayed, system-finalized,
  solution-accepted order. Typed cancellation raises `EQ0506`, is idempotent,
  and publishes no partial Result; the linear solve remains atomic.
- The algebraic output fingerprint is observable, but it is not presented as a
  digest of the semantic Field array. The Run output digest set is empty because
  no durable result Artifact exists in this slice.
