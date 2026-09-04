# Execution, diagnostics, and arrays

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
stable categories and structured diagnostics. The maintained
[execution and array guide](https://github.com/nkiyohara/eqiora/blob/main/docs/python/execution-and-arrays.md)
has complete lifecycle, ownership, cancellation, NumPy, and DLPack details.
