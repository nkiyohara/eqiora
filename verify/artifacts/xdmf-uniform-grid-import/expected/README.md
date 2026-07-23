# Expected evidence

The metadata plan normalizes four arrays into canonical role order:

1. `f64[4,2]` geometry `[[0,0],[1,0],[1,1],[0,1]]`;
2. `u64[2,3]` topology `[[0,1,2],[0,2,3]]`;
3. node `f64[4]` scalar `[10,20,30,40]`; and
4. cell `f64[2,2]` vector `[[1,0],[0,1]]`.

The reconstructed mesh has two positively oriented triangles covering the
unit square. Exact source-byte digests are recorded in `source.sha256`; value
and accepted-artifact identities are recomputed by the public integration
test instead of copied from the generating tool.
