# Expected

- `classification.json` — the complete producer-semantic classification of
  every Model-bearing fixture found by the repository search, with the search
  scope and method that produced it, and, under `search.transition`, the exact
  two-state contract: the 44 retired paths, the 13 required post-reset targets
  — 11 carrying a frozen promotion digest and 2 required by existence alone —
  the 40 invariant preserved-evidence paths, and the 1 promoted-evidence pair
  whose bytes survive at a different path. Every path there is exact; there is
  no glob, suffix rule, or directory allowance.
- `classification.json` also declares, under `dispositions`, the seven fates an
  entry may assign, and gives exactly one of them to every path it names. The
  229 candidates no entry names inherit the remainder's `migrate-in-place`,
  which excludes retired paths by construction: all 34 retired inventory
  members are named explicitly, so no path the reset removes is described as
  migrating in place.
- `classification.json` also declares, under
  `search.forbidden_product_tokens`, the post-reset-only token contract: 3
  narrowly frozen product-source scopes and the 102 exact substrings the reset
  deletes from them. Those scopes are the only place a glob appears in this
  case, and they reach no verify fixture, crate test directory, RFC,
  documentation page, changelog, schema, or retained artifact byte.
  `pre_reset_occurrence` beside them records what this checkout actually
  carries — 98 of the 102, with `from_program_v2`, `from_json_v2`,
  `from_transaction_v2`, and `digest_v2` absent and forbidden prospectively.
- `classification.json` also declares two disjoint containment-only successor
  permissions. `post_reset_admitted` contains exactly 9 identity-free
  classified paths — the unchanged 5 existing rows plus the exact 4 RFC 0085
  RFC/source/test/derivation rows — each optional, absent before reset, exact in
  path and ordered signals, and fixed at zero Model-derived identity literals.
  `post_reset_fixture_admitted` contains exactly the 10 RFC 0085 expected paths,
  each optional and exact in path, ordered signals, same-line lower-hex-64
  literal occurrence count, fixture class, owner, and note. Multiple matching
  identities on one qualifying line are counted separately. Neither permission
  changes a historical inventory, transition, evidence, promotion, or required count;
  fixture admission joins no identity-free set and weakens no zero-identity
  predicate. There is no glob, directory, suffix, inferred sibling, or
  proximity admission.
- `classification-inventory.txt` — the sorted, exact 338-path output of that
  executable candidate sweep; it contains no glob or inferred path. The sweep
  excludes two exact files, declared in `search.excluded_paths`: this case's own
  integration-test root and the private support module it includes. Every other
  test file in the repository is an ordinary candidate.
- `transition.json` — the precommitted identities: for each deterministic
  fixture its current Model bytes, digest, ULID, source revision, superseded
  values, and every downstream reference edge; for each historical bundle the
  bridge fields.
- `deterministic/<fixture>/model.json` — complete current Model canonical
  bytes.
- `deterministic/<fixture>/<target>.json` — the complete replacement for the
  consumer fixture, byte-identical to the committed one outside its
  precommitted identity pointers.
- `deterministic/<fixture>/*.json` — the canonical bytes of each downstream
  artifact whose identity changes, so its digest and its Model edge are
  re-derivable without any producer.
- `deterministic/fixed-topology-ale-monolithic-3d/trajectory-segment-*.json`
  and `trajectory-root-0.json` — the intermediate canonical bytes that close
  both segment identities and `previous_root_sha256` before the final root.
- `bridge/<bundle>/current-model.json` — the current Model artifact built from
  the recorded bundle's decoded semantic program.
- `retained/realization-v4.json` — the exact 8,333-byte separately versioned
  golden, verified opaquely without a historical Model decoder.

Nothing here is generated during verification. These are the pre-committed
values the implementation wires; regenerating them is not permitted.
