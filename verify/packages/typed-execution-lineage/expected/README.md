# Expected identities

`identities.json` freezes the package semantic and source domains, exact
resolution and compilation, canonical Model, typed Realization, Run v2, and
separate package execution binding. Any update requires identifying which
domain changed and why; replacing all values as one undifferentiated snapshot
would defeat the evidence.

RFC 0055 changed literal Component bindings from fabricated occurrence-local
Parameters to typed constants. The package and exact resolution identities
remain fixed, while the Model and every identity-derived downstream artifact
change together.
