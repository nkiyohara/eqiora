"""Run the Mathematical Modeling textbook's bounded decay comparison."""

from __future__ import annotations

import math
from typing import NamedTuple

import eqiora


SOURCE = """
model decay {
  field x: 1 = 1;
  parameter rate: 1 / s = 1;
  relation flow continuous {
    derivative(x) + rate * x = 0;
  }
}
"""
OUTPUT_TIMES_S = (0.25, 0.5, 1.0)


class Sample(NamedTuple):
    """One computed value beside the independently stated closed form."""

    time_s: float
    computed: float
    closed_form: float

    @property
    def absolute_error(self) -> float:
        return abs(self.computed - self.closed_form)


def solve() -> tuple[Sample, ...]:
    """Compile, resolve, run, and compare the bounded decay problem."""

    model = eqiora.compile(source=SOURCE, filename="decay.eqi")
    field = model.field(model.field_ids[0])
    plan = eqiora.resolve(
        model,
        temporal=eqiora.time.Tsitouras45(
            initial_step_s=0.01,
            relative_tolerance=1.0e-9,
            absolute_tolerances={field: 1.0e-11},
        ),
    )
    result = eqiora.run(
        plan,
        state=eqiora.State.initial(plan),
        until_s=OUTPUT_TIMES_S[-1],
        output_times_s=OUTPUT_TIMES_S,
    )
    series = result.series(field)
    times = series.time.numpy()
    values = series.values.numpy()
    return tuple(
        Sample(float(time_s), float(value), math.exp(-float(time_s)))
        for time_s, value in zip(times, values, strict=True)
    )


def main() -> None:
    for sample in solve():
        print(
            f"t={sample.time_s:.2f} "
            f"computed={sample.computed:.10f} "
            f"closed_form={sample.closed_form:.10f} "
            f"absolute_error={sample.absolute_error:.3e}"
        )


if __name__ == "__main__":
    main()
