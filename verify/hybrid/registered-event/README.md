# Registered canonical event execution

This case proves the first complete path from canonical event meaning to a
production root proposal and back. A versioned root-registration artifact
links the immutable model digest, semantic revision, time-lowering digest, and
the complete partition of Event Activations. The runtime reconstructs one
scalar callback per structural guard group in the artifact's canonical order;
the backend never receives a bare root vector or assigns Activation meaning to
an index.

The bouncing-ball fixture has two reset Relations activated by the same
falling height guard. Diffsol localizes the first impact through the registered
callback. Eqiora checks the proposal's registration identity, solves the two
implicit `Next` Relations as one reset, derives the event-time and saltation
linearization, and explicitly restarts the unchanged ODE from the post-event
state. Impact time, pre/post velocity, saltation entries, and the subsequent
flight sample are compared with analytic values.

The wire round-trip rejects over-limit callback and Activation counts,
non-canonical ordering, and incomplete external linkage. A numerically valid
proposal carrying another registration identity also fails before reset.

This does not claim a complete hybrid scheduler. Distinct guards that localize
at the same time, periodic-tick coincidence, priority, general DAE events,
mode-dependent flow, checkpoint lineage, and Zeno policy remain separate
evidence gates.

Run only this case with:

```console
cargo run -p eqiora-verify -- run --case hybrid.registered-event
```
