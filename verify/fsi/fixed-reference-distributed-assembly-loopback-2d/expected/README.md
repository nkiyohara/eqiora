# Acceptance

For one, two, and four logical partitions:

- every cell has one producer and is evaluated exactly once;
- the packet set is content-bound to the authenticated mesh revision before
  any packet is evaluated;
- derived owned/ghost vertices, facets, process boundaries, and exchanges are
  complete for the exact mesh identity;
- every active reduced/full row has one equation-support-derived owner;
- unordered route delivery is admitted only after exact route inventory and
  payload validation, then folded in global packet order;
- reconstructed reduced and full CSR indices, matrix bits, and RHS bits equal
  independent complete CPU reference assembly;
- the reduced canonical CSR fingerprint equals the reference fingerprint; and
- serial-host MINRES passes the unchanged coupled FSI acceptance checks.

The contract tests independently reject invalid cell claims, a same-cell-count
foreign mesh, unsupported or nonminimal collective row owners, missing or
duplicate producer admissions, route loss/duplication/substitution, payload
drift, and order-sensitive reduction. A target-local empty owner shard remains
valid and reconstructs without weakening unique global row ownership.
