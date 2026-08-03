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

Normal profiles retain ordinary dependency resolution. A separate CPython
3.12 environment pins NumPy 2.1.0, replays the array-ownership/DLPack
falsifiers and base quick start, then records the observed dependency version.
This verifies the declared NumPy floor without pretending every dependency
version and interpreter form one tested cross-product.

The generated candidate manifest binds the full source commit, optional tags,
tool versions, artifact filenames and SHA-256 values, wheel tags, and passing
profiles. It is release provenance, not a claim that independent builds are
byte-identical.

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
- a public quick-start failure before upload or drift from the exact NumPy
  2.1.0 lower-bound profile;
- a validation profile mutating the shared sdist, wheel family, or extracted
  source, or any admitted profile failing before the joined manifest barrier.

## Boundary

This case does not upload anything. A TestPyPI candidate additionally requires
maintainer release authority, a tagged commit, trusted publishing, installation
by exact uploaded version, and quick-start replay. macOS, Windows, other
architectures, musllinux, free-threaded CPython, `abi3`, GPU wheels, bundled
MPI, signatures, reproducible-build certification, and production PyPI remain
outside the claim.

The governing process is
[`docs/development/python-release-policy.md`](../../../docs/development/python-release-policy.md).
