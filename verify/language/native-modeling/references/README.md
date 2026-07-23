# References

The exact structural oracle is the typed kernel produced by the existing
source compiler after replacing fresh symbol IDs with declaration aliases. The
reference-execution oracle is backward Euler for `dx/dt + x = 0`, giving
`x_n = (1 + dt)^(-n)` for the selected fixed step.

This deliberately avoids treating either source bytes or native builder
objects as canonical persisted meaning; the accepted model artifact remains
the durable boundary.
