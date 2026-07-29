# Acceptance

No frozen decimal solution table is required. Tests require analytic solution
agreement, accepted true residuals under the declared plan, and fail-closed
capability behavior.

`sparse-lu-contract.json` is the one exception. It is the frozen exact-rational
fixture consumed by this case for the `SparseLu` algorithm introduced through
[Issue #126](https://github.com/nkiyohara/eqiora/issues/126), committed ahead of
implementation by a non-implementing author.

It records integers and `{"num", "den"}` rational pairs only — no binary
floating-point value anywhere, including in the falsifiers. Expected residuals
are exact squared norms.

It also binds one acceptance threshold: relative tolerance exactly `0`, absolute
tolerance exactly `2^-30`, `maximum_iterations` exactly `1`, and a componentwise
solution-error ceiling of `2^-28`. Tolerances use the same rational encoding and
are dyadic, so every acceptance comparison stays decidable in exact arithmetic
and a consumer compares against exactly the number that was proved about. The
threshold is a choice made by the non-implementing author; what the oracle proves
is that it clears binary64 rounding and rejects every frozen wrong route.

`../oracle/sparse_lu_oracle.py` re-derives every recorded value from the stored
CSR arrays and fails on disagreement. See
[the oracle reference](../references/sparse-lu-oracle.md).
