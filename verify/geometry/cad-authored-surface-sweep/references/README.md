# Reference provenance

The expected values were derived independently before implementation by the
two routes recorded in `../expected/independent-oracles.md`. No production
output, fixture, or implementation source was an input to either route. The
tetrahedron quality definition is restated from the frozen public claim
already owned by `SimplicialMesh`, not read from source, and the pinned
`powf` evaluation is an x86-64 Linux/glibc observation recorded in that same
document; the acceptance gates do not depend on it.

## Exact-reapplication addendum

[`../expected/exact_reapplication.py`](../expected/exact_reapplication.py) is a
third, post-freeze route from the same independent evidence lane, added after
the freeze to settle the acceptance arithmetic and the lateral coverage
question. Its provenance is the same: the frozen public claim only. It imports
nothing from the repository, reads no production output, no fixture, and no
implementation source, and it was written without reading the implementation
Rust or the integration test. Standard library only, CPython 3.12, x86-64
Linux.

Every value it asserts is exact rational arithmetic over binary64 coordinates
reinterpreted with `Fraction.from_float`, so no asserted value depends on the
platform. The sizing predicate is taken both through `math.hypot` and through
an exact `2D² ≤ h²` test, and the two are required to agree, so no interval
count depends on libm bits either.

The one binary64 quantity that document reports — the naive `fl(det/6)` fold —
is an explicitly NON-GATING observation. It is deterministic on any IEEE-754
platform because only correctly rounded multiply, subtract, divide, and add are
involved, but it is order dependent, so it is recorded as an observation and
never as a frozen constant or an acceptance oracle. The script prints it and
asserts nothing about it. Cross-platform mesh-byte identity remains unclaimed.

The addendum changed no number and relaxed nothing. Where it found a row
unreachable in exact arithmetic — the lateral minimum determinants, ideal-real
references because `4/3` is not dyadic — it moved the realized dyadic value into
`minimum_determinant_exact_m3` and preserved the original number verbatim under
`minimum_determinant_ideal_real_m3`, renaming `minimum_quality_exact` to
`minimum_quality_ideal_real` on the same two witnesses for the same reason. That
is a naming correction, not an evidence change: a field named *exact* must not
hold a value the generated arithmetic never reaches.
