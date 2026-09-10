# Eqiora.Electromechanical.DcDrive

A deliberately small, ordinary Model Package for the packaged sampled-drive
verification case. It depends on `Eqiora.Electrical.Basic` by exact semantic
identity and adds one nominal rotational connector plus five reusable
electromechanical components:

- a signal-controlled ideal voltage source;
- a permanent-magnet DC motor with resistance, inductance, back EMF, and
  torque coupling;
- a viscously damped inertial load;
- an ideal non-loading speed sensor;
- the public `RotationalFlange` conserving connector.

The package has no privileged compiler path or motor-specific kernel node.
Its continuous component relations intentionally combine causal signals,
dynamic fields, and scalar conserving ports. They are fixtures for the general
mixed implicit-network execution seam, not a claim of a broad motor library,
switching electronics, saturation, or production component fidelity.

