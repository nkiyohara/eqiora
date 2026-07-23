# Model fixtures

`decay.eqi` is the scalar implicit-ODE comparison fixture. `resistor.eqi` is
the scalar-physical fixture that exercises nominal Domain identity, Ports,
multi-residual Relations, and an N-ary conserving Connection. The integration
test builds equivalent native drafts independently and constructs all
falsifiers in memory so no derived artifact is mistaken for source truth.
