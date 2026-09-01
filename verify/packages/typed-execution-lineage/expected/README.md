# Expected identities

`model.json` is the canonical Model fixture consumed by the package projection.
`historical-alpha1-compilation.json` is retained only for the typed compilation
decoder/accessor compatibility assertion that names that release.

The current Model fixture was recompiled when scalar sine moved from bare
`sin` to compiler-owned `math.sin`. That pre-1.0 source-vocabulary migration
changed the package namespace used to derive Model identities, while the
historical alpha1 compilation remains byte-for-byte unchanged.

`identities.json` freezes the package semantic and source domains, exact
resolution and compilation, canonical Model, typed Realization, Run v2, and
separate package execution binding. Any update requires identifying which
domain changed and why; replacing all values as one undifferentiated snapshot
would defeat the evidence.

RFC 0055 changed literal Component bindings from fabricated occurrence-local
Parameters to typed constants. The package and exact resolution identities
remain fixed, while the Model and every identity-derived downstream artifact
change together.
