# Expected evidence

`model.json` is the canonical current Model fixture consumed by the package and
installed-Python owners. `historical-alpha1-compilation.json` is retained only
for the typed decoder/accessor compatibility assertion that names that release.

`identities.json` freezes the two package semantic digests, two exact source
bundle digests, resolution-record digest, canonical Model digest, and package
compilation digest produced by the registered case. It also freezes the
accepted output-less Run v1 digest and the separate package-compilation-to-Run
binding digest. Any change requires an explicit review of which digest domain
changed and why.

RFC 0055 preserves both package identities and the exact resolution record,
but specializes the three literal Component Parameter bindings instead of
materializing child Parameters. The checked-in Model, compilation, Run, and
Run-binding digests were therefore re-frozen together; they form one downstream
lineage from the changed canonical Model rather than independent fixture edits.

The same oracle is required when those package trees enter through the
capability-rooted directory adapter and when their exact inventories are
constructed directly in memory.

It is also required after preparation state is discarded and the checked-in
`models/resolution.json` plus source-digest-addressed `models/store/` releases
are replayed through both caller-opened and explicitly ambient-opened retained
directory capabilities.

The oracle is also required after both checked-in releases are atomically
installed into an initially empty explicit root. Installation state and
staging names are locators only and cannot change an identity preimage.
