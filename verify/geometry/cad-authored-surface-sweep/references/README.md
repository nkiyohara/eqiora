# Reference provenance

The expected values were derived independently before implementation by the
two routes recorded in `../expected/independent-oracles.md`. No production
output, fixture, or implementation source was an input to either route. The
tetrahedron quality definition is restated from the frozen public claim
already owned by `SimplicialMesh`, not read from source, and the pinned
`powf` evaluation is an x86-64 Linux/glibc observation recorded in that same
document; the acceptance gates do not depend on it.
