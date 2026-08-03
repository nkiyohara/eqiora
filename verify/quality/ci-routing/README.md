# Public pull-request CI routing

This case specifies the repository-owned public pull-request trust contract.
Opening or reopening a pull request, or synchronizing a new head, creates the
workflow, runs the change classifier and documentation contract, and reports
one aggregate `CI gate`. Opening a Draft runs the same contract immediately;
making its unchanged head Ready reuses those contexts without a duplicate run.
A conditional job may be skipped only when the reviewed path ownership says
its surface is irrelevant.

A separate `pull_request_target` workflow runs only protected-base code, reads
changed-file metadata, and rejects pull requests that modify workflows,
CI/release tooling, custom Actions, or CODEOWNERS. It never checks out or
executes head code. Legitimate trust-definition changes therefore require the
documented maintainer ruleset bypass and cannot approve their own replacement.

The classifier, aggregate predicate, and trust guard have dependency-free unit
falsifiers under `tools/ci/tests`. This case remains `specified` until the new
public repository has exercised both required contexts and its ruleset binds
them to the GitHub Actions provider.

Manual dispatch retains exact-commit full compatibility reproduction. There is
no scheduled suite, activity ledger, cost target, duplicate `main` push suite,
private runner, or merge queue claim.
