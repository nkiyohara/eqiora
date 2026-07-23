# Independent references

The checked-in direct Model is the semantic oracle. It contains the ordinary
conservative incompressible Navier--Stokes, small-strain elastodynamic, and
conserving mechanical-interface Relations. Mesh revision and transfer policy
remain Realization concerns and do not add remeshing nodes to that Model.

The numerical oracle is a deterministic geometric common refinement. It
intersects independently indexed affine source and target cells in the
declared material or current-spatial chart, integrates each positive fragment,
and checks bidirectional coverage. Target projection residuals and conserved
functionals are reassembled independently of solver convergence reports.

The transfer ordering and chart distinction follow the formulation fixed by
RFC 0065. Only the executable checks in this directory admit the registered
claim; citations in the RFC motivate the method but are not acceptance
evidence.
