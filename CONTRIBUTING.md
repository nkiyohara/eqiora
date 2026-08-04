# Contributing to Eqiora

Thank you for helping build Eqiora. Contributions are accepted under
Apache-2.0 through the Developer Certificate of Origin process.

## Before starting

Read the [architecture summary](docs/architecture.md), the
[glossary](docs/glossary.md), and the
[contract-wave capability guide](docs/development/vertical-slice-development.md).

- Small bug fixes and documentation improvements may go directly to a pull
  request.
- New public concepts, semantics, persisted or wire formats, dependency-layer
  changes, and governance changes require an RFC.
- Separate the contract cell, implementation lane, and capability closure;
  reuse accepted contracts and applicable conformance kits instead of
  rebuilding them. Execution-provider tuples retain their exact evidence.
- Do not add a Semantic Kernel node for UI or adapter convenience. First show
  why a typed named subgraph cannot express the concept.
- Open an issue before substantial implementation so alternatives can be
  compared without duplicating work.

An implementation may optionally report machine-agent provenance under
[RFC 0068](rfcs/0068-optional-implementation-agent-attestations.md). It is not
a substitute for review, DCO, repository-owned verification, or registered
evidence, and omitting it does not block a contribution.

## Development

The default developer entry point is [mise](https://mise.jdx.dev/). It installs
the repository's Rust, Python, Node, npm, and uv versions without replacing the
language-specific lockfiles or acceptance gates:

```bash
mise install
mise run setup
mise run fast
mise run affected
```

Pass `--case <case-id>` after `--` to either verification task to forward
explicit semantic evidence to the repository planner. Use `mise tasks` to list
the Studio and full-compatibility entry points. Large verification scratch
remains home-backed under `~/.cache/eqiora`; mise tasks do not use OS `/tmp`.

The Rust tool remains the latest stable toolchain with `rustfmt` and `clippy`.
Public crates also support the workspace MSRV declared in `Cargo.toml`. The
[verification topology](docs/development/ci-topology.md) and
[local-verification guide](docs/development/local-verification.md) define the
affected and complete gates.

The complete Rust gate is:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets
cargo xtask check-layers
cargo xtask check-facade
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
cargo deny check
```

Run the narrower affected gate while iterating. Use the complete gate before
integration when the dependency closure is uncertain or a release,
compatibility, or trust boundary changes. Python, Studio, physical GPU/MPI,
and installed-artifact commands are documented beside their own contracts.

Tests should demonstrate the invariant, its failure mode, and transaction
rollback where relevant. Public failures use stable diagnostic codes rather
than bare strings.

Capability-changing pull requests must update the relevant row in the
[capability matrix](docs/capability-matrix.md). Contract, execution,
verification, and maturity are independent gates: implementation does not earn
a verification mark until a reproducible `verify/` case supports the exact
claim. Preserve explicit nonclaims and do not generalize from a bounded
fixture.

## Architecture rules

- Dependencies point from higher layers to lower layers.
- `eqiora-core` depends only on external crates.
- Standard Ontology schemas may depend on core but never alter kernel meaning.
- The reference interpreter remains small and unoptimized.
- Unsafe Rust is denied by default and must be isolated behind a reviewed
  boundary with a documented safety invariant.
- Generated files are committed, and verification detects schema drift.

## Developer Certificate of Origin

Every commit must be signed off:

```bash
git commit -s -m "Describe the change"
```

The sign-off certifies the
[Developer Certificate of Origin 1.1](https://developercertificate.org/).
Use your real name and an email address you are entitled to use. Maintainers do
not merge unsigned commits.

## Pull requests

- Keep commits reviewable and explain the invariant, not only the
  implementation.
- Link the relevant public issue or RFC.
- State the bounded claim, nonclaims, positive oracle, falsifier, and
  verification performed.
- Note environment-dependent evidence that could not be reproduced.
- Do not mix unrelated formatting or refactors into a semantic change.
- Review is technical and evidence-based; authority does not replace tests or
  conformance evidence.
