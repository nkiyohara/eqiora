# Bouncing-ball event execution

This case exercises canonical hybrid semantics without a block- or
bouncing-ball-specific kernel node. Two continuous residuals define flight;
two independent event Relations reset height and velocity at the same falling
height crossing.

The reference interpreter re-solves the implicit continuous step while
bracketing the event. It records the state immediately before and after reset
at one model time, and solves both reset Relations as one system so graph
insertion order cannot affect the result.

Run:

```bash
cargo test -p eqiora-sem --test reference_event
```

The status is `implemented`, not `verified`. This execution evidence fixes
activation, localization, reset, and safety semantics but does not yet
establish temporal convergence or long-time behavior. The separate
`differentiation.hybrid-event` case verifies a narrow localized saltation
contract, and `hybrid.registered-event` connects that class to a production
proposal and explicit restart, without widening this trajectory claim.
