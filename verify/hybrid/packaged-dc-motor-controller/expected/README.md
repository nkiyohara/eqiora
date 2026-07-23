# Frozen identity lineage

`identities.json` freezes the exact package closure and the accepted
Model-to-Run lineage. RFC 0055 changed literal Component bindings from
fabricated occurrence-local Parameters to typed constants, so the Model,
Compilation, Run, and package/Run binding identities changed together. The
three package semantic/source identities and the exact resolution identity did
not change.
