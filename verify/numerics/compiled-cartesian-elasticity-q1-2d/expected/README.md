# Expected evidence

The exact rational local matrix, load, residual, forward actions, transpose
actions, scalar pairings, and both distinct patch checks are frozen by this
registered evidence case and encoded as literals in the independently owned
Rust oracle.

Centered binary64 finite differences use the exact dyadic step `2^-12`. Every
vector and scalar finite-difference comparison, including expected zeros, uses
one absolute max-norm tolerance of `1e-9`. No existing evidence tolerance is
changed.

The mutant corpus must independently reject the full-gradient substitution,
Lamé-term swaps/omissions/halving, component and scalar-node ordering faults,
reversed load gradient, corrupted transpose actions, parameter-coordinate
omission or permutation, stale or foreign certificate identity, ambiguous or
incomplete roles, and inadmissible quadrature or boundary configurations.

The exact two-point and complete-boundary restrictions apply to the
certificate-carrying derived form. The witness-only primal compiler retains
the pre-existing generic elasticity envelope, including body-free operation,
accepted scalar spatial loads, and compatible higher-exactness hypercube
quadrature, but must reject differential actions because it carries no Model
certificate.
