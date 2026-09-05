# Specimen: UnitDelay and sampled integration

This is a complete target-language model with explicit external clock and input requirements,
following the [core rules](core.md). The converged signatures, initial equations, and supplied-clock
component workflow are specified here; the full source below is not yet an executable compiler
example. The standard components use these same equations.

## Inspect the components

These voltage-specialized definitions make every physical dimension explicit. General type
specialization must preserve the same equations: an integrator's rate has the memory dimension
divided by time. It cannot accept an arbitrary same-shaped input.

```eqiora
component UnitDelay(
  clock tick: periodic,
  parameter initial_value: V,
  input u: V at tick,
  output y: V at tick
) {
  state memory: V at tick;

  initial {
    memory = initial_value;
  }

  relation update at tick {
    y = pre(memory);
    next(memory) = u;
  }
}

component DiscreteIntegrator(
  clock tick: periodic,
  parameter initial_value: V,
  input rate: V / s at tick,
  output y: V at tick
) {
  state memory: V at tick;

  initial {
    memory = initial_value;
  }

  relation update at tick {
    next(memory) = pre(memory) + period(tick) * rate;
    y = next(memory);
  }
}

model SampledPair(
  clock tick: periodic,
  input sample: V at tick,
  input rate: V / s at tick,
  output delayed: V at tick,
  output integrated: V at tick
) {
  instance delay: UnitDelay(tick = tick, initial_value = 5 [V]);
  instance integrator: DiscreteIntegrator(tick = tick, initial_value = 1 [V]);

  connect sample -> delay.u;
  connect rate -> integrator.rate;

  relation expose at tick {
    delayed = delay.y;
    integrated = integrator.y;
  }
}
```

The signature's input values are external mathematical requirements. The caller must bind the
same exact periodic clock to both input sequences and to `tick`; the table below specifies one
complete binding. Output ports and each component's private memory are owned by their occurrence.
Connecting an input does not allocate a second memory or supply an initialization guess.

UnitDelay publishes pre-tick memory, then commits the current input as its new memory. These
are simultaneous equations, not two sequential callbacks. The integrator deliberately publishes
post-tick memory: it has direct feedthrough from its current rate. Publishing pre-tick memory
would be a different component contract, not an execution optimization.

Both states are lumped. There is no spatial domain or boundary condition. Inputs and outputs
are clocked and have no continuous value between ticks. A continuous consumer must use an
explicit hold with its own pre-first-tick value. Memory retention alone is not a hold adapter.

## Use packaged definitions

The intended short form imports the same definitions from an exact standard control package:

```eqiora
import Eqiora.Control.Discrete.discrete as discrete;

model SampledPair(
  clock tick: periodic,
  input sample: V at tick,
  input rate: V / s at tick,
  output delayed: V at tick,
  output integrated: V at tick
) {
  instance delay: discrete.UnitDelay(tick = tick, initial_value = 5 [V]);
  instance integrator: discrete.DiscreteIntegrator(tick = tick, initial_value = 1 [V]);
  connect sample -> delay.u;
  connect rate -> integrator.rate;
  relation expose at tick {
    delayed = delay.y;
    integrated = integrator.y;
  }
}
```

`Eqiora.Control.Discrete` is the proposed package name, not an already published dependency.
Its delivered type-specialization interface must be checked against these concrete voltage
instances. The package remains inspectable ordinary source; block names select no runtime code.

## Independent tick sequence

Bind `tick` to a clock with period 10 ms and phase zero. Fresh initialization establishes delay
memory 5 V and integrator memory 1 V before the first tick. Bind the following inputs at the
first three ticks; inputs for any later requested tick must also be supplied.

| Tick time | Sample | Rate | Delay output | Committed delay memory | Integrator output and committed memory |
|---|---|---|---|---|---|
| 0 s | 2 V | 1 V/s | 5 V | 2 V | 1.01 V |
| 0.01 s | -1 V | 2 V/s | 2 V | -1 V | 1.03 V |
| 0.02 s | 4 V | -1 V/s | -1 V | 4 V | 1.02 V |

For input `u_k`, rate `r_k`, pre-tick delay memory `d_k`, and pre-tick integrator memory `q_k`,
the recurrences are `d_(k+1) = u_k` and `q_(k+1) = q_k + h*r_k`. Outputs are `d_k` and
`q_(k+1)`. Substitution with `h = 0.01 s` gives the table directly. In particular the tick at
zero includes an increment: this discrete recurrence does not claim to be a continuous-time
integral from zero elapsed time.

Repeat the same input sequence with a 20 ms period. The delay outputs remain 5 V, 2 V, -1 V;
the integrator outputs become 1.02 V, 1.06 V, 1.04 V at times 0, 0.02 s, 0.04 s. With a
10 ms period and 30 ms phase, the original outputs occur at 0.03 s, 0.04 s, 0.05 s. Before
0.03 s the initialized memories exist, but no clocked output sample has occurred.

An exact restart after the second accepted tick retains delay memory -1 V, integrator memory
1.03 V, and clock progress. The next outputs are -1 V and 1.02 V for the third input pair.
Restart must neither repeat the second tick nor reapply 5 V and 1 V initialization.

## Rejections and invariants

| Input or change | Required outcome |
|---|---|
| Omit `initial_value` | Missing required binding, not zero initialization |
| Bind a rate of type `V` | Dimension error: multiplying by period would yield `V*s` |
| Connect an independent equal-period clock | Nominal clock mismatch |
| Connect a continuous voltage directly to `u` | Activation mismatch; require explicit sampling |
| Add a second driver for `delay.u` | Driver uniqueness error |
| Supply contradictory initial equations | Initialization rejection before accepted execution |
| Read `delay.memory` from the enclosing model | Private member access error |

Permuting equations in either update relation must preserve each tick's simultaneous solution.
A rejected solve must not partially update one component. Calling an observer between attempts
must not advance clock progress or state. These checks belong to the existing clocked runtime
and component tests; the specimen adds no separate executor or evidence registry.
