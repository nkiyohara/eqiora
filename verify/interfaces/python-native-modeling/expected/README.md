# Expected evidence

- The Python scalar artifact replays exactly and executes the three accepted
  backward-Euler samples for `dx/dt + x = 0`.
- The Python physical artifact replays exactly with one Domain, three Ports, one
  two-residual Relation, and one conserving Connection.
- The Python spatial artifact replays exactly, matches the source Poisson
  structural fingerprint, and reaches the existing one-dimensional FEM path.
- Independently authored source/Python scalar, scalar-physical, and spatial
  pairs have distinct exact digests and equal generation-v1 structural
  fingerprints.
- Foreign same-named symbols, nominal Domain substitution, omitted Connection
  members, spatial scope/parent and Representation substitution, dimensional
  mismatch, and invalid trace support fail through shared Rust diagnostics.
- No rejected `Model.define(...)` call returns a partial Model.
- Independent source/Python exact artifact equality is false by design.
