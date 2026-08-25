# Execution, diagnostics, and arrays

The ordinary numerical path is `Model + typed Mesh request + policies → Plan →
Run`. The `.eqi` Model remains mathematical authority; Python supplies only
admitted numerical choices and repeats no Domain bounds. The first common path resolves Q1 FEM or
cell-centred TPFA FVM with the closed reference linear solve, then executes the
Model- and Mesh-owning value through `eqiora.run(plan)`.

`eqiora.run(...)`, `submit(...).result()`, and `await submit(...)` share one
native lifecycle. Runs expose monotone state, coalesced progress, cooperative
cancellation at accepted boundaries, and one immutable result. Cancellation
never publishes a partial result.

Dense result arrays are rank-one CPU `float64` values in the current alpha.
`numpy(copy=False)` is lifetime-safe and read-only; an impossible zero-copy
request fails instead of copying silently. `numpy(copy=True)` returns an
independent writable allocation. DLPack exports are explicit CPU snapshots,
not aliases of immutable evidence.

Failures cross the Python boundary as typed `EqioraError` subclasses with
stable categories and structured diagnostics. Read the maintained
[execution and array contract](https://github.com/nkiyohara/eqiora/blob/main/docs/python/execution-and-arrays.md)
for exact states, ownership, cancellation behavior, and nonclaims.
