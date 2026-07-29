# Expected values

The non-implementing oracle is [`../oracle.py`](../oracle.py), SHA-256
`0bdbbec6f9ff9c532ba5f30c856d1cd3b25e64949e4b11abf5fa3823e6a25742`. It freezes
every expected value for this case — the 50-segment selection, the 104-vertex
and 104-triangle reference topology, the high-precision sagitta, area-deficit
and perimeter-deficit values, the derived binary64 evaluation and area
allowances, and the five exact entity-set mappings — and reports 99 checks with
0 failures.

Those values are frozen ahead of implementation and are duplicated in
[`../case.toml`](../case.toml) for indexing only. No expected value is derived
from production output, and none may be tuned or relaxed by the implementing
lane: an implementer that believes a value is wrong returns the proof rather
than adjusting the value.

Two quantities are deliberately *not* expected values. The measured cell quality
is not an oracle and must only pass the supplied 1e-5 gate, and the mesh byte
layout is not fixed, so no cross-platform byte identity is expected here.
