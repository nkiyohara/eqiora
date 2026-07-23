# Expected evidence

`identities.json` freezes the semantic and source digests of the leaf,
intermediate, and root packages together with the exact resolution, canonical
Model, and package-compilation digests.

The integration target reconstructs this object from compiler-derived
releases after installation and locked replay. Any change requires an explicit
review of the semantic, author-source, resolution, Model, and compilation
digest domains rather than a mechanical fixture refresh.

RFC 0055 changed only elaboration output for this frozen source: literal
Component Parameter terms now specialize to constants instead of allocating
six occurrence-local Parameter nodes. Package semantic/source identities and
the resolution digest therefore remain fixed, while the canonical Model and
compilation digests intentionally move to the smaller exact Relation network.
