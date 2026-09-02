# Expected boundary

`model.json` is the canonical Model fixture consumed by the package projection.
`historical-alpha1-compilation.json` is an immutable archival release artifact.
No current reader, decoder test, or expectation consumes it.

The current Model fixture was recompiled when scalar sine moved from bare
`sin` to compiler-owned `math.sin`. That pre-1.0 source-vocabulary migration
changed the package namespace used to derive Model identities, while the
historical alpha1 compilation remains byte-for-byte unchanged.
