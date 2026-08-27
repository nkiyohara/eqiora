# Python distribution candidate

This case closes one Python artifact family without turning package publication
into mathematical evidence. From one clean source commit it builds a source
distribution, extracts it, and builds four ordinary-GIL Linux x86-64 wheels
only from that extracted source.

The accepted wheel family is CPython 3.11 through 3.14 with per-interpreter ABI
tags and a `manylinux_2_17` floor. Every wheel is installed outside the source
tree and runs the base, NumPy ownership, synchronous/awaitable execution, and
cancellation tests plus a strict typed consumer. The CPython 3.13 wheel also
runs the exact PyTorch 2.13.0, JAX/JAXLIB 0.11.0, and Matplotlib 3.11.1
profiles and allowlist-free runtime/stub parity. Before any upload, the
published base quick start runs against every installed wheel, the framework
quick starts run against that same CPython 3.13 wheel, and the copied
exact-cylinder demo saves a headless pressure PNG from the Matplotlib profile.

The N1 acceptance predicate retains its pinned exact-cylinder Marimo host and
bounded process cleanup checks on CPython 3.13. No private Mesh or Trajectory
notebook viewer is part of candidate admission.

Normal profiles retain ordinary dependency resolution. A separate CPython
3.12 environment pins NumPy 2.1.0, replays the array-ownership/DLPack
falsifiers and base quick start, then records the observed dependency version.
This verifies the declared NumPy floor without pretending every dependency
version and interpreter form one tested cross-product.

An accepted v3 candidate manifest must bind the full source commit, optional
tags, tool versions, artifact filenames and SHA-256 values, wheel tags, passing
profiles, the closed Marimo-host dependency/browser identity, and a detached
candidate-bound H2 receipt. Its H2 predicate requires two clean locked host
validations to consume identical inputs and emit no generated product assets. That is
release provenance, not a claim that independent Python distributions are
byte-identical. This oracle commits no H2 PASS; the real receipt remains a
post-writer, pre-integration gate.

For each Notebook host scenario, success additionally requires the bounded
cleanup decision to observe the complete owned notebook, kernel, browser, and
profile-helper membership as empty. Cleanup begins from one monotonic epoch:
the existing host-status predicate still accepts status 0, or exactly
`-SIGTERM` after the candidate runner requested SIGTERM; unsolicited signals,
every other nonzero status, timeout, and forced kill reject. The cleanup
predicate is an additional conjunct, not a replacement for that status rule.
graceful shutdown and observation receive at most 30.0 seconds, and forced
escalation, reaping, and the final observation share only the remaining time
through the absolute 35.0-second decision deadline. Forced escalation always
rejects even if the later observation is empty. A primary host failure is
retained while cleanup adds its own terminal; cleanup is never skipped.

The focused decision oracle caps one scenario at 256 stable
`(role, PID, Linux start-time)` identities and 64 KiB of canonical UTF-8
diagnostics. A nonempty or incomplete observation, identity or output
overflow, authority denial, or deadline rejects. Same-name processes and a
reused numeric PID are not ownership authority. This is deliberately not a
promise that an uninterruptible, inaccessible, or incompletely observed
survivor disappears within a fixed time.

Execution is a direct three-stage handoff at one exact clean revision. The
candidate driver first prepares an immutable directory containing only one
sdist and the four wheels. The sole conventional H2 executor then safe-extracts
that retained sdist into two home-backed roots with disjoint homes, npm caches,
temporary, installation, browser-cache, and output paths; it publishes at most
one canonical receipt outside the family. Finalization consumes those unchanged
inputs, derives and validates the frontend manifest projection, runs every
installed-wheel profile, and retains exactly the manifest and unchanged receipt
as non-distribution metadata. It does not rebuild the family or synthesize H2.

Schema selection is derived before parsing from the safe-extracted sdist, all
four wheels, the manifest, and the requested profile. The retained Marimo host
harness, notebook profile/check, v3 format, or `build.frontend` member activates
v3. Only a complete family with the exact Marimo-host checks and canonical H2
receipt is accepted.

Read compatibility remains for a genuinely signal-free pre-N1 v2 family. It
does not permit relabelling or stripping one N1 field: any signal in any one
artifact activates the complete v3 predicate. The synthetic complete-v3 data
in the release-tool tests is only a schema and mutation oracle; it is not a
product asset, a retained candidate, or an H2 PASS receipt for Eqiora source.

Construction and inspection complete before validation fans out. Each profile
has a disjoint environment, consumer tree, temporary directory, and log under
home-backed scratch. A shared resource scheduler admits profiles within the
candidate lane's CPU and memory budget, serializes the memory-heavy framework
profiles, joins all admitted work, and merges immutable receipts in the order
listed by the case rather than completion order. The gate rechecks every wheel,
the sdist, and the extracted source identity before it writes the manifest.
This changes verification wall-clock and isolation, not the accepted artifact
or scientific claim.

This case and the JAX, PyTorch, and exact-cylinder pressure-still cases select
the same registered host target. One aggregate execution builds the candidate
once, then separately requires the base, typing, JAX, PyTorch, and Matplotlib
check groups from its manifest. Each case remains independently attributable
through the verification report; none may substitute an ambient wheel or an
independently rebuilt candidate.

## Falsifiers

The gate rejects:

- a dirty source tree or output written into the checkout;
- an incomplete sdist or a wheel built from the checkout;
- a host-native or wrong-interpreter wheel tag;
- absent PEP 639 license files, `NOTICE`, SBOM, `py.typed`, or public stubs;
- a PyTorch, JAX, or Matplotlib dependency leaking into the base dependency
  set;
- an import resolved from the checkout;
- a strict consumer, runtime/stub parity, framework, ownership, async, or
  cancellation failure;
- a missing required base, typing, PyTorch, JAX, or Matplotlib manifest check;
- any N1 signal hidden in a v2 family, including a host-harness,
  frontend-path-only, manifest-only, or requested-profile signal;
- a v3 manifest with a missing, extra, misspelled, or incorrectly typed
  frontend, runtime, browser, or Notebook-check member;
- a missing, noncanonical, wrong-preimage, internally incomplete, or
  cross-candidate detached H2 receipt;
- a dirty or different source revision, incomplete one-sdist/four-wheel family,
  shared H2 home/cache/output, checkout-sourced frontend, partial receipt, or
  command failure represented as a Notebook PASS;
- deletion or bypass of prepare, H2, finalization, frontend prerequisite, host,
  or browser commands; family mutation or rebuild after H2; receipt synthesis
  by the finalizer; or a receipt copied across source revisions or families;
- workflow stages without the exact prepare-family to H2 to finalization
  dependencies, disjoint family/receipt/metadata retention, or a publish barrier
  that consumes only the accepted distribution family;
- a non-registry or integrity-free lock node, changed lifecycle-script
  inventory, post-install external request, inconsistent locked inputs, or
  wrong exact browser identity;
- a public quick-start failure before upload or drift from the exact NumPy
  2.1.0 lower-bound profile;
- a validation profile mutating the shared sdist, wheel family, or extracted
  source, or any admitted profile failing before the joined manifest barrier.
- direct-host exit reported as success while an owned kernel, browser, or
  helper remains; skipped cleanup after a primary host-test failure; or forced
  escalation represented as success after later empty observation;
- nonempty or incomplete observation at the absolute 35.0-second decision
  deadline followed by an unbounded wait or success;
- signalling a same-name foreign process or a reused PID whose Linux start
  identity does not match the admitted owned identity;
- cleanup diagnostics that hide the primary failure or omit a stably observed
  survivor's role, PID, Linux start identity, state, requested stages,
  per-stage result, or authority-denied state; or
- a 257th stable identity or canonical diagnostic output beyond 64 KiB
  represented as a complete observation.

## Boundary

This case does not upload anything. A TestPyPI candidate additionally requires
maintainer release authority, a tagged commit, trusted publishing, installation
by exact uploaded version, and quick-start replay. macOS, Windows, other
architectures, musllinux, free-threaded CPython, `abi3`, GPU wheels, bundled
MPI, signatures, whole-distribution reproducible-build certification, and
production PyPI remain outside the claim. The frontend receipt does not turn
browser pixels into scientific evidence and adds no broader Mesh, Notebook,
operating-system, architecture, browser, or host support.

The governing process is
[`docs/development/python-release-policy.md`](../../../docs/development/python-release-policy.md).
