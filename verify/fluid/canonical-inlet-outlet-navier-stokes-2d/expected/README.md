# Acceptance contract

One canonical transient run must combine a nonzero essential velocity and a
constant-traction facet. Its realization must omit the pressure gauge row and
retain boundary-determined pressure. The all-essential companion must contain
and use exactly one zero-integral gauge.

An all-essential prescribed trace with nonzero net parent-outward flux must
fail through an injected assembly backend without invoking that backend. An
all-traction model must fail with the unresolved constant-velocity mode in its
diagnostic. Missing and overlapping Cartesian side conditions must fail
canonical lowering.

The pre-existing homogeneous registered test compares every accepted state and
the complete per-step evidence values exactly against the direct numerical
path; equality of outcome alone is insufficient.
