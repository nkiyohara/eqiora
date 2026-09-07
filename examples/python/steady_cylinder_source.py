"""Author the steady-cylinder equations as one Eqiora Language Source."""

from eqiora import lang as q
from eqiora import Dimension, FieldRole, ValueType

def build_source() -> q.Source:
    """Return the complete equations-only steady-cylinder Component."""

    source = q.Source()
    stokes = source.component(
        "SteadyFlowPastCylinder",
        doc="Equations-only steady incompressible flow around a cylinder.",
    )
    fluid = stokes.volume("fluid", dimensions=2)
    inlet = stokes.boundary("inlet", parent=fluid)
    outlet = stokes.boundary("outlet", parent=fluid)
    walls = stokes.boundary("walls", parent=fluid)
    cylinder = stokes.boundary("cylinder", parent=fluid)

    dynamic_viscosity = stokes.parameter(
        "dynamic_viscosity", value_type=ValueType.real(Dimension(mass=1, length=-1, time=-1))
    )
    zero_pressure = stokes.parameter(
        "zero_pressure", value_type=ValueType.real(Dimension(mass=1, length=-1, time=-2))
    )
    inlet_speed = stokes.parameter(
        "inlet_speed", value_type=ValueType.real(Dimension(length=1, time=-1))
    )
    channel_height = stokes.parameter("channel_height", value_type=ValueType.real(Dimension(length=1)))

    velocity = stokes.field(
        "velocity", role=FieldRole.Variable,
        on=fluid,
        value_type=ValueType.vector(ValueType.real(Dimension(length=1, time=-1)), 2),
    )
    pressure = stokes.field(
        "pressure", role=FieldRole.Variable,
        on=fluid,
        value_type=ValueType.real(Dimension(mass=1, length=-1, time=-2)),
    )
    force_potential = stokes.field(
        "force_potential", role=FieldRole.Variable,
        on=fluid,
        value_type=ValueType.real(Dimension(mass=1, length=-1, time=-2)),
    )
    inlet_profile = stokes.field("inlet_profile", role=FieldRole.Variable, on=fluid, value_type=ValueType.real(Dimension(length=1, time=-1)))

    stokes.relation(
        "force_definition",
        on=fluid,
        right=0, left=force_potential - zero_pressure,
    )
    stokes.relation(
        "inlet_profile_definition",
        on=fluid,
        right=0, left=(
            inlet_profile
            - 4
            * inlet_speed
            * q.coordinate(1)
            * (channel_height - q.coordinate(1))
            / channel_height**2
        ),
    )
    stress = 2 * dynamic_viscosity * q.symmetric_part(
        q.grad(velocity)
    ) - q.isotropic_lift(pressure)
    stokes.relation(
        "momentum",
        on=fluid,
        right=0, left=-q.div(stress) - q.grad(force_potential),
    )
    stokes.relation(
        "incompressibility",
        on=fluid,
        right=0, left=q.div(velocity),
    )
    stokes.relation(
        "inlet_velocity",
        on=inlet,
        right=0, left=q.trace(velocity) + q.normal(q.isotropic_lift(inlet_profile)),
    )
    stokes.relation(
        "outlet_traction",
        on=outlet,
        right=0, left=q.normal(stress),
    )
    stokes.relation(
        "wall_velocity",
        on=walls,
        right=0, left=q.trace(velocity),
    )
    stokes.relation(
        "cylinder_velocity",
        on=cylinder,
        right=0, left=q.trace(velocity),
    )
    return source


if __name__ == "__main__":
    print(build_source().to_eqi(), end="")
