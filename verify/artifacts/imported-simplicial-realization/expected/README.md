# Acceptance

Canonical mesh and Realization bytes and digests round-trip exactly. The
Realization references the exact mesh digest and dimension admitted before
assembly. The single interior P1 degree of freedom equals `1 / 12`, total load
equals `1`, and boundary reaction equals `-1` to roundoff.

Capability, digest, dimension, resource-limit, unknown-field, or recomputed
quality-evidence mismatch must fail closed.
