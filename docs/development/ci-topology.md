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
   protected base revision. It reads changed-file metadata through the GitHub
   API and never checks out, imports, or executes head code. A change to
   workflow files, local CI/release tooling, the dependency policy, the
   layer/facade checks, the registered-evidence runner, custom Actions, or
   CODEOWNERS fails this context.

Changed-file pagination must match the provider-owned pull-request count.
Pull requests beyond the API's complete 3,000-file visibility boundary fail
closed instead of trusting a truncated list.

The main ruleset binds both contexts to the GitHub Actions provider, rejects
deletion and non-fast-forward updates, requires pull requests, and resolves
review threads. Repository Actions must be immutable-SHA pinned.

The trust guard intentionally cannot approve its own replacement. With one
maintainer, a legitimate trust-definition change therefore uses an explicit
owner ruleset bypass after the exact proposed tree has passed repository-owned
verification. This is an auditable bootstrap exception, not independent
review. Other pull requests cannot use that bypass. Once a second active human
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
- The hosted quality job validates the complete evidence registry and executes
  the exact `host-cpu` environment. A physical target remains visible as
  `not-selected`; it is never relabeled as a hosted success.
- Physical GPU/MPI evidence remains an explicit maintainer-run verification
  boundary. Running the unfiltered or exact physical case without its declared
  environment still fails closed.

## References

- [GitHub secure use reference](https://docs.github.com/en/actions/reference/security/secure-use)
- [Events that trigger workflows](https://docs.github.com/en/actions/reference/workflows-and-actions/events-that-trigger-workflows)
- [Rules available in rulesets](https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-rulesets/available-rules-for-rulesets)
- [PyPI trusted publishing](https://docs.pypi.org/trusted-publishers/using-a-publisher/)
