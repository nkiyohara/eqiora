# Model fixtures

The executable integration test owns three minimal source fixtures: a spatial
Model selected as wire v1, a scalar-physical Model selected as wire v2, and a
field-boundary Model selected as wire v3. Keeping them beside the Rust
construction code makes the selected wire and resulting typed reference part
of one reviewable falsifier; this directory adds no second copy of those
sources.

The cross-wire substitution test constructs one native semantic graph once
and encodes that same graph through v1, v2, and v3, so Model identity and
revision remain equal while the schema-domain-separated digests differ.
