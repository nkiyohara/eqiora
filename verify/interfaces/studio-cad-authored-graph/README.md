# Native Studio authored-CAD graph projection

This case registers one bounded Studio application surface over the accepted
authored-CAD owner. The workspace constructs exactly a closed rectangle face
and positive-z extrusion, optionally followed by the one accepted strictly
interior circular through-cut. Native Rust alone validates and constructs the
graph, then projects its exact canonical graph identity, observations,
graph-bound face handles, and complete analytic build receipt.

The frontend admits only the independently versioned
`eqiora.studio.cad-authored/v1` presentation protocol. It checks bounded
opaque encodings and structural echo coherence, displays the complete four- or
eight-operation history, and sends a selected opaque handle back to native
Rust. It does not calculate a digest, clearance, bound, area, volume, lineage,
or tolerance substitution. Browser preview returns an explicit native-only
failure instead of canned scientific values.

The numerical and semantic observations are not new oracles. They remain
owned by `geometry.cad-authored-rectangle-extrusion` and
`geometry.cad-authored-circular-through-cut`; this case projects those already
accepted values without retuning them. The registered Cargo executor replays
the cut case, including its rectangle predecessor. Studio's native adapter and
frontend protocol/session tests are exercised by the repository Studio gate.

Run:

```bash
cargo run --locked -p eqiora-verify -- run --case interfaces.studio-cad-authored-graph
cargo run --locked -p eqiora-verify -- run --case geometry.cad-authored-rectangle-extrusion
cargo run --locked -p eqiora-verify -- run --case geometry.cad-authored-circular-through-cut
npm --prefix studio run check
npm --prefix studio test
cargo test --manifest-path studio/src-tauri/Cargo.toml --locked cad_authored
```

The claim does not include a general feature DAG, pure output-Geometry digest,
tessellation or rendering, mesh, Model binding, solver, Python surface,
Studio-to-Python export, provider choice, imported CAD, or healing.
