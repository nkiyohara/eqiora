# Model fixtures

The executable integration test owns three minimal source fixtures: a spatial
Model, a scalar-physical Model, and a field-boundary Model. Each is compiled
and replayed through the one current owner. Keeping them beside the Rust
construction code makes the meaning and resulting typed reference part of one
reviewable path; this directory adds no second copy of those sources.

A second fixture constructs one native semantic graph, encodes and decodes its
current artifact, and validates the resulting reference against an existing
Realization.
