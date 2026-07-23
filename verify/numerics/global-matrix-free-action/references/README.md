# References

- [PETSc `MATSHELL`](https://petsc.org/release/manualpages/Mat/MATSHELL/)
  keeps a user-defined operator action distinct from explicit matrix storage
  and notes that matrix-dependent preconditioners require a separate contract.
- [MFEM assembly levels](https://mfem.org/howto/assembly_levels/) distinguish
  full, element, partial, and matrix-free operator representations.
- [MFEM partial assembly](https://mfem.org/performance/) describes essential
  row/column elimination through a constrained operator rather than by
  changing the underlying element action.

Eqiora does not adopt either library's types. These sources support the
separation among local numerical work, constraint projection, global action,
and optional explicit sparse storage.
