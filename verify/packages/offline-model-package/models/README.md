# Models

The executable sources are the ordinary package trees at
[`packages/Eqiora.Electrical.Basic`](../../../../packages/Eqiora.Electrical.Basic/)
and
[`packages/org.example.parallel`](../../../../packages/org.example.parallel/).
The integration target opens each explicit root as a retained directory
capability and admits only `package.json` plus its closed inventory, so the
evidence cannot silently diverge from the reusable package content or acquire
content through directory discovery.

`resolution.json` is the exact two-package `ResolutionRecordV1` input used for
restart. `store/` contains the corresponding `PackageReleaseV1` wires under
their source-bundle-digest filenames plus one unrelated decoy. The test first
requires compiler-derived preparation to reproduce the canonical lock and
release bytes, then discards those values and replays the checked-in inputs
through retained read-only directory capabilities.

The same two release wires are decoded, canonicalized, and published into an
initially empty temporary store by the separate atomic installer before the
unchanged `resolution.json` is replayed. The checked-in fixture directory is
never mutated by that test.

The fixture layout is an evidence locator, not a package registry, publishing
format, or workspace search path. RFC 0029 defines installation of the release
wire itself; it does not make this fixture layout a distribution format.
