# Acceptance criteria

- Explicit Model v1, v2, and v3 artifacts each produce a sealed reference
  containing their exact digest, typed Model identity, and semantic revision.
- Every reference constructs the unchanged Realization v1 wire and a coherent
  Run v2 content-link chain.
- Explicit selected-wire decode preserves the exact reference accepted by the
  Realization.
- Equal Model identity and revision in a different Model wire digest domain
  fail closed as an artifact substitution.
- No wire auto-detection, schema upgrade, execution, numerical acceptance, or
  physical-result claim follows from the identity chain.
