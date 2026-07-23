# Acceptance contract

The executable integration test owns the exact assertions. Acceptance
requires:

- the checked-in exact dependency Component containing only one volume support
  slot, two continuum Field slots, two scalar material Parameter slots, and
  one balance Relation;
- root ownership of the two Fields, four Parameters, the Cartesian body and
  four sides, the load definition, and four homogeneous boundary Relations;
- exact alias identity between each root Lamé Parameter and its Component
  binding occurrence;
- a complete identity bijection under which the packaged and existing
  explicit-flat Model v4 structures agree after relation-expression
  implementations are projected out;
- identical method-neutral bounds, Lamé coefficients, conservative load
  evaluation, Q1 algebraic solutions, componentwise equilibrium, and
  registered L2/H1 convergence;
- monotonically decreasing L2 and H1 errors on every registered refinement;
- a nonzero affine-potential solve with integrated body force `[1, 0]` and
  matching componentwise boundary reaction;
- invariant package identity and flattened Model bytes under dependency alias,
  declaration, binding, and input-file order;
- unchanged numerical acceptance after replacing the provider package name;
  and
- replayable exact package compilation, Model v4, Realization v1, Run v2, and
  package-execution-binding lineage.

No assertion may select a lowerer, Realization, solver, or expected numerical
value from a package or Component name. Cross-frontend residual-DAG byte
identity is intentionally not claimed by this case; structural comparison is
owned by [RFC 0073](../../../../rfcs/0073-structural-semantic-fingerprint.md).
