"""Resolve, run, and optionally plot the accepted fixed-mesh FSI case."""

import argparse
from importlib.resources import files
from pathlib import Path

import eqiora


def compile_model() -> eqiora.Model:
    source = (
        files(eqiora)
        .joinpath("examples", "fixed-reference-fsi.eqi")
        .read_text(encoding="utf-8")
    )
    return eqiora.compile(
        source=source,
        filename="fixed-reference-fsi.eqi",
    )


def intent() -> eqiora.fsi.FixedMeshMonolithic:
    return eqiora.fsi.FixedMeshMonolithic(
        time_step_s=0.05,
        steps=2,
        initial_velocity_m_per_s=(0.0, 0.0),
        initial_free_interface_displacement_m=(0.02, 0.0),
        length_scale_m=2.0,
        velocity_scale_m_per_s=0.5,
        pressure_scale_pa=4.0,
        relative_tolerance=1.0e-11,
        absolute_tolerance=1.0e-13,
        maximum_iterations=20_000,
    )


def solve(model: eqiora.Model) -> eqiora.Result:
    plan = eqiora.fsi.resolve(model, intent())
    return eqiora.submit(model, plan=plan).result()


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--fsi-png",
        type=Path,
        help="save the accepted deformed field still (requires eqiora[matplotlib])",
    )
    parser.add_argument(
        "--pressure-png",
        type=Path,
        help="save the accepted pressure field still (requires eqiora[matplotlib])",
    )
    parser.add_argument(
        "--step",
        type=int,
        choices=(1, 2),
        default=2,
        help="accepted step ordinal used only by the optional still",
    )
    parser.add_argument(
        "--displacement-scale",
        type=float,
        default=12.0,
        help="visible solid displacement scale used only by the optional still",
    )
    arguments = parser.parse_args()

    model = compile_model()
    result = solve(model)
    trajectory = result.trajectory
    evidence = eqiora.fsi.fixed_mesh_monolithic_evidence(result)
    print(evidence.run_digest)
    print(trajectory.digest)
    for state in trajectory.states:
        state_evidence = evidence.state(state)
        print(
            f"step {state.step} at {state.time_s:g} s",
            state_evidence.solve,
            f"energy defect {state_evidence.energy_defect_j_per_m:.6e} J/m",
        )
    if arguments.fsi_png is not None or arguments.pressure_png is not None:
        import eqiora.matplotlib as eqplot

        if arguments.pressure_png is not None:
            pressure = eqplot.plot_scalar_field(
                trajectory,
                step=arguments.step,
                field=model.field("fluid_pressure"),
            )
            pressure.savefig(arguments.pressure_png, dpi=160)
            print("Pressure field still", arguments.pressure_png)
        if arguments.fsi_png is not None:
            deformed = eqplot.plot_deformed_field(
                trajectory,
                step=arguments.step,
                field=model.field("solid_displacement"),
                scale=arguments.displacement_scale,
            )
            deformed.savefig(arguments.fsi_png, dpi=160)
            print("Deformed field still", arguments.fsi_png)


if __name__ == "__main__":
    main()
