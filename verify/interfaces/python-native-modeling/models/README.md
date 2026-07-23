# Model fixtures

The integration test builds the scalar, scalar-physical, and spatial fixtures
through the public Python native constructors and independently compiles
equivalent bounded source fixtures for structural comparison. It does not
deserialize an arbitrary Python dictionary into model meaning.

`poisson.eqi` is the source half of the bounded spatial pair: one Cartesian
interval, two oriented boundaries, one continuum Representation, one scalar
Field, a constant source Parameter, and three scoped Relations. Its source is
ordinary Eqiora Language, not Python-generated text.
