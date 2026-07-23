# Expected evidence

- Q1 FEM and TPFA FVM full-state directional sensitivities agree with
  independently recompiled centered differences.
- Their adjoint gradients with respect to both selected Domain bounds agree
  with independent objective differences.
- A fixed-area log-aspect optimization started at `s = 0.45` decreases the
  negative algebraic-mean objective and reaches the square neighborhood.
- Geometry or design-coordinate mismatches fail through typed validation;
  topology and cell counts remain fixed throughout each derivative.
