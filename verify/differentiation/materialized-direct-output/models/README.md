# Model authority

This package introduces no Model, Geometry, Mesh, Realization, Run, Result, or persisted model bytes. The registered Rust evidence constructs exactly one typed Model Parameter `p` and one scalar output from the precommitted generic relation `R(w,p) = A w - b p`, `J(w,p) = b^T w`.

The authoritative coefficient source, rational vectors, solver plan, bounds, and falsifiers remain in [`../references/README.md`](../references/README.md) and its bound sparse-LU fixture. This file exists only to satisfy the complete verification-package layout; it owns no value, tolerance, identity, schema, or Stokes claim.
