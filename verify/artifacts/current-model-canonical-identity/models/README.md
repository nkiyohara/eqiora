# Models

This case has no separate model source. Its subject is the wire identity of two
artifacts that are constructed in code:

- the frozen public fixture — one root length Parameter driving two endpoints of
  one 3D Cartesian Domain, recorded by exactly one `Domain --DependsOn-->
  Parameter` edge — built by `fixture()` in
  `crates/eqiora-artifact/tests/current_model_wire_oracle.rs`; and
- the re-encoded `examples/steady-flow-past-cylinder.model.json` resource, which is
  itself the artifact under test rather than an input to one.

The historical negative specimens under `../expected/historical/` are committed
bytes, not model sources. Nothing regenerates them.
