# References

- [PETSc Stokes finite-element guide](https://petsc.org/release/tutorials/physics/guide_to_stokes/)
  records the traction weak-boundary term and pressure-nullspace distinction.
- [DOLFINx Stokes demo](https://docs.fenicsproject.org/dolfinx/main/python/demos/demo_stokes.html)
  provides a current independent mixed finite-element formulation.
- [Fabricius, Stokes flow with mixed Dirichlet and pressure boundary
  conditions](https://arxiv.org/abs/1702.03155) gives the continuous uniqueness
  setting for mixed velocity/traction data.

These references inform sign and nullspace checks. The committed fixture,
exact algebra, and independently recomputed resultants are the executable
oracle.
