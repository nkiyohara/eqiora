# Acceptance

No frozen decimal solution table is required. Tests require analytic solution
agreement, accepted true residuals under the declared plan, and fail-closed
capability behavior.

`sparse-lu-contract.json` is the one exception, and it belongs to a capability
this case does not yet claim. It is the frozen exact-rational fixture for the
`SparseLu` algorithm proposed in
[Issue #126](https://github.com/nkiyohara/eqiora/issues/126), committed ahead of
implementation by a non-implementing author.

It records integers and `{"num", "den"}` rational pairs only — no binary
floating-point value and no tolerance anywhere, including in the falsifiers.
Expected residuals are exact squared norms, so a consumer derives its own
tolerance rather than inheriting one.

`../oracle/sparse_lu_oracle.py` re-derives every recorded value from the stored
CSR arrays and fails on disagreement. See
[the oracle reference](../references/sparse-lu-oracle.md).
