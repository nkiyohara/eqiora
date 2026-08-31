"""Author the steady-cylinder equations as one Eqiora Language Source."""

from eqiora import lang as q
from eqiora.lang import units as u


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

    dynamic_viscosity = stokes.parameter("dynamic_viscosity", unit=u.kg / (u.m * u.s))
    zero_pressure = stokes.parameter("zero_pressure", unit=u.kg / (u.m * u.s**2))
    inlet_speed = stokes.parameter("inlet_speed", unit=u.m / u.s)
    channel_height = stokes.parameter("channel_height", unit=u.m)

    velocity = stokes.field(
        "velocity",
        on=fluid,
        unit=u.m / u.s,
        shape=q.spatial_vector,
    )
    pressure = stokes.field(
        "pressure",
        on=fluid,
        unit=u.kg / (u.m * u.s**2),
        initial=0,
    )
    force_potential = stokes.field(
        "force_potential",
        on=fluid,
        unit=u.kg / (u.m * u.s**2),
        initial=0,
    )
    inlet_profile = stokes.field("inlet_profile", on=fluid, unit=u.m / u.s, initial=0)

    stokes.relation(
        "force_definition",
        on=fluid,
        residual=force_potential - zero_pressure,
    )
    stokes.relation(
        "inlet_profile_definition",
        on=fluid,
        residual=(
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
        residual=-q.div(stress) - q.grad(force_potential),
    )
    stokes.relation(
        "incompressibility",
        on=fluid,
        residual=q.div(velocity),
    )
    stokes.relation(
        "inlet_velocity",
        on=inlet,
        residual=q.trace(velocity) + q.normal(q.isotropic_lift(inlet_profile)),
    )
    stokes.relation(
        "outlet_traction",
        on=outlet,
        residual=q.normal(stress),
    )
    stokes.relation(
        "wall_velocity",
        on=walls,
        residual=q.trace(velocity),
    )
    stokes.relation(
        "cylinder_velocity",
        on=cylinder,
        residual=q.trace(velocity),
    )
    return source


if __name__ == "__main__":
    print(build_source().to_eqi(), end="")
