# Independent reference authorities

`analytic-derivation.md` and `derive_reference.py` are the two read-only,
independently authored scientific derivations frozen before this state-machine
oracle. The state/public evidence writer does not edit, tune, or relax either
file.

The Python route can be checked without writing the source tree:

```console
python3 verify/numerics/faer-sparse-lu-reuse/references/derive_reference.py --check
```

The complete cross-oracle and state-manifest check is owned by the adjacent
`run_case.py`; no third scientific value, tolerance, or discretization is
introduced by the state oracle.
