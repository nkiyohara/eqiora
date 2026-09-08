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

## Bounded clock/event composition

The affine companion fixture isolates calendar semantics from flight accuracy.
With `x' = 1`, `y' = 2`, `z' = 0`, and initial values zero, distinct rising
crossings of `x - 1` and `y - 2` meet a period-one clock with phase one.
The event resets assign `x = 10` and `y = 20`; the clock samples the left-state
sum into memory. A post-reset crossing of `x - 5` assigns `z = 7` in an
event-only microstep. Quarter steps and these affine values give an exactly
representable numerical-zero witness at the nominal tick.

| Accepted boundary | x | y | z | memory |
| --- | ---: | ---: | ---: | ---: |
| 0.75 | 0.75 | 1.5 | 0 | 0 |
| 1, after stabilization | 10 | 20 | 7 | 3 |
| 1.25 | 10.25 | 20.5 | 7 | 3 |

The memory value distinguishes left-state sampling from sampling the reset
sum, 30. Reversed declaration order and independently allocated IDs must
preserve this projection. In-memory clones at 0.75 and stabilized time 1
must reproduce the activation sequence and final State without replaying the
tick. Conflicting simultaneous writes reject without advancing the session.

Nearby distinct roots have a separate positive and negative control: moving
the first root to `1 - 2^-20` yields memory `12 + 2^-20` when resolved;
coarser overlapping localization brackets reject rather than manufacture
coincidence. Entering the arming band before zero must neither fire early nor
lose the armed crossing, including across a checkpoint.

This clause concerns the bounded reference session, numerical-zero witnesses,
and stabilized in-memory clones. It does not establish arbitrary mathematical
real-time equality, a persisted checkpoint format, general contact mechanics,
prioritized conflicting resets, or the separate Diffsol event contract.
