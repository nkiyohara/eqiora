# Exact-packaged sampled acausal DC drive

This case resolves three ordinary Model Packages offline and flattens their
typed components into one Model-v2 Relation network. The reusable drive
package depends exactly on `Eqiora.Electrical.Basic`; the third-party-shaped
root depends directly on both packages. No package is privileged by the
compiler or runtime.

The seven numeric Component bindings are typed lexical constants. They
specialize the accepted residual expressions without creating occurrence-local
Kernel Parameters or aliases; changing one literal therefore recompiles
immutable Model meaning rather than mutating hidden state.

The accepted model couples one electrical and one rotational conserving
domain through ordinary continuous residuals. Its continuous Newton system
contains differential and algebraic Fields, a continuously determined speed
signal, and every physical `Across`/`Through` coordinate. Original Relation
roots are followed by the deterministic junction roots from RFC 0024. The
system is 23-by-23 in both initial consistency and a backward-Euler step.

At model time zero, consistency is solved before the phase-zero 10 ms
controller tick; the controller update commits atomically and physical
consistency is restored before the first sample. The voltage output then holds
between exact ticks. `ClockDomain` is model meaning only: the registered Run
records the host execution topology and numerical step separately.

The numerical reference does not reuse Eqiora residual evaluation. It applies
the closed-form matrix exponential of the two-state linear motor/load system
over each held-input interval. The test compares current and angular speed at
0.1 s, requires the 1 ms backward-Euler error to be smaller than 62% of the
2 ms error, and enforces the declared absolute state bound.

Dimensioned physical samples then reaccept every component equation and all
three electrical/rotational junctions at a non-tick boundary. The evidence
checks electrical input power, copper and viscous loss, electromechanical
transduction, stored-energy change, and the nonnegative numerical dissipation
term introduced by backward Euler. It never labels that numerical term as a
physical loss.

The same test proves that:

- resolution node and edge insertion order cannot change canonical bytes;
- nested dependency alias spelling, source-file location, source-unit insertion
  order, and instance declaration order cannot change semantic package or
  Model meaning;
- a changed sample period changes package and Model meaning;
- execution-topology changes leave the Model identity unchanged and change
  the Run identity;
- the static affine physical specialization rejects this dynamic model;
- same-dimension but nominally different physical Domains fail before Model
  exposure;
- causal signal Ports cannot be presented as conserving Ports; and
- a deliberately incomplete run stopped by its semantic-step safety limit
  cannot reach package/Run binding construction.

Only after the analytic trajectory, residual, power, convergence, and package
checks pass does the case create an output-less `RunManifestV1` and its
`PackageRunBindingV1`. The binding is exact identity lineage, not execution
attestation. It is output-less because Eqiora does not yet have a durable
general trajectory artifact; physical observations remain an in-memory
reference result.

Run:

```bash
cargo test --locked -p eqiora --test packaged_dc_motor_controller
cargo run -p eqiora-verify -- run --case hybrid.packaged-dc-motor-controller
```

The evidence is limited to a scalar, ideal linear motor, viscous lumped load,
one proportional controller, one exact clock, host-serial `f64`, dense Newton,
and backward Euler. It does not claim switching, saturation, arbitrary DAE
index, Event/Guard composition, Stateflow, Simulink or Simscape
interoperability, code generation, fixed point, real-time scheduling, MPI,
GPU, a broad component catalog, or dynamic plugins.
