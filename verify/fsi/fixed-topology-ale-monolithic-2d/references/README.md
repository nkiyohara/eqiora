# Independent references

The checked-in direct Model is the semantic oracle: it contains the ordinary
conservative incompressible Navier--Stokes, small-strain elastodynamic, and
conserving mechanical-interface relations. ALE motion is absent from that
Model and enters only through the Realization selected by RFC 0064.

The numerical oracles are independent of accepted solver iterates:

- every analytic Jacobian column is reconstructed from deterministic,
  conservatively colored centered reassemblies of the complete nonlinear
  residual, including harmonic geometry motion;
- current coordinates are independently regenerated from the immutable
  reference coordinates and the sealed harmonic action;
- `dJ/dt - J div(w)` is recomputed per affine cell;
- fluid and solid interface actions are recovered from separate uneliminated
  residuals; and
- backward-Euler order is measured from `h`, `h/2`, and `h/4` solutions in
  one consistent P1 mass norm on the immutable solid reference topology.

The endpoint differential ALE/GCL formulation follows Förster, Wall, and
Ramm, <https://doi.org/10.1002/fld.1093>, and Fehn et al.,
<https://arxiv.org/abs/2003.07166>. These papers motivate the formulation;
only the executable Eqiora checks above admit the registered evidence.
