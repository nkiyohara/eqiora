# Frozen expected evidence

- `analytic.json` is the read-only exact-rational Oracle A.
- `symbolic.json` is the read-only independently constructed Oracle B.
- `state-machine.json` is the third, non-implementing state/public oracle. It
  freezes public API inventory, counter meanings, exact phase traces, identity
  relations, candidate commit and retention, attempt bounds, mutants,
  concurrency/storage boundaries, ordering behavior, and nonclaims.

`run_case.py --check` verifies the hashes of both scientific files and both
scientific derivation sources, reruns Oracle B deterministically, proves exact
scientific agreement, and validates the state-oracle invariants. It never
rewrites an expected file.
