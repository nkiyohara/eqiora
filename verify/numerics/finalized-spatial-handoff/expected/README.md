# Acceptance

- FEM and FVM expose the same public finalized-problem contract.
- Independently solved and reconstructed results exactly equal the one-call CPU
  entry points.
- In this fixture, a same-shaped solution from the other finalized system
  fails the receiving system's independent residual under the shared target.
- Exact `SolverPlan` and producer-target mismatches are rejected before field
  reconstruction.
- A vector satisfying two systems would be admissible to both; origin identity
  is not claimed.
