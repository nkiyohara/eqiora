# Direct Parameter-driven Cartesian coordinates

This case proves one bounded Model-v8 semantic path. One exact root length
Parameter supplies the upper endpoint of axis 0 and the lower endpoint of
axis 1 in one three-dimensional Cartesian Domain. The Domain persists the
ordered fixed-or-Parameter recipe and one deduplicated `DependsOn` edge;
`KernelProgram` alone resolves the revision-local metric bounds.

The committed oracle was derived independently before implementation by an
analytic route and a separate direct numerical route. The base value
`extent = 2`
gives extents `3 × 4 × 5` and volume `60`; an independently committed second
immutable revision at `s = 3.5` gives `4.5 × 2.5 × 5` and volume `56.25`.
Changing the value through an application regeneration plan is deliberately
not claimed here; that is the next slice.

The test also checks v8 Model and Transaction replay, declaration-order
structural equivalence without relabelling exact occurrences,
structural-fingerprint generation v3, fixed-only
cross-generation structural equivalence, closed source syntax, dependency
equality, one-Domain ownership, non-Cartesian dependency rejection,
historical-codec rejection, and rejection by both incomplete edit paths until
the regeneration owner exists.
The artifact crate separately freezes deterministic Model/Transaction v8 byte
lengths and digests in `model_v8_wire`.

Run:

```bash
cargo test --locked -p eqiora --test direct_parameter_cartesian_coordinates
cargo test --locked -p eqiora-artifact --test model_v8_wire
cargo run --locked -p eqiora-verify -- run --case geometry.direct-parameter-cartesian-coordinates
```
