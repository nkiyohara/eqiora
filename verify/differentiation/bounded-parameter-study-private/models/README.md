# Model source

The crate-private oracle embeds the exact accepted source text from
[`spatial-poisson-fem-fvm/models/poisson.eqi`](../../spatial-poisson-fem-fvm/models/poisson.eqi)
inside its package-owned Rust test source. This keeps packaged evidence
self-contained without changing or independently interpreting the scientific
model.
