# Model fixtures

`natural.eqi` is exactly 187 UTF-8 bytes including its final LF and has SHA-256
`0760b9592377f59e6a753f105bda9dac2020be2f3a592de7c1d31aa49b23fdbf`.

`explicit-residual.eqi` is exactly 199 UTF-8 bytes including its final LF and
has SHA-256
`a15140041e73e9bb245c9645d90652aaf35b547ee8086368c3ac2b0efbcf6b82`.

The natural fixture is compiled first. The separately compiled fixtures are
compared only for public structural equivalence and equal structural semantic
fingerprints. Fresh Model IDs, artifact references, canonical bytes, digests,
and provenance are deliberately not equality oracles.
