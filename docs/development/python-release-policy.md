# Python distribution policy

This policy governs Python distribution artifacts. It does not widen Eqiora's
scientific or numerical claims, and a successful package upload is not
mathematical verification.

## Release identity

The Cargo workspace package version is the sole authored Eqiora version. The
public Rust `eqiora::VERSION`, private native-module version, public Python
`eqiora.__version__`, sdist, wheel metadata, and installed distribution all
derive from it. The first alpha maps Cargo `0.1.0-alpha.1` to normalized Python
`0.1.0a1`; Python metadata must not repeat an authored version. Publication
requires the exact derived tag, `v0.1.0a1`. A candidate records that expected
tag, the full source commit, and every source and wheel artifact by filename
and SHA-256, and rejects identity drift before publication.

The first wheel family is deliberately narrow:

- ordinary-GIL CPython 3.11, 3.12, 3.13, and 3.14;
- Linux x86-64 with a `manylinux_2_17` compatibility floor;
- one per-CPython wheel, because the NumPy C API is not an `abi3` claim.

macOS, Windows, other architectures, musllinux, free-threaded CPython, `abi3`,
GPU runtimes, bundled MPI, Conda, and system packages graduate independently
after installed-artifact evidence.

## Candidate construction and acceptance

A candidate is accepted only from a clean source commit. The release gate:

1. builds the source distribution;
2. rebuilds every wheel from that source distribution, not the checkout;
3. checks the declared wheel tags and metadata;
4. installs each wheel into an isolated environment outside the source tree;
5. runs the base, NumPy ownership, async/cancellation, typing, PyTorch, JAX,
   and Matplotlib profiles within their exact declared boundaries;
6. replays the public base quick start on every wheel and the public framework
   quick starts on the exact framework interpreter before upload;
7. verifies NumPy 2.1.0 separately on CPython 3.12 while retaining the ordinary
   latest-resolution profiles;
8. records source identity, artifact hashes, build-tool versions, the observed
   NumPy floor, and passing profiles in the candidate manifest.

Registered host evidence builds this complete candidate once for one source
commit and platform. The distribution, typing, PyTorch, JAX, and Matplotlib
validations then require their own closed check groups from that same manifest
and wheel family. The distinct evidence cases share one exact aggregate target,
so the verification runner may execute the target once while retaining a
report for each case. The focused adapter scripts remain developer diagnostics;
they are not a second candidate identity and may rebuild during standalone use.

The manifest is provenance for one artifact set. It is not a reproducible-build
claim, a signature, or evidence that another machine will produce identical
bytes.

Build into an empty directory outside the source tree:

```bash
python3 tools/release/python_candidate.py --out <candidate-directory>
```

The development-only `--skip-extras` mode may shorten iteration, but its
manifest is not eligible for upload. Add `--require-tag` at the publication
gate.

TestPyPI is the staging boundary. Upload uses a dedicated trusted-publishing
identity with the minimum OIDC permission and a protected release environment.
Hosted automation is transport for that upload, never the merge-quality gate.
The exact TestPyPI artifacts must be installed by version in clean
environments and replay the documented quick starts before promotion. A
TestPyPI upload, tag selection, or production PyPI release requires maintainer
release authority; ordinary feature work does not imply it.

## Dependencies and extras

NumPy is the only mandatory Python runtime dependency. Importing `eqiora` must
not eagerly import NumPy. PyTorch, JAX, and Matplotlib remain optional extras,
and importing the base package must neither require nor import any of them.
The first Matplotlib adapter uses exact release 3.11.1 with the headless Agg
backend. Its registered adapter profile runs on ordinary-GIL CPython 3.13;
the other wheel interpreters are not Matplotlib adapter compatibility
evidence. A wider compatibility range requires separate boundary evidence.

Dependency ranges are compatibility claims. Widening one requires an installed
wheel test at the new boundary; changing an exact framework baseline requires
its registered adapter evidence. The first candidate proves the NumPy 2.1.0
floor in one compatible installed-wheel environment; it does not claim every
NumPy version by CPython-version combination has been tested.

## Pre-1.0 compatibility and deprecation

Before 1.0, public Python names and behavior may change between minor releases,
but not silently within a released artifact. When practical, a renamed or
replaced public API remains available for at least one subsequent prerelease
and emits `DeprecationWarning` with the replacement. Immediate removal is
reserved for security, data-integrity, or scientifically incorrect behavior
that cannot be retained safely.

[RFC 0083](../../rfcs/0083-current-model-artifact-epoch.md) authorizes one
additional exception: the bounded pre-1.0 Model and Model Transaction
compatibility epoch reset. It names every removed surface, freezes the retained
persisted contract and its exact rejection behavior, requires migration of
every live consumer, and requires the break in release notes. This one-time
exception cannot reinterpret an old identifier, silently migrate bytes, or
authorize unrelated removals.

Persisted Eqiora artifact codecs and versioned control contracts follow their
own compatibility rules. A Python package version never authorizes silent
migration, wire sniffing, or reinterpretation of those bytes.

## Yanking and security

Yank a Python release when users should not select it for new installations but
existing exact pins may still need resolution. Grounds include:

- a security vulnerability;
- data corruption or scientifically incorrect accepted output;
- a broken or materially mis-tagged wheel;
- metadata that installs an incompatible dependency set.

Publish a corrected version instead of replacing files under an existing
version. Use the private reporting path in
[`SECURITY.md`](../../SECURITY.md) for suspected vulnerabilities. Deprecation
alone is not a reason to yank, and yanking is not deletion or revocation.

Production PyPI promotion remains a separate public-release decision.
