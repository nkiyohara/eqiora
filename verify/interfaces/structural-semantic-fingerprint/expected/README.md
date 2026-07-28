# Expected evidence

- Independent source compilation, source renaming/formatting/reordering, native
  Rust construction, and exact Model codecs v1 and v7 agree structurally.
- Every independent construction retains a distinct exact Model artifact
  reference; exact replay retains the original exact reference.
- The scalar and scalar-physical source/native pairs compare equal.
- Parameter value, residual operator, symbol rewiring, and nominal Domain
  sharing changes compare unequal.
- Dimension, spatial vector shape/frame, support edge, periodic-clock period,
  and Model boundary-membership changes compare unequal.
- `-0.0` and `0.0` compare equal, while a nonzero value does not.
- Exhausting the exact graph-labeling search budget returns `EQ0901` and no
  fingerprint.
- Independently allocated and reordered symmetric graphs choose the same exact
  canonical label under the default bounded policy.
- An exact value-edit child differs structurally from its stale base; no public
  API constructs a partial projection.
