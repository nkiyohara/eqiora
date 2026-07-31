# Current authoring and exact codec conformance

This case separates the ordinary authoring profile from immutable artifact
history. Rust `ModelDocument::compile` and `define`, Python `compile` and
`Model.define`, and Studio's authoring request all select the same current
semantic vocabulary without accepting a codec argument from the user.

The shared `expected/profile.json` fixture fixes the cross-client mapping for
this revision. It is a conformance input, not a promise that `current` will
remain Model v8. Exact compatibility operations continue to name v1 through
v8, replay only the selected codec, preserve the artifact schema and digest,
and reject unknown or mismatched generations without fallback. Quantitative
edits retain that exact transaction codec and reconstruct a child Model in the
same generation, including ordinary current-v8 Studio authoring. The current
fixture contains a generic content-addressed pure operator, so exact v4 rejects
it instead of silently dropping its definition table.

The registered Rust test owns the ordinary/exact boundary and falsifiers.
Installed-wheel Python and native/TypeScript Studio tests consume the same
fixture as companion client-adapter checks.

Run:

```bash
cargo test --locked -p eqiora --test current_authoring_profile
cargo run --locked -p eqiora-verify -- run --case interfaces.current-authoring-profile
```
