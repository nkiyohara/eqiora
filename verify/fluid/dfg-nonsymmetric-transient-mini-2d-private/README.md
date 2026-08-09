# Private DFG transient MINI mechanics evidence

This pre-implementation package freezes NUM0 only: one crate-private,
exact-source-bound two-dimensional transient MINI/P1 path for
`sigma_DFG = mu grad(u) - p I`. It is intentionally RED on protected base
`4de2a24cc41dbe1f1cc72e4bfafda61b551224f1`; that revision has neither the
private DFG binding/advance entry nor the production DFG viscous-pair action.

The package starts with an ordinary positive, not a local or exact-zero
substitute. The Rust selector must:

1. replay the accepted circular-hole source, mesh owner, five-set
   correspondence, Model, revision, and serial-host Realization;
2. bind the exact positive `rho=1`, `nu=mu=0.001`, `Umax=0.3`, `H=0.41`
   DFG Model without a public/runtime stress selector;
3. construct prescribed values from correspondence ownership before using
   coordinates to evaluate the inlet profile;
4. independently obtain one finite, nonzero, weakly continuous MINI/P1
   initial state from the already accepted steady MINI solver on the same
   boundary data;
5. advance exactly one checked backward-Euler/Newton step while a scoped
   test-only probe observes the actual DFG pair used by its local
   assembly/linearization on both P1 and MINI-bubble actions; and
6. return two finite states, one accepted step, advanced time,
   `BoundaryTraction`, no gauge coefficient, at least one checked packet, and
   a nonempty centered-Jacobian audit.

Only after that positive does the selector execute the direct DFG local-pair
discriminator and the linked mutant obligations. A second run makes the same
scoped pair probe return non-finite data and must fail: this proves the
source-bound advance consumes the observed pair rather than calling a correct
unused helper before delegating to the symmetric path. The probe exists only
under the crate-private test surface; it is not a public/runtime stress
selector, product option, result DTO, or durable evidence channel.

The standalone exact oracle
uses `fractions.Fraction` to derive the direct block independently of the
production symmetric-minus-crossed expression:

```bash
python3 verify/fluid/dfg-nonsymmetric-transient-mini-2d-private/oracle.py
```

It fixes the affine P1, actual MINI-bubble, pressure/continuity, inlet,
no-gauge, convection-identity, all-16-mutant, coefficient-count, packet-count,
the exact finite abstract-work product, and targeted checked-overflow outcomes
with no floating tolerance. It chooses no DFG
benchmark result, comparison interval, mesh/refinement family, time-series
observable, or solver acceptance tolerance.

The future integrator-owned manifest must register the exact library selector

```text
canonical_stokes::navier_stokes_geometry_realization::tests::
registered_dfg_nonsymmetric_transient_mini_oracle_executes_all_falsifiers
```

and run the exact Python oracle before claiming the case verified. Until the
implementation exists, a Rust compile failure caused by the absent private
DFG entries is the expected RED result, not evidence of acceptance.

## Claim boundary

This evidence claims only the private DFG scientific operator, matching zero
natural outlet, correspondence-owned nonzero inlet, no-gauge pressure closure,
one nonzero source-bound step, and deterministic abstract bounds. It does not
claim DFG 2D-1/2D-2 acceptance, S1/S2, stationary continuation, periodic
development, `C_D`, `C_L`, `Delta p`, `St`, F1/F2, reaction equivalence,
pressure recovery, vorticity, a production mesh campaign, general
nonsymmetric stress, a general do-nothing boundary, public Rust/Python/Studio
API, durable trajectory/Result/schema/wire, gallery publication, performance,
GPU, MPI, external-source redistribution, or SI interpretation of source
coordinates.

## Precommitment

The evidence author read the accepted NUM0 contract and review and the sealed
dual-derivation reconciliation at SHA-256
`b5427867b7039d15e7a776a80a6a3a6bf9a34b0993850a640b6cdc416c5e9a78`.
The four focused corrections answer the exact evidence review at SHA-256
`98ce1eab3ce1d755412199e95f46631b27b758687595f1236bd6419069449f63`;
they do not reopen or amend the scientific formulation.
No NUM0 implementation, candidate output, implementation-writer scratch,
benchmark value, or tuned tolerance was read. The finite positive inputs use
the exact accepted DFG tuple, predecessor source/mesh owner, predecessor
quadratures and solver plans, and an independently solved steady MINI state;
none depend on candidate DFG behavior.
