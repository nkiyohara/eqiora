# Specimen: UnitDelay and sampled integration

These ordinary source components declare explicit clock and input requirements, following the
[core rules](core.md). The open model below requires a caller to supply its clock and drive its
inputs before execution. Each component owns its output and private initialized memory.

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
explicit `hold(memory)` of a directly named periodic State with an explicit initial equation.
That equation supplies the value before the first tick; after each accepted tick, the hold reads
the committed memory. A clocked output Port by itself is not a hold operand.

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

## Independent memories under one clock

Two occurrences of `UnitDelay` share a 0.25 s clock but own distinct memories. Bind the first
input to a constant 2 V with initial memory 5 V, and the second input to a constant 7 V with
initial memory -3 V. Bind `DiscreteIntegrator` to the same clock, initial memory 1 V, and
constant rate 2 V/s.

| Tick time | First delay output | Second delay output | Integrator output |
|---|---|---|---|
| 0 s | 5 V | -3 V | 1.5 V |
| 0.25 s | 2 V | 7 V | 2 V |
| 0.5 s | 2 V | 7 V | 2.5 V |

Each delay publishes its own pre-tick value. Each integrator tick adds exactly
`0.25 s * 2 V/s = 0.5 V`, including the tick at zero. This table follows directly from the
recurrences above; it does not depend on an execution trace or a component-name convention.

## Fixed channel memory

The sampled reference executor also accepts fixed channel arrays of real or exact
integer values. A whole array is one typed assignment target. Component expressions
use ordinary scalar arithmetic and explicit indexing:

```eqiora
model ChannelDelay(clock tick: periodic, input u: array<V, 2> at tick,
                   output y: array<V, 2> at tick) {
  state memory: array<V, 2> at tick;
  initial { memory = [0[V], 0[V]]; }
  relation update at tick {
    y = pre(memory);
    next(memory) = [u[1], u[0]];
  }
}
```

For input `[2 V, 3 V]`, the first tick publishes `[0 V, 0 V]` and commits
`[3 V, 2 V]`. Every right-hand side reads the same accepted pre-tick state;
equation order cannot update one component early. A failed component evaluation,
including exact integer overflow, rejects the whole tick and preserves state,
calendar, input position and output presence. Checkpoint/resume retains complete
array values without rerunning initialization.

Extents are fixed by the Model. Scalar broadcasting, partial indexed assignment,
general whole-array arithmetic, spatial tensors, complex execution and Boolean
arrays are outside this profile. The existing one-million retained-value limits
count scalar components; intermediate array construction also has a bounded total
component budget. These are focused product capabilities, not new registered
scientific evidence or a production scheduling claim.
