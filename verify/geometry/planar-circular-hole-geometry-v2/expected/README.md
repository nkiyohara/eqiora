# Evidence boundary

The registered Rust test derives structural expectations from the closed
rectangle-minus-circle topology: one face, five edges, explicit retained and
created producer lineage, exact graph owner identity, and uniform-scale
invariance of semantic membership. Producer lineage remains separate evidence
and does not enter Geometry content. The test replays each generated canonical
v2 value and exercises structural, cross-version, and noncanonical-wire
mutants.

Two blinded derivations independently agree that the ordinary scale-1 value is
the following exact 491-byte UTF-8 sequence, with no trailing newline:

```json
{"schema":"eqiora.planar-circular-hole-envelope/v2","encoding":"eqiora.canonical-json/v1","kind":"axis-aligned-rectangle-with-circular-hole-v2","length_unit":"metre","bounds":[[0.0,2.2],[0.0,0.41]],"circle":{"center":[0.2,0.2],"radius_m":0.05},"entity_sets":[{"name":"cylinder","dimension":1,"members":[4]},{"name":"inlet","dimension":1,"members":[0]},{"name":"outlet","dimension":1,"members":[1]},{"name":"walls","dimension":1,"members":[2,3]},{"name":"fluid","dimension":2,"members":[0]}]}
```

Its plain SHA-256 is
`f9a278430c0033f2b0ec148b66d4608cf1f2b559cb46aa31ac9e7259861a26f3`.
SHA-256 over the UTF-8 schema
`eqiora.planar-circular-hole-envelope/v2`, one NUL byte, and those exact bytes
is the accepted Geometry identity
`c1226bdfc83a5539f21ecced9afe180c60c5f4ca07a952711e3f3529213dee14`.
