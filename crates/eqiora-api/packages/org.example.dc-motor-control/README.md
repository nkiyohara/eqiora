# org.example.dc_motor_control

A third-party-shaped root Model Package for one linear permanent-magnet DC
drive under exact periodic proportional control. Both dependencies are direct:
`Eqiora.Electrical.Basic` supplies the electrical ground, while
`Eqiora.Electromechanical.DcDrive` supplies the actuator, motor, mechanical
load, sensor, and rotational connector family.

`SampledPController` is intentionally local to this package. Its 10 ms clock
is model-time meaning; numerical step size, worker placement, and execution
scheduling remain separate run or realization concerns.

The `drive` dependency in `package.json` names the exact semantic digest of
`Eqiora.Electromechanical.DcDrive@0.1.0`. Resolution is offline and
fail-closed: an altered package, digest, or dependency edge is rejected before
model compilation.
