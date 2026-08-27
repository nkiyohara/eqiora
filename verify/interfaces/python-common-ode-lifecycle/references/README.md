# Independent reference

The Model is the initial-value problem

`dx/dt + x / s = 0`, `x(0) = 1`.

Separating variables gives `x(t) = exp(-t / s)`. The registered test evaluates
this expression independently with Python's `math.exp`; it does not obtain
expected values from Eqiora output. The separately declared comparison boundary
uses a conservative factor of 20 over the requested adaptive relative and
absolute controls; it is an acceptance falsifier, not an adaptive global-error
bound.
