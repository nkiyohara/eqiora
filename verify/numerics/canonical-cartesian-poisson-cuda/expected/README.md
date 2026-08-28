# Expected evidence

`current-model-bridge.json` is the current canonical Model consumed by the
host replay of the retained CUDA observation. The historical Model in
`artifacts/model.json` remains intentionally rejected by the current decoder.

This directory specifies and accompanies the committed public-source physical
observation. Portable replay pins its exact clean source commit.

The collector must emit canonical Model, Q1/TPFA Realization, and Q1/TPFA Run
bytes under `artifacts/`, plus bounded environment, source identity, accepted
algebraic values, convergence reports, deployment and queue identity, typed
transfers, waited completion sequences, solution generations, execution
fingerprints, the receipt DAG, timings, method metrics, and CPU comparison
under `observations/`.

`environment.json` must use the privacy-safe environment v2 schema. It records
the exact clean public source commit and bounded non-identifying execution
facts. Host names, raw device selectors, PCI or UUID identity, process
identifiers, and process text are forbidden.

The replay contract separates compiler-v0 fresh IDs from stable evidence
identity. `source-identity.json` binds lexical source declarations to the raw
collected Model and supplies the target of a complete, bijective
alpha-renaming. The recorded Model, Realization, and Run retain their exact
historical bytes and digests. A separately frozen current Model is normalized
to the same named program and must match the historical Model's independently
recorded generation-v2 structural fingerprint; incomplete or surplus
correspondences fail closed. Newly reconstructed current Realization and Run
lineage must not be presented as the recorded device artifacts.

The physical collector records input-ready, solve-visible, and
solution-visible completion only after successful waits on actual CUDA events.
Ordinary host replay does not re-attest those events. It reconstructs bounded
synthetic successful fences, then re-finalizes both methods and checks the
one-device/one-queue deployment, transfers, recorded fence sequences, solve
generation, execution fingerprints, complete host output, and receipt DAG. It
reconstructs accepted `LinearSolution` values through the solver-native serial
verifier and checks a second independent serial-host residual replay before
method-native finish, L2, balance, and reference-CPU comparisons.

Mutated values, artifacts, deployment identity, transfer shape or order,
completion order, generations, fingerprints, DAG steps, duplicate or unknown
fields, malformed bytes, and incomplete, duplicate, or unused identity
correspondences are negative evidence. The graph-bound seam is not claimed as
a curated facade or general public execution API.
