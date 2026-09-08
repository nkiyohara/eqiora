# Authored crossing events

An event owns a real scalar zero-crossing guard and an explicit direction:

```eqiora
event impact = crossing(height, direction = falling);
let reflected_velocity at impact = -restitution * pre(velocity);
relation reset at impact {
  next(height) = 0[m];
  next(velocity) = reflected_velocity;
}
```

The guard retains its physical dimensions. It is a quantity crossing zero, not a Boolean
test or an equation. `direction` is exactly `rising`, `falling`, or `any`; omission rejects.
Events are private to a Model or Component. Each Component occurrence owns a distinct event
identity, even when names and guards match. All Relations naming one event share that identity.
Fields, ports and borrowed clock requirements retain their periodic-clock profile.

In an event reset, `pre` reads the committed left state and `next` denotes the accepted right
state of an eligible continuous state. Reset equations are simultaneous. States absent from
the reset targets retain their accepted values. The reference interpreter stages the complete
reset and post-reset continuous consistency solve before committing; inconsistent equations
or a failed consistency solve do not publish partial state.

An event-local alias retains its exact reset-use context separately from the state declaration's
continuous activation. Direct or transitive use at another event or in an ordinary continuous
Relation rejects. An `at event` assertion requires an actual reset obligation or a dependency
on an alias of that event; it cannot turn a static expression or ordinary current-state read
into an event dependency. Aliases introduce no new unknowns or physical equations.

The reference interpreter arms a crossing after an accepted value outside the guard tolerance
band on the required side. Arming persists while the trajectory approaches zero through that
band. Localization searches for zero; the arming tolerance does not move the crossing surface.
Resetting onto the guard requires an accepted departure before a new crossing. Localization
and nonlinear tolerances are execution settings, while the guard, direction and reset equations
remain Model meaning.

## Coincident activations

An exact periodic clock supplies its nominal tick time. At that time, an armed guard whose
numerically evaluated value is zero can join the tick. This is a bounded numerical execution
contract, not proof that arbitrary mathematical roots are exactly equal. A nonzero guard in
an unresolved root bracket overlapping a tick rejects with the involved event and clock
identities. Overlapping event brackets without an admitted common zero likewise reject;
localization tolerance supplies no implicit precedence.

The first microstep solves the admitted simultaneous event and tick Relations together. Every
`pre` and coincident `sample` reads the same left state. Distinct active owners may update
disjoint states. If distinct owners target the same `next` state, the boundary rejects and
identifies the owners and state. Splitting one event's equations across several Relations
retains one simultaneous owner. Declaration order and identity sorting supply no priority.

After resets and continuous consistency, newly crossed guards execute in event-only
microsteps. Each microstep reads the preceding candidate state; the tick and its input samples
occur once. The complete boundary commits state, clock progress, input cursors and outputs
only after stabilization. A conflicting reset, failed solve or exceeded zero-time iteration
bound leaves the preceding accepted boundary intact. Diagnostics identify the involved
activations; continuation through Zeno accumulation is not admitted.

For a concrete affine example, start `x = y = z = 0` with slopes `1`, `2` and `0` in
consistent units. At a period-1 tick at time 1, sample `x + y` into clocked memory while
guards `x - 1` and `y - 2` reset `x = 10` and `y = 20`. Memory becomes 3. A guard
`x - 5` crossed by that reset can set `z = 7` in the next microstep. The stabilized state
is `(10, 20, 7)` with memory 3; at time 1.25 it is `(10.25, 20.5, 7)` with memory 3.
These values follow directly from the affine equations with quarter-second steps.

`ExecutionSession` advances through the same accepted boundaries used by ordinary reference
and CPU runs. Its activation sequence reports the exact owners in each microstep of the last
accepted boundary. In-process checkpoints retain the immutable program, request, accepted
state, event arming, clock progress, input cursors and output presence. Resuming a checkpoint
does not repeat initialization or the already accepted tick. Durable serialization remains
outside this profile.

For a thermostat with initial temperature 20 K and slope +1 K/s, explicit events at 22 K and
18 K can reverse the slope through `next(rate) + pre(rate) = 0[K/s]`. The independently
derived switches occur at 2, 6 and 10 seconds, with temperature 20 K and slope -1 K/s at
12 seconds. This memory is authored state and reset behavior. A
[conditional value](conditionals.md) alone creates no hysteresis or event.

Python `Component.event(name, guard, *, direction=...)` returns a distinct `q.Event` handle.
`relation(..., at=event)` and `let_alias(..., at=event)` retain that local owner; a Clock or
foreign Event cannot substitute for it. Python authors the same source and adds no event
evaluator. The current authoring helpers do not establish a complete Python-authored ODE
surface; ordinary source compilation and the admitted reference Run exercise crossing dynamics.

This profile does not establish resting contact, arbitrary multiple roots inside one numerical
step, authored priority between conflicting resets, a durable event checkpoint, the common
Diffsol ODE event path, or a general hybrid solver.
