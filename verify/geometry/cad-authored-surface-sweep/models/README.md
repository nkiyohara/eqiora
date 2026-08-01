# Model inputs

`box.eqi` is the minimal generic 3D Cartesian body model for the target: the
same rectangle `x [-2,3] m`, `y [-1,2] m`, `z [0.5,4.5] m` as the authored
graph, with the six derived boundary domains `x_lower` … `z_upper`. It is the
Model side of the existing generic Cartesian Model/Geometry/Mesh artifact
boundary through which — and only through which — the complete-body and
all-six-boundary correspondence is accepted. Its domain names are the boundary
inventory rows in `../case.toml`. It adds no new grammar, wire, or schema.

The sweep source is not this file: it is the accepted source-bound surface
mesh on the immutable authored rectangle-extrusion graph already verified by
`geometry.cad-authored-rectangle-extrusion` and realized by
`geometry.cad-authored-face-reference-mesh`. The circular-through-cut
falsifier target is the authored cut graph already verified by
`geometry.cad-authored-circular-through-cut`, referenced rather than
duplicated here; it must reject, and its outer bounds must never be filled.
