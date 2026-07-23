# Reference provenance

The zero-initial inflow-step spectral solution, Robin eigenvalue condition,
modal coefficients, decay rates, and reverse-flow reflection are derived in
[`models/problem.md`](../models/problem.md). No external numerical table or
third-party dataset is used as an oracle.

The canonical/Realization boundary and claim exclusions are fixed by
[RFC 0069](../../../../rfcs/0069-conservative-cell-centered-transport.md).
That RFC also records the primary finite-volume references for owner/neighbor
face sums, first-order upstream donor selection, and the primary van Leer and
Sweby MUSCL/TVD sources used by the Cartesian minmod profile. Those sources justify the
method contract; only this repository's executable checks can admit the
registered Eqiora capability.
