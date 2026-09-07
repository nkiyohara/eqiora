# Expected invariants

The installed wheel must produce the same complete primary Field, directional
tangent, and input cotangent as the accepted framework-neutral Eqiora program
at the same Parameter point. Eager and `jit` execution must agree, while the
lowered StableHLO must contain the declared Eqiora typed custom calls and no
Python host callback or per-sample unrolling. Permutations and duplicate points
preserve their ordered occurrences. Point axes and independent derivative-seed
axes preserve their association under nesting and transposition; adding basis
seeds does not duplicate the FFI Parameter-point buffer.

For the already admitted elliptic discretization, linearity independently gives
`u_h(s,k,b) = (s/k) q_h + b`, where `q_h` is the separately accepted unit-source,
unit-diffusion, zero-boundary response. The mapped fixture uses source rows
`[2,3,2]` and `[1,4,1]`, globally shared `k=2`, and row-shared offsets
`[-0.1,0.2]`. For the sum of the six mean-Field outputs:

- the value is `13 mean(q_h)/2 + 3(-0.1+0.2)`;
- the shared diffusion derivative is `-13 mean(q_h)/4`;
- the two offset derivatives are `[3,3]`;
- each source derivative is `mean(q_h)/2`.

The explicit mean of those six outputs divides every derivative by six. This
falsifies lost multiplicity, detached shared inputs, unintended averaging, and
global sharing substituted for row-only sharing. The pointwise accepted product
tolerances remain unchanged; no observed batched gradient sets an expectation.

Only the numerical point may be traced. The exact common Plan, caller-supplied
Mesh, Model, ordered input identities, output identity, shapes, dtype, layout,
and host-CPU platform remain static; the concrete CPU ordinal follows the
input. Empty and singleton grids retain exact shapes; a failed required member
rejects the dense result with its occurrence identified. Unsupported metadata,
direct or explicitly compiled input sharding, named collectives, `pmap`,
higher-order transformations, non-finite values, and unknown program identities
must fail closed without gathering, dropping members, or falling back to Python.
Explicit output sharding remains an unverified nonclaim.
