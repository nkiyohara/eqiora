# Independent analytic oracle

For `x=[-2,3] m`, `y=[-1,2] m`, `z0=1/2 m`, and depth `4 m`, the exact spans
are `(5,3,4) m`, bounds are `[-2,3] × [-1,2] × [1/2,9/2] m`, volume is
`60 m^3`, and total surface area is `94 m^2`.

| provenance | centroid (m) | area (m²) | outward normal |
| --- | --- | ---: | --- |
| start cap | `(1/2,1/2,1/2)` | 15 | `(0,0,-1)` |
| end cap | `(1/2,1/2,9/2)` | 15 | `(0,0,1)` |
| x-lower edge sweep | `(-2,1/2,5/2)` | 12 | `(-1,0,0)` |
| x-upper edge sweep | `(3,1/2,5/2)` | 12 | `(1,0,0)` |
| y-lower edge sweep | `(1/2,-1,5/2)` | 20 | `(0,-1,0)` |
| y-upper edge sweep | `(1/2,2,5/2)` | 20 | `(0,1,0)` |

The positive finite requested tolerance is identity only: changing `1e-9 m`
to `2e-9 m` changes the graph identity and no value in this table.
