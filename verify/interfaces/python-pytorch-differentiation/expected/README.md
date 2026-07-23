# Expected invariants

The installed wheel must produce the same complete primary Field and
first-order input cotangent as the accepted native evaluation at the same
Parameter point. `opcheck`, `gradcheck`, and full-graph compilation must pass
without tracing into native solver internals or silently graph-breaking.

No operator may mutate or alias an input, perform a device transfer, cache an
accepted evaluation as mutable hidden state, or import PyTorch from the base
`eqiora` module. Unsupported Tensor metadata and stale process-local tokens
must fail before numerical execution.
