# Expected observations

Completed, cancelled, and failed runs end in one typed terminal state. Only a
completed run materializes a Result, and repeated access returns that same
object. Accepted cancellation publishes exact boundary evidence and raises
`CancellationError` with `EQ0506`; execution failure retains its original
diagnostic. Progress storage and lifecycle history remain bounded.
