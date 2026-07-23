# Acceptance contract

For Q1 FEM and TPFA FVM independently:

- every forward state component agrees with a centred finite difference to
  `5e-7` relative plus `3e-10` absolute tolerance;
- every adjoint component for the method-native arithmetic-mean objective
  satisfies the same tolerance; and
- the adjoint directional contraction agrees with the forward contraction to
  `2e-10` relative plus `2e-12` absolute tolerance.
