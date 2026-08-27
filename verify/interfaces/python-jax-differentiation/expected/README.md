# Expected invariants

The installed wheel must produce the same complete primary Field, directional
tangent, and input cotangent as the accepted framework-neutral Eqiora program
at the same Parameter point. Eager and `jit` execution must agree, while the
lowered StableHLO must contain the declared Eqiora typed custom calls and no
Python host callback.

Only the numerical point may be traced. The exact common Plan, caller-supplied
Mesh, Model, ordered input identities, output identity, shapes, dtype, layout,
and host-CPU platform remain static; the concrete CPU ordinal follows the
input. Unsupported
metadata, direct or explicitly compiled input sharding, `pmap`,
`vmap`/higher-order transformations, non-finite values, and unknown program
identities must fail closed rather than copy, gather, batch, or fall back to
Python. Explicit output sharding remains an unverified nonclaim.
