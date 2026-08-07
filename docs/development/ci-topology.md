# Public verification and trust topology

Eqiora's ordinary development authority remains repository-owned verification
run on the exact source under review. GitHub Actions reproduces that contract
for untrusted public pull requests and protects release identity; it does not
replace targeted local or physical-backend verification.

## Pull-request boundary

Every pull request reports two independent required contexts:

1. **CI gate** runs the untrusted head revision with read-only repository
   permission and no project secrets. The repository-owned classifier selects
   the affected Rust, MSRV, dependency, Python, Studio, and isolated-experiment
   jobs. Unknown paths fail closed to the complete surface.
2. **CI definition trust** runs under `pull_request_target` from the exact
   protected base revision. It reads complete changed-file metadata and only
   the bounded inert blobs needed for the exact ratchet class below through the
   authenticated GitHub API; it never checks out, imports, or executes head
   code. Except for that narrow class, a change to workflow files, local
   CI/release tooling, the dependency policy, the layer/facade checks, the
   registered-evidence runner, custom Actions, or CODEOWNERS fails this
   context.

Both workflows start on `opened`, `reopened`, and `synchronize`. Opening a
Draft therefore verifies its exact head immediately, while changing that same
head from Draft to Ready reuses the existing required contexts instead of
starting and cancelling an identical full run. A pushed head still starts a
new run, and the per-pull-request concurrency groups cancel only its stale
predecessor. This relies on GitHub's provider-owned event contract delivering
those activities for Draft pull requests and retaining commit-bound check runs
across a readiness-only transition. Repository tests pin the workflow side of
that boundary; live Actions observation owns the provider side.

Changed-file pagination must match the provider-owned pull-request count.
Pull requests beyond the API's complete 3,000-file visibility boundary fail
closed instead of trusting a truncated list.

The protected-base trust classifier may approve a coupled exact file-line
ratchet when the only changed protected path is
`tools/ci/architecture-debt.toml`, its only byte changes strictly lower
existing `[[file_lines]]` ceiling tokens, and protected-base code measures each
bound-head source at exactly the new ceiling. The source reduction and ratchet
remain in one pull request. This is a successful required trust check, not an
owner bypass: architecture review, exact-head mise gates, and every relevant
hosted context remain mandatory. Entries, limits, public surfaces, globs,
prose, paths, and every other protected change remain fail-closed.

The live main ruleset is active and strict, binds required `CI gate` and
`CI definition trust` contexts to the GitHub Actions provider, rejects deletion
and non-fast-forward updates, requires pull requests, and resolves review
threads. Its administrator enforcement is disabled (`enforce_admins=false`)
and it exposes the owner/admin pull-request bypass actor. Repository Actions
must remain immutable-SHA pinned. These are external live facts, not settings
this repository policy may silently mutate.

Normal integration and every high-risk delta wait for the relevant required
contexts. The trust guard intentionally cannot approve its own replacement, so
with one maintainer a legitimate trust-definition change retains its explicit
owner bootstrap bypass after exact-tree repository verification and required
fresh review. This is auditable bootstrap, not independent review.

The same existing owner/admin bypass may be used deliberately for no other
class except one of these localized low-risk deltas: a one-line non-protection
agent-capacity setting, non-governance documentation, reproducible mechanical
output, a private behavior-preserving refactor, or a localized correction to an
already-reviewed low-risk finding. Before bypass, the exact head must pass its
mise gate, scope and DCO audit, and any required review; the base-owned
`CI definition trust` context should be allowed to complete whenever possible.
Record the exact head, actor, reason, commands, and results in the pull request.
Continue already-running CI as a post-merge signal and immediately assess
repair or rollback on failure.

This route does not weaken, remove, or reconfigure required contexts or the
ruleset; it uses the live auditable bypass narrowly. Ambiguous risk, a change to
scientific meaning or an oracle, public or versioned API, persisted schema or
exact artifact, security or data integrity, release or CI trust, governance,
or architecture is ineligible and waits pre-merge. Once a second active human
maintainer exists, critical trust paths should require independent Code Owner
review.

## Change ownership

[`tools/ci/classify_changes.py`](../../tools/ci/classify_changes.py) is the
shared fail-closed surface map. Documentation-only changes avoid heavy product
jobs; changes to CI definitions or unknown paths select every surface.

Conditional jobs may be skipped only when their surface is irrelevant.
[`tools/ci/check_gate.py`](../../tools/ci/check_gate.py) validates the complete
relevance/result vocabulary and publishes the single `CI gate` context.

## Exact-commit reproduction

The same workflow retains an explicit manual mode for a release candidate or
CI-contract audit:

```bash
gh workflow run ci.yml --ref main -f commit=<full-lowercase-commit-sha>
```

Manual mode checks out exactly that object, rejects abbreviated or mismatched
SHAs, and selects the full compatibility matrix. A hosted run is evidence only
for the exact commit it reports. No schedule, activity ledger, or missed-run
monitor exists; add one only if a recurring measured failure requires it.

The separate `Windows compile probe` workflow is a manual portability
measurement outside the `CI gate` aggregation. It provisions the Microsoft MPI
SDK and runs the workspace-wide all-targets, all-features check without changing
pull-request runner consumption. Dispatch it from protected `main` only when its
result will choose a concrete portability slice; it has no schedule or standing
monitor.

## Release separation

Release workflows are small and separate from merge CI:

- candidate construction produces one source distribution, wheels, SBOM, and
  hash manifest from one clean source commit;
- TestPyPI publication uploads those exact bytes through OIDC in the protected
  `testpypi` environment;
- replay downloads the exact version, verifies every hash, and installs
  downloaded wheels while resolving dependencies from production PyPI;
- production promotion first authenticates a completed successful candidate
  workflow dispatched from protected `main` at the exact release commit, so a
  failed replay or unrelated workflow run cannot supply artifacts;
- publication accepts only that same verified artifact set, uses the protected
  `pypi` environment, and grants `id-token: write` only to its publish job.

Long-lived PyPI tokens are not repository secrets. Production is never rebuilt
after TestPyPI acceptance.

## Cost and security boundaries

- Pull-request jobs run only on GitHub-hosted ephemeral runners.
- No private developer, GPU, or HPC runner is attached to public Actions.
- Checkout credentials are not persisted.
- Fork pull requests receive no release environment or package credential.
- Dependency caches are added only after measured benefit and key-isolation
  review.
- The hosted quality job owns formatting, linting, workspace tests, dependency
  layers, the public facade, and rustdoc. Registered Cargo evidence and the
  Python installed-wheel candidate execute in two concurrent fresh-runner jobs
  so neither serializes the other's critical path. The Cargo job remains on
  the Rust surface. The Python candidate job belongs to both Rust and Python:
  Rust changes retain the complete evidence coverage formerly owned by the
  combined job, while Python-only changes exercise the installed-wheel claim.
- Hosted test and registered-evidence steps use Cargo's ordinary `test`
  profile with debug information disabled, incremental compilation disabled,
  and optimization level 1 because their target trees are disposable. Debug
  assertions and overflow checks remain enabled, and no relaxed floating-point
  mode is used. `tools/ci/local_verify.py` applies that profile to every command
  it runs, so a local gate reproduces the hosted one without an operator
  remembering a prefix; `HOSTED_TEST_PROFILE` in that file is the single
  definition and a contract test fails if it stops matching the workflow. A
  `cargo test` or `eqiora-verify` command invoked by hand outside the planner
  still needs the variables set explicitly, and the difference is not cosmetic:
  the registered preconditioner-scaling case takes 1150.8 s at Cargo's default
  `opt-level = 0` and 64.5 s at the hosted `opt-level = 1`.
- Each evidence job validates the complete registry, then intersects the exact
  `host-cpu` environment with either the `cargo` or
  `python-installed-wheel` runner kind. Targets outside either intersection
  remain visible as `not-selected`; they are never relabeled as hosted
  successes. The Python candidate path uses no MPI system packages because its
  default-feature wheel does not link MPI.
- Physical GPU/MPI evidence remains an explicit maintainer-run verification
  boundary. Running the unfiltered or exact physical case without its declared
  environment still fails closed.

## References

- [GitHub secure use reference](https://docs.github.com/en/actions/reference/security/secure-use)
- [Events that trigger workflows](https://docs.github.com/en/actions/reference/workflows-and-actions/events-that-trigger-workflows)
- [Rules available in rulesets](https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-rulesets/available-rules-for-rulesets)
- [PyPI trusted publishing](https://docs.pypi.org/trusted-publishers/using-a-publisher/)
