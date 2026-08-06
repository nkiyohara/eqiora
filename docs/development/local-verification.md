# Local verification

Repository-owned local verification is Eqiora's technical acceptance
authority: a hosted service does not replace or broaden the evidence of a
locally closed slice. High-risk public deltas additionally wait for their
relevant provider-bound identity, trust, and execution contexts from
[the public verification topology](ci-topology.md) before merge. Hosted
contexts protect the submitted or integrated commit and gate definitions; they
do not become a second scientific acceptance authority.

The only ordinary executable gate entry points are `mise run fast` while
iterating and `mise run affected` before integration. Both depend on
`mise run setup`; in each new worktree its first gate installs the locked
Studio tree with `npm ci` before verification begins. `mise install` provisions
the declared developer tools. `mise.toml` owns this setup and invocation
ordering, but remains neither a dependency lock nor a second acceptance
implementation.

The repository-owned planner reuses the same path classification as CI and
includes committed merge-base changes, staged changes, unstaged changes, and
untracked files:

```bash
# Inner loop. Name semantic evidence explicitly.
mise run fast -- --case packages.hierarchical-physical-boundary

# Before integration: reverse-dependent Cargo closure plus affected clients.
mise run affected -- --case packages.hierarchical-physical-boundary

# Inspect the exact deterministic command plan without running it.
mise run plan

# Full local compatibility gate when the affected closure is unknown or a
# release/integration boundary requires it. This is not a calendar ceremony.
mise run periodic
```

Direct `python3 tools/ci/local_verify.py` execution is limited to an explicitly
setup-free `--plan` inspection or diagnostic and infrastructure debugging. It
is not an ordinary executable gate entry point; use the mise tasks for an
acceptance run so locked Studio setup cannot be skipped accidentally.

Every plan prints its selected paths, Cargo packages, registered cases, exact
commands, execution lanes, resource requests, and limitations. Execution is
shell-free. Commands remain ordered and fail-fast inside one lane; independent
lanes admitted by the CPU, memory, GPU, and named-lock budget overlap. A lane
failure does not cancel useful work already running elsewhere, and the final
captured logs and failures are reported in deterministic plan order. An absent
tool, unsupported environment, failed child process, malformed manifest, or
unknown path never becomes a silent pass.

The default budget uses the current host's detected CPUs and available memory,
with no GPU admission unless `EQIORA_LOCAL_VERIFY_GPU_SLOTS` declares it. A
constrained or shared machine can set `--cpu-slots`, `--memory-mib`, and
`--gpu-slots` explicitly. A lane whose own request exceeds the budget is
rejected before any child process starts. When the detected CPU budget exceeds
the sum of lane minima, the scheduler apportions the remaining Cargo build jobs
by those declared weights rather than throttling the longest lane to its
minimum.

A newly added directory under `verify/` is path-selected before manifest
discovery can name its case. Land its valid `case.toml` in the same change as
the first evidence files so `fast` and `affected` never plan an unknown case.

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

## Positive path and non-vacuity

An oracle or falsifier package counts as evidence only when its execution order
first proves at least one ordinary positive end-to-end path. Negative probes
then name the specific gate they target and demonstrate non-vacuity: the same
package must fail if an earlier unrelated denial makes the capability unusable.
A sandbox, parser, identity, or admission failure before the targeted boundary
cannot be reported as successful rejection of the intended mutant. If the
positive probe fails, the case fails; later negative outcomes do not rescue it.

Local verification executes the case manifest and reports its technical result;
a zero exit status cannot broaden a claim whose evidence package is vacuous.
Case review therefore checks the positive-path ordering and targeted denial as
part of the registered claim. The contract may require fail-closed behavior,
authority separation, or a resource envelope without requiring one OS or
allocator mechanism unless that mechanism is itself the claim.

Resource probes use raw input caps plus a deterministic,
implementation-independent abstract cost or step function. They do not sample
live allocation, queue depth, or worklist lifetime unless resource residency is
the declared public claim. When an oracle would become more complex than the
product seam or require a new OS trust mechanism, stop and simplify the outcome
contract or separate authority; never turn the mismatch into a passing gate.

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

Fast and affected collect those exact case IDs into one canonical, sorted
`eqiora-verify run` invocation. The runner still reports every semantic case
separately while executing each shared private execution key once. This batches
only work selected by one local-verification plan; it does not reuse a result
from another invocation or source revision.

Root Cargo, installed-Python, Studio, CubeCL, dependency policy, and lightweight
repository checks are separate local lanes. Every lane receives an explicit
build root and per-invocation temporary directory below
`~/.cache/eqiora/local-verify/<checkout>/`; the temporary directory is removed
after reporting. Build roots remain as recomputable Cargo products so a later
invocation does not pay for a clean rebuild. Set `--scratch-root` only to an
equivalent home-backed location. The runner never uses OS `/tmp` for these
artifacts.

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

## Heavy scientific candidates

Production-resolution gallery solves, refinement campaigns, and complete media
encodes are not ordinary pull-request or periodic verification. They use the
same three-authority boundary as the
[gallery contract](../verification/gallery/README.md#heavy-result-production):

1. **PR conformance** exercises the ordinary implementation path with bounded
   meshes, trajectories, witnesses, mutants, and renderer fixtures. Its narrow
   evidence does not accept or claim the production result.
2. **Exact-head scientific candidates** execute the full frozen campaign
   explicitly against one final source revision in the declared trusted
   environment. The claim, independent oracle, tolerances, stop conditions,
   affected registered cases, and invalidation inputs are fixed before the
   candidate runs.
3. **Immutable publication projections** consume the accepted Result or
   trajectory and reuse digest-verified admitted bytes. Publication never
   invokes the scientific solve.

The candidate records the exact Model, Geometry, mesh family, correspondence,
Realization, Run, fields and results, source revision, producer and runtime
environment, solver and library identities, oracle and evidence IDs, output
inventory, and content digests. It is eligible only for the affected claim at
that exact head. Equation, lowering, assembly, boundary-law, mesh-family,
time-integrator, solver-acceptance, scientific-observable, benchmark, or oracle
changes require a new full candidate on the final affected head. Renderer,
scene-profile, encoder, or accessibility changes may rerender the unchanged
accepted trajectory. Documentation, site-shell, and unrelated product changes
reuse the accepted result and admitted media.

Selection and failure are visible and fail closed. When the required heavy
environment cannot run, record the limitation and narrow the claim or stop its
promotion. `not-selected`, a stale bundle, a cache hit, or successful PR
conformance is never full scientific acceptance. Each scientific slice names
the exact affected registered cases; these rules do not infer semantic
invalidation from a broad path glob.

Actions, compiler, workstation, Vault, and site caches may accelerate or mirror
bytes but have no scientific or publication authority. The accepted receipt
and digests identify bytes retrieved from the first consumer's durable delivery
location. The current verification planner does not discover, schedule, cache,
publish, or transfer heavy candidates automatically, and this policy does not
define a provider, scheduler, archive, wire format, signing scheme, retention
policy, remote-execution API, or calendar cadence.
Nothing in this boundary weakens independent-oracle ownership, convergence or
refinement obligations, registered evidence, or the gallery's publication
predicate.

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

**Studio's locked npm dependencies are installed automatically; browser and
system dependencies are not.** Ordinary mise gates run their `setup`
dependency first and execute locked `npm ci` when the worktree needs it. A
missing Chrome executable or native system package remains an external
prerequisite and is reported as a limitation. `biome: not found` after an
ordinary gate therefore indicates skipped or invalid setup, not a lint result.

**A whole-file hash cannot verify `icon.icns` regeneration.** The pinned
Tauri CLI 2.11.4 may reorder the type-keyed ICNS chunks across identical
runs, so compare chunk type, declared length, and payload hash or decoded
pixels instead. Every other generated icon output remains byte-comparable.

**Run `tools/ci/python_package_gate.py`, not a hand-assembled `uv run`.** A
hand-written `uv run --with .` can answer from a cached wheel, so tests may miss
a Rust change that must be visible to Python. The repository gate does not have
this problem: it passes `--reinstall-package eqiora`, which rebuilds the source
tree even when its package version is unchanged.

## Hosted integration timing

Hosted waiting is proportional to durable risk. Scientific meaning or an
oracle, public or versioned API and compatibility, persisted schema or exact
artifact, security or data integrity, release or CI trust, governance, and
architecture changes wait for every relevant hosted check before merge.

A localized low-risk delta in the exact class defined by
[the public verification topology](ci-topology.md) may use the live auditable
owner/admin bypass after its exact-head mise gate, scope and DCO audit, and any
required review pass. Prefer completion of the base-owned `CI definition trust`
context. Record the bypass actor, reason, head, commands, and results; continue
already-running checks as post-merge signals and immediately assess repair or
rollback on failure. Normal and high-risk deltas wait for relevant required
contexts. This changes neither the required contexts nor the ruleset.

## Integration loop

After the affected gate and any applicable optional-provenance check pass,
record the exact commands and limitations in the pull request or issue, merge
the short-lived integration branch, push the accepted main commit, and delete
the merged branch. If a command fails, fix the same slice and rerun it; do not
create a standing status report.
