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

The reference interpreter localizes crossings with its configured guard and time tolerances.
A crossing must start outside the guard tolerance band on the armed side. Resetting onto the
guard does not immediately retrigger it; an accepted departure must precede a new crossing.
Localization and nonlinear tolerances are execution settings, while the guard, direction and
reset equations remain Model meaning. Bounded event iteration and chattering diagnostics remain
in force; continuation through Zeno accumulation is not admitted.

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

This profile does not establish resting contact, arbitrary simultaneous distinct events,
periodic-event coincidence, a durable event checkpoint, the common Diffsol ODE event path,
or a general hybrid solver.
