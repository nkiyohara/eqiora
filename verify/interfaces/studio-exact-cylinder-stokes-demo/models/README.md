# Model fixture

The executable composition embeds the repository-owned
`examples/steady-flow-past-cylinder.model.json` and
`examples/steady-flow-past-cylinder.geometry.json` bytes. The accompanying
`.eqi` source is the readable authoring input from which this canonical example
was frozen. Replay requires the exact geometry-backed Model and exact geometry
source identity; an equal-named polygon or another Model revision is not
substitutable.

The readable `.eqi` file is executed separately in evidence and must reproduce
the accepted velocity, pressure, reaction, and balance observations. Its fresh
entity identifiers deliberately prevent a byte-identity claim between that
execution and the frozen current Model artifact. The Model resource's
canonical payload and current artifact digest are owned by
`artifacts.current-model-canonical-identity`; this application case does not
derive or retune them.
