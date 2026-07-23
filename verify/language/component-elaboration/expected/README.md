# Acceptance

The registered integration target must satisfy all of the following:

- the canonical and permuted fixtures compile to one complete model with
  identical deterministic entity IDs, Model v2 canonical bytes, and digest;
- the two instances of `Resistor` have distinct IDs, while each instance keeps
  the same IDs across formatting, declaration-order, and file-name changes;
- the flattened model contains one nominal scalar physical Domain, three root
  Parameters, seven Ports, four Relations, four continuous Activations, and
  two Connections, with no component, instance, or occurrence-local Parameter
  kernel node;
- elaboration provenance associates expanded identities with definition,
  instance, and binding spans, and changing only source location changes that
  sidecar without changing model identity;
- the complete expanded hierarchy and the separately authored explicit flat
  fixture have byte-identical canonical Model v2 artifacts and equal semantic
  digests after one total, bijective ID normalization; Activation and
  Connection correspondences must be derived from graph structure rather than
  omitted from the comparison;
- the selected physical closure is a square 14-by-14 general CSR problem whose
  normalized structure agrees with the explicit flat fixture;
- faer BiCGSTAB with identity preconditioning and the registered all-ones
  initial vector reports a semantic residual no larger than `1.2e-11`;
- voltages and signed currents agree with the analytic solution within
  `2e-11`, both conserving junction sums vanish within the same bound, and the
  original Relation and generated junction DAGs reaccept the solution;
- missing, duplicate, unknown, private, or dimension-incompatible bindings,
  private member selection, nominal connector mismatch, and direct or
  indirect recursion fail before a graph Transaction is returned.

Passing this case does not imply package resolution, a distributed component
library, cross-boundary connection-set union, general physical hierarchy, or
nonlinear, transient, hybrid, distributed, or accelerated execution.
