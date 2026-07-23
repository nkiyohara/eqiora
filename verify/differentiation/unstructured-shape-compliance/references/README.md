# References and independent oracles

The derivative oracle changes the canonical Domain declaration, recompiles it,
rebuilds the distorted vertex coordinates and identical connectivity, reruns
all mesh-quality gates, and solves the perturbed P1 system. It does not call the
analytic geometry action.

For the unit source, the integral of a P1 field on each physical triangle is
computed independently as triangle area times the arithmetic mean of its three
nodal values. This checks the objective lowering without reusing its quadrature
loop.

The continuous compliance oracle is the odd Fourier-sine series stated in the
problem description, truncated at mode 121 in both axes. It is used only to
check refinement toward the continuous functional, not as canonical model
meaning or as a derivative implementation.
