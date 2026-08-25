# Evidence boundary

The standalone `../oracle.py` independently freezes the complete ordinary DFG
v2 literal, its 511-byte length, domain-separated digest
`1811037532ef5697a2c331d47786d39b2a0d3a64b2f348e7859342e742fecca0`,
and the distinct plain JSON hash. Its literal, hand-written encoder, and stdlib
JSON encoder must agree before the Rust evidence consumes its output.

The registered Rust test derives all structural expectations from the closed
rectangle-minus-circle topology: one face, five edges, explicit retained and
created lineage, exact owner identity, and uniform-scale invariance of semantic
membership. It also replays each generated canonical v2 value and exercises
the named structural and exact-wire mutants. The frozen v2 literal is the exact
artifact boundary only for the ordinary scale-1 DFG witness; the scale family
does not claim one shared metric identity.
