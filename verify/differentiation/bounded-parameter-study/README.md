# Ordered independent evaluation maps

This existing case now owns the current ordered map contract. Its historical case ID is
retained; sorted single-Parameter studies are no longer an active API or acceptance claim.

The primary Rust integration test evaluates the same complete points outside the map, then
checks every mapped occurrence against those ordinary accepted evaluations. Both Q1 and TPFA
run with ordered inputs `[source_scale, diffusion, boundary_offset]` and points
`p1=[2,0.75,0.5]`, `p2=[3,1.25,-0.25]`. The request `[p2,p1,p2]` must preserve all three
positions; `[p1,p2,p2]` must produce that different order. No default anchor is requested.

The reference is composition, not another scalar solver: exact point and Field bits, program
identity, accepted operator/output fingerprints and exposed evidence match separate evaluations.
Retained primal/JVP/VJP actions survive other points and repeated `p0=[1,1,0]` evaluations.
There are no new scientific expected coefficients, digests, or tolerances.
Pointwise numerical truth remains with
[`differentiation.spatial-poisson-fem-fvm`](../spatial-poisson-fem-fvm/README.md).

Success requires one complete collection. Numerical failure and boundary cancellation preserve
the individually accepted prefix, identify the terminal occurrence, and leave later positions
not started. Empty maps finish without evaluation or cancellation polling. Singleton maps,
structural rejection, exact byte-limit admission and cancellation precedence are exercised.

The required [private companion](../bounded-parameter-study-private/README.md) tests constructor
and evaluator substitution, membership, call count/order, and terminal-state falsifiers.
Both cases are required for this composition claim.

The intentional migration removes the former two-to-sixty-four/default-anchor/unique-order
oracle and its obsolete precommitment claims. Expectations now come from the current
[axis contract](../../../docs/evaluation/independent-map-axes.md) and ordinary independent
point evaluations, not observed map output. Published historical artifacts are unchanged.

```bash
mise run affected -- \
  --case differentiation.bounded-parameter-study \
  --case differentiation.bounded-parameter-study-private
```

This does not verify Python/JAX adapters, derivatives across the map axis, threading, caching,
stochastic paths, persistence, or peak process memory. The byte estimate bounds additional
retained numerical storage; deployment metadata, allocator overhead and solver scratch are
outside that estimate.
