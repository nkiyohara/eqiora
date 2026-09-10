# Sampled controls

`Eqiora.Controls.Sampled@0.1.0` exports two ordinary real-scalar components:

| Component | Supplied inputs | Tick outputs |
| --- | --- | --- |
| `UnitDelay` | exact periodic `tick`, required `initial_value: 1`, `u: 1` | `y = pre(memory)` |
| `DiscreteIntegrator` | exact periodic `tick`, required `initial_value: 1`, `rate: 1/s` | `before = pre(memory)`, `after = next(memory)` |

Each occurrence owns exactly one initialized state. The delay commits `u`; the
integrator commits `pre(memory) + period(tick) * rate`. All equations at a tick
are simultaneous: their source order does not choose the output or update order.
Connect the integrator's `before` or `after` endpoint to select the desired timing.
At the first tick these are respectively the supplied initial memory and the
first updated value. Outputs are absent before that tick and between ticks;
the components do not create a second held state.

No period is built in. The caller supplies one nominal clock, including its
phase, and supplies input samples at that same clock. Equal periods do not make
two distinct clocks interchangeable. Continuous-to-clocked input requires an
explicit `sample`; a continuous observation requires explicit initialized
`hold` semantics in the caller. Cross-clock wiring is rejected rather than
silently rate-converted.

## Physical quantities

The current specialization is dimensionless real scalar memory, not generic
Component type parameters, complex arithmetic, integer memory, or arrays.
To process a physical quantity, explicitly divide it by a fixed nonzero scale
of the same dimension, pass the normalized signal (or rate divided by that
scale) to these components, and multiply the selected output by the scale.
Initial memory is normalized by that same scale. These are ordinary typed
equations, not implicit unit casts or another state owner. For example, voltage
uses a scale in V and a rate in V/s; displacement uses a scale in m and a rate
in m/s. A rate with the wrong physical dimension fails compilation.

The repository's `examples/standard-sampled-components` demonstrates both
physical dimensions, exact quarter- and half-second periods, and installed
Python execution through an offline locked package. It does not implement a
RateTransition, a real-time scheduler, or a block-name-specific executor.
