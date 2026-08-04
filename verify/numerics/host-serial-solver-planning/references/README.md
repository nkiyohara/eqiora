# Independent reference

`derive_policy_v1.py` is the independent authority for the exact rational
system and deterministic selection table. It uses Python `Fraction` arithmetic
to derive `[1, 2]` and the zero residual, locates both diagonal entries directly
from the captured CSR offsets and column indices, freezes the decimal control
values and their IEEE-754 bit patterns, and ranks a literal three-record
catalog without importing or executing Rust.

The derivation enumerates all six input permutations and every nonempty
admitted subset, checking the complete ID-ordered reason trace and exact
reranking for every objective. Independent mutants reverse each precedence
ordering whose effect is observable in the frozen catalog and replace the
final candidate-ID tie-break with input enumeration. Swapping Fast's reduction
and algorithm axes is explicitly not a falsifier because it changes no
selection for the full catalog or any admitted subset. The script also freezes
every rejection and execution falsifier consumed by the Rust oracle. An
isolated test-only ledger on the exact owned canonical view first observes and
resets one direct `problem.operator()` apply and diagonal self-control. It then
freezes the two total exact-operator applications required to recompute initial
and final true residuals on successful fake-backend acceptance, and rejects
one extra direct apply or diagonal.

The derivation separately freezes the complete Robust, Fast, and LowMemory
traces when faer BiCGSTAB simultaneously has stale evidence and provider
identity. The observed first and only rejection is
`catalog.evidence-mismatch`; substituting provider-first precedence changes
each trace.

The faer descriptor's actual dependency inventory has exactly one member, so
the oracle requires exact ordered equality of that inventory and rejects a
changed, missing, or extra dependency. It creates no fake second dependency
and makes no same-members reorder claim.

## Pre-implementation residual audit

Before this evidence was committed, all three accepted live backends were run
at protected base `f36f6f029e7cdc59b81163355ff07ec1cdb9c78e` on the frozen
fixture and common controls. Independent CSR replay observed:

| Candidate | Values | True residual norm | Maximum component error |
| --- | --- | --- | --- |
| reference BiCGSTAB | `[1.0000000000000002, 1.9999999999999996]` | `2^-49` | `2^-51` |
| faer BiCGSTAB | `[1.0000000000000002, 1.9999999999999998]` | `2^-50` | `2^-52` |
| faer SparseLU | `[1, 2]` | `0` | `0` |

Each is strictly inside the precommitted componentwise `2^-40` bound and also
passes the unchanged backend-independent true-residual target. These one-host
values validate the bound at the frozen host-serial `f64` execution boundary;
they are not performance, portability, or bitwise cross-environment claims.
