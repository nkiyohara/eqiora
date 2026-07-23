# Reference provenance

The expected solution follows directly from Ohm's law and Kirchhoff current
balance. The two parallel branches draw `12 / 2 = 6 A` and `12 / 4 = 3 A`.
Consequently, the ideal source supplies `9 A`, the high junction is at `12 V`,
and the explicitly grounded junction is at `0 V`.

No external solver output or numerical table is used as reference data. The
source/native paths construct the exact analytic vector in their independent
canonical unknown orders, submit it as an initial-residual witness, and
evaluate both the captured CSR action and original relation/generated-junction
DAGs. Solver identity, algorithm, reduction policy, and serial execution
provenance come from the typed solve reports. Convergence from an arbitrary
initial guess is deliberately left to controlled solver evidence.
