# Current authoring and replay conformance

This case fixes one current-only authoring profile. Rust
`ModelDocument::compile` and `define`, Python `compile` and `Model.define`, and
Studio's authoring request all select the same current semantic vocabulary
without accepting a wire or codec argument from the user.

The shared `expected/profile.json` fixture fixes the cross-client mapping for
this revision: the current Model and Transaction schemas are v8. It is a
conformance input, not a promise that the `current` profile will always use
those schema identifiers. Source authoring, native definition, a quantitative
edit, current artifact replay, and control-v2 compilation all traverse the one
current owner and preserve their typed identity relations.

Historical v1--v7 bytes are negative specimens only in
`artifacts.current-model-canonical-identity`. No historical codec selector,
decoder, fallback, or silent migration is part of this case.

The registered Rust test owns the current-authoring/replay boundary.
Installed-wheel Python and native/TypeScript Studio tests consume the same
fixture as companion client-adapter checks.

Run:

```bash
cargo test --locked -p eqiora --test current_authoring_profile
cargo run --locked -p eqiora-verify -- run --case interfaces.current-authoring-profile
```
