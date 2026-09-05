# Checking a claim

Start with [Capabilities](../capabilities.md), then consult the repository's
[`verify/`](https://github.com/nkiyohara/eqiora/tree/main/verify) manifests for
the selected claim, reference, command, and environment.

Run `cargo run -p eqiora-verify -- index` from a checkout for the current mapping,
then use `mise run fast -- --case <case-id>` to run a selected case.
