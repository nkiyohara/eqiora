# Model construction

There is deliberately no durable model fixture in this case. The Rust oracle
compiles the previously accepted transient conservative Navier--Stokes grammar
with homogeneous inlet/wall/cylinder trace and homogeneous outlet traction,
then replaces its Cartesian authoring scaffold with typed `GeometryRegion` and
`GeometryBoundary` nodes bound to the exact source digest.

Keeping construction in the crate-private oracle avoids introducing a second
model contract, source artifact, or public model surface. The Cartesian
lowerer must reject the transformed positive program with `EQ0703`; only the
frozen private source-bound composition may admit it.
