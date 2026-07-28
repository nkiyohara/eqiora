# Local verification

Repository-owned local verification is Eqiora's technical acceptance
authority: a hosted service does not replace or broaden the evidence of a
locally closed slice. Public pull requests additionally require the two
provider-bound identity and trust contexts described in
[the public verification topology](ci-topology.md) before merge. Those hosted
contexts protect the submitted commit and gate definitions; they do not become
a second scientific acceptance authority.

The repository-owned planner reuses the same path classification as CI and
includes committed merge-base changes, staged changes, unstaged changes, and
untracked files:

```bash
# Inner loop. Name semantic evidence explicitly.
python3 tools/ci/local_verify.py fast \
  --base origin/main \
  --case packages.hierarchical-physical-boundary

# Before integration: reverse-dependent Cargo closure plus affected clients.
python3 tools/ci/local_verify.py affected \
  --base origin/main \
  --case packages.hierarchical-physical-boundary

# Inspect the exact deterministic command plan without running it.
python3 tools/ci/local_verify.py affected --base origin/main --plan

# Full local compatibility gate when the affected closure is unknown or a
# release/integration boundary requires it. This is not a calendar ceremony.
python3 tools/ci/local_verify.py periodic
```

Every plan prints its selected paths, Cargo packages, registered cases, exact
commands, and limitations. Execution is shell-free and fail-fast. An absent
tool, unsupported environment, failed child process, malformed manifest, or
unknown path never becomes a silent pass.

The verification planner establishes patch correctness evidence; an optional
implementation-agent identifier is separate provenance. When the final
pull-request body supplies one, run the protected-base check:

```bash
python3 tools/ci/check_implementation_agent.py \
  --base origin/main \
  --pr-body-file /path/to/pr-body.md
```

The checker accepts an omitted, empty, or exact `not-supplied` field. Otherwise
it resolves the content-derived identifier only against the registry in the
merge base, verifies score and expiry, and rejects malformed, unknown, stale,
or candidate-added entries. See [RFC
0068](../../rfcs/0068-optional-implementation-agent-attestations.md).

## Tiers

`fast` runs formatting, direct changed-package tests and Clippy, explicitly
named or directly changed verification cases, and only the relevant
documentation, CI-contract, or dependency-layer checks. It is the ordinary
edit loop.

`affected` expands direct Cargo packages through the complete workspace
reverse-dependency closure, adds Rustdoc, and conservatively selects registered
cases changed under `verify/` plus cases named explicitly with `--case`. It
checks the complete registered manifest inventory once, but never treats an
evidence executor crate as semantic ownership: a facade test package may serve
many unrelated cases. It also uses the existing change-surface classifier to
add Python, Studio, dependency-policy, or isolated-experiment checks. A
fail-closed infrastructure or unknown-path classification selects the complete
Rust workspace.

An evidence package is an executor, not semantic ownership. Therefore callers
must pass every semantically affected case with `--case`; only direct changes
inside that case's `verify/` directory select it automatically.

Fast and affected Clippy use default features. Optional MPI, CUDA, Diffsol, or
other backend features run through their explicitly selected evidence command
or a matching environment-specific check. Only the manual `periodic` gate asks
the current machine to compile the complete all-feature workspace.

`periodic` runs the full current-machine workspace, all registered cases, MSRV,
dependency policy, Python, Studio, and isolated-experiment commands. Use it for
an unknown affected closure, a broad integration/release boundary, or an
exception-triggered process investigation—not on a standing calendar.

The public-facade check compares `crates/eqiora/src/lib.rs` with the exact
stable and transitional inventory in `api/eqiora-facade-v1.json`. Fast and
affected plans select it whenever the facade, inventory, or checker changes;
the periodic plan always runs it. A stable glob or unregistered public export
is a gate failure, not an implicit API expansion.

When Python is selected, the planner creates one ephemeral virtual environment,
installs the declared build requirements and pinned test tool, builds and
installs the project non-editably, and runs pytest through that same
interpreter. It prefers `uv run --isolated --no-editable` and falls back to a
standard-library virtual environment. The environment is removed when the gate
ends. It never mutates an externally managed system interpreter or mixes a
project-discovered development environment with a different pytest
interpreter.

## Environment boundary

The planner states evidence that the current machine cannot generalize:

- one local Python interpreter is not the complete supported-version matrix;
- one-host MPI is not physical multi-node evidence;
- a build without a matching GPU and driver is not physical CUDA evidence; and
- Studio native/browser checks require the documented system dependencies.

Run and record a matching environment-specific command when a slice changes
one of those claims. Do not remove the limitation merely because a conditional
test returned successfully.

## Local environment gaps and misleading substitutes

The gate reports missing local prerequisites directly. Record the exact failed
stage instead of treating an incomplete run as a clean gate, and do not replace
the gate with a hand-assembled command that measures a different artifact.

**Studio's end-to-end suite needs a browser the runner does not have.**
Playwright expects Google Chrome at `/opt/google/chrome/chrome`, and
`npx playwright install chrome` needs root. The gate reaches this stage after
every other Studio browser stage has passed. Say which stage failed in the
change rather than reporting a clean gate, and rely on CI for that stage.

**Studio's own dependencies are not installed by the gate.** Without
`npm ci` in `studio/`, verification stops earlier still, at `biome: not found`.
That failure looks like a lint error and is not one.

**A whole-file hash cannot verify `icon.icns` regeneration.** The pinned
Tauri CLI 2.11.4 may reorder the type-keyed ICNS chunks across identical
runs, so compare chunk type, declared length, and payload hash or decoded
pixels instead. Every other generated icon output remains byte-comparable.

**Run `tools/ci/python_package_gate.py`, not a hand-assembled `uv run`.** A
hand-written `uv run --with .` can answer from a cached wheel, so tests may miss
a Rust change that must be visible to Python. The repository gate does not have
this problem: it passes `--reinstall-package eqiora`, which rebuilds the source
tree even when its package version is unchanged.

## Integration loop

After the affected plan and any applicable optional-provenance check pass,
record the exact commands and limitations in the pull request or issue, merge
the short-lived integration branch, push the accepted main commit, and delete
the merged branch. If a command fails, fix the same slice and rerun it; do not
create a standing status report.
