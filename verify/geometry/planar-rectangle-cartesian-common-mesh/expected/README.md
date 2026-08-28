# Expected

For `(nx, ny) = (2, 3)`, x coordinates are `[0, 1, 2]` and y coordinates are
`[-1, 0, 1, 2]`. Vertex ID is `ix * 4 + iy`; cell ID is `ix * 3 + iy`.
Ordered cell vertices are:

```text
[0,4,1,5] [1,5,2,6] [2,6,3,7]
[4,8,5,9] [5,9,6,10] [6,10,7,11]
```

Facet IDs `0..7` vary in x first with ordered vertices `[0,4]`, `[1,5]`,
`[2,6]`, `[3,7]`, `[4,8]`, `[5,9]`, `[6,10]`, `[7,11]`; IDs `8..16` vary in
y with `[0,1]`, `[1,2]`, `[2,3]`, `[4,5]`, `[5,6]`, `[6,7]`, `[8,9]`,
`[9,10]`, `[10,11]`. Exact source memberships are left `[8,9,10]`, right
`[14,15,16]`, bottom `[0,4]`, and top `[3,7]`. Their respective local side
ordinals are 2, 3, 0, and 1, every orientation is identity, and their exact
parent cells are asserted in the registered test. Boundary facets
`{0,3,4,7,8,9,10,14,15,16}` and interior facets `{1,2,5,6,11,12,13}` form a
complete disjoint 17-facet partition.
