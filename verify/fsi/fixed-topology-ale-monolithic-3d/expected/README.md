# Acceptance contract

The numerical case advances one common initial state to `0.02 s` with step
widths `0.02`, `0.01`, and `0.005 s`. Successive final solid-displacement
differences are measured in one consistent tetrahedral P1 reference-mass norm;
they must decrease and their base-two observed order must exceed `0.70`.

Every accepted step must satisfy its nonlinear residual target, the
dimensionless weak-continuity gate, binary64-scaled solid-kinematic tolerance,
exact shared-interface velocity identity, interface action and power
acceptance, affine metric-identity defect below `1e-11`, positive current and
complete-path signed Jacobians, and the declared mean-ratio quality gate.
Every analytic Jacobian column is compared with centered complete-residual
reassembly under the evidence-owned scaled tolerance. Audited-column count,
color count, complete residual-assembly count, and maximum error are retained
in each accepted step; the private deterministic color pattern never depends
on analytic matrix values.

The constant-stream probe observes zero-trace MINI bubble rows and continuity
rows on moving fluid cells. Its accepted residual uses the evidence-owned
binary64 tolerance and its omitted-GCL witness must be strictly nonzero; this
is not a claim that a nonzero free stream satisfies the case's homogeneous
exterior boundary conditions.

`accepted-trajectory.json` is a regression fixture for the public projection
of the accepted medium trajectory. It is derived output, not an independent
physical oracle and not a cross-platform floating-point reproducibility
promise. Complete field blocks, including MINI bubble and solid velocity, are
validated in the content-addressed artifact graph before this smaller
human- and renderer-facing projection is built.
