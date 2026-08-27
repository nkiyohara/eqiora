# Model fixtures

The primary store and resolution are reused read-only from
`verify/packages/offline-model-package/models/`.

`typed-compilation-lineage/` contains only the exact canonical release/store and
resolution bytes derived by the already accepted Rust producer for
`org.example.poisson`. That secondary fixture is used solely because the
primary electrical Model has no editable Field or Parameter; it proves that a
committed child clears its parent's package-compilation lineage.
