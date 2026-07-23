# Reference provenance

The oracle independently performs complete ordered CPU assembly, reference
MINRES, and the ordinary FSI finish. The candidate uses accepted MPI
owner-routed assembly, the same complete operator identity, host-owned MPI
MINRES, and CUDA only for rank-local sparse actions.

Reduced and full CSR/RHS systems agree bit-for-bit before execution. After
execution, dimensionless algebraic coefficients and exact-support physical
Fields divided by their Realization scales agree within `2e-10` absolute and
relative tolerance. Both paths independently pass host residual and physical
FSI acceptance.
