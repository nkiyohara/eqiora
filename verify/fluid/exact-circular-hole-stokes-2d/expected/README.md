# Expected production observations

The production integration test reads the physical values directly from
[`../routes/python/result.json`](../routes/python/result.json). The independent
Julia route and [`../agreement/expected/agreement-report.json`](../agreement/expected/agreement-report.json)
establish that those values were independently reproduced before production
implementation.

The accepted quantities are five velocity probes, six pressure probes, signed
inlet and outlet fluxes, the cylinder constraint force on the fluid, complete
reaction/body/traction balance, the boundary-traction pressure reference, and
independently reapplied true and weak-continuity residual gates.
