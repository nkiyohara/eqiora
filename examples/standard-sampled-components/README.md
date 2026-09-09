# Sampled voltage and displacement

This model composes the shipped `Eqiora.Controls.Sampled` components through
ordinary typed adapters. Voltage uses a fixed scale of 2 V; displacement uses
0.5 m. The adapters have no state. Four independent block occurrences own four
memories, initialized to 10 V, 6 V, 2.5 m and 1.5 m after rescaling.

From a checkout, install the current package and run the ordinary Python API:

```bash
uv pip install .
```

```python
from pathlib import Path
import eqiora

project = Path("examples/standard-sampled-components")
store = Path(".eqiora-sampled-store")
store.mkdir(exist_ok=True)
resolution = eqiora.add_bundled_dependency(
    project, store, "Eqiora.Controls.Sampled", version="0.1.0"
)
model = eqiora.compile_package(store, resolution, entry="Main")
session = model.execution_session(
    end_time_s=0.5, max_step_s=0.25,
    inputs={
        "voltage": ("tick", [4.0, -2.0, 6.0]),
        "slew": ("tick", [4.0, -2.0, 6.0]),
        "position": ("tick", [-0.5, 2.0, 1.0]),
        "velocity": ("tick", [-0.5, 2.0, 1.0]),
    },
)
session.advance_ticks(3)
print([session.output("voltage_after", i) for i in range(3)])
```

Values supplied to each typed input are in its coherent SI unit. In normalized
coordinates the voltage signal and rate are `[2, -1, 3]`, while displacement
and velocity are `[-1, 4, 2]`. Each delay memory starts at 5 and each integrator
memory at 3. The independent recurrence gives:

| Group | Period | Delay output | Integrator `before` | Integrator `after` |
| --- | --- | --- | --- | --- |
| Voltage | 1/4 s | 5, 2, -1 | 3, 3.5, 3.25 | 3.5, 3.25, 4 |
| Voltage | 1/2 s | 5, 2, -1 | 3, 4, 3.5 | 4, 3.5, 5 |
| Displacement | 1/4 s | 5, -1, 4 | 3, 2.75, 3.75 | 2.75, 3.75, 4.25 |
| Displacement | 1/2 s | 5, -1, 4 | 3, 2.5, 4.5 | 2.5, 4.5, 5.5 |

Multiply these numbers by 2 V or 0.5 m for the physical outputs. To exercise
the half-second clock, change the caller Model's clock to `periodic(0.5[s])`,
explicitly update the project lock, and set `end_time_s=1.0` and
`max_step_s=0.5`. Neither standard component definition changes. Both clocks
first tick at zero. Before that first tick, no
output sample is present. Selecting `before` or `after` does not add memory.

For the current type and clock boundaries, see the
[sampled package contract](../../packages/Eqiora.Controls.Sampled/README.md).
