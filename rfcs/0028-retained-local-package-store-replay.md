# RFC 0028: Retained local package-store replay

- Status: Accepted; bounded v1 implementation
- Depends on: [RFC 0022](0022-exact-package-identity-and-resolution.md),
  [RFC 0027](0027-capability-rooted-package-directory-admission.md)

## Summary

Eqiora replays one existing canonical `ResolutionRecordV1` against a caller-
selected, read-only directory capability. The store reads only release bytes
named by exact source-bundle digest. The ordinary resolver revalidates the
release, semantic, source, dependency, and resolution identities before the
compiler may construct a Model.

This closes durable offline input and restart evidence without introducing a
workspace search path, package installer, registry, dynamic plugin, or second
package semantics.

## Motivation

RFC 0022 already defines the exact lock, content-addressed store trait,
resolver, and compilation sidecar. RFC 0027 admits author sources from one
explicit directory capability. The registered package case previously joined
those boundaries only through an in-memory release store. Consequently, the
wire and read-only directory store existed, but no registered path proved that
checked-in lock and release bytes could restart the same compilation and
accepted Run lineage.

The existing directory-store reader also predated RFC 0027's stricter I/O
discipline. A store entry is untrusted input. Opening a FIFO or other special
entry before a nonblocking regular-file check can stall a process, and
infallible capacity allocation is inconsistent with bounded package admission.

## Decision

### Existing semantic contracts remain authoritative

No lock or package wire changes. `ResolutionRecordV1` remains the sole exact
resolution record and retains its canonical JSON decoder, normalizer, digest,
root, nodes, and edges. `PackageReleaseV1` remains the only store payload.
`ExactResolver` and `PackagedModelDocument::compile_locked` remain the only
replay and compiler composition paths.

The checked-in lock may contain structurally equivalent JSON whitespace. The
closed decoder normalizes it, and all identity preimages use canonical output.
File placement and locator spelling never enter the resolution digest.

### Authority is explicit and retained

`DirectoryPackageStore` exposes two constructors:

```rust
DirectoryPackageStore::try_from_dir(cap_std::fs::Dir)
DirectoryPackageStore::open_ambient(path)
```

`try_from_dir` acquires no ambient filesystem authority. `open_ambient` names
its one caller-requested ambient lookup and opens the root's final component
without following a symbolic link. Both validate a directory and retain its
open handle. Later path replacement or rename cannot redirect a load.

`PackageStore` remains read-only. Authority to resolve packages does not imply
authority to install, overwrite, garbage-collect, or publish them.

### Reads are exact and bounded

For expected source-bundle digest `d`, the directory adapter may read only:

```text
<retained-root>/<lowercase-d>.json
```

It never enumerates the root, follows a fallback, or probes a package name or
version. The final entry is opened handle-relative, no-follow, and nonblocking.
It must be a regular file.

Metadata length is checked before allocation. Buffer reservation is fallible.
The read uses a fixed stack chunk and stops at the active caller/global limit,
then performs a one-byte probe so concurrent growth beyond the bound fails
closed without allocating an oversized vector. The in-memory store uses the
same fallible clone discipline.

Root, entry, non-regular, limit, allocation, package-contract, and digest-
collision failures remain typed and preserve their underlying error sources
where applicable.

### Replay revalidates meaning

Directory placement is only a locator. After bytes are read, the ordinary
resolver must still:

1. decode the closed release wire;
2. reconstruct and compare exact semantic package identity;
3. reconstruct and compare source-bundle digest;
4. compare manifest dependencies with locked edges;
5. verify graph closure, uniqueness, acyclicity, and reachability; and
6. let the compiler rederive canonical declarations from exact sources before
   Model mutation.

An entry stored under the wrong digest name, malformed release, unrelated file,
or replacement ambient path cannot become accepted package meaning.

### Concurrency boundary

One load binds the bytes actually returned from one opened entry. It is not an
atomic filesystem snapshot and does not prevent concurrent in-place mutation.
Mutation can yield one self-consistent release, a parse/identity failure, or a
bounded read failure; it cannot bypass resolver revalidation.

The registered two-package fixture is immutable project evidence. General
multi-file snapshot and writer concurrency semantics are deferred to the
separate installation contract.

## Verification

The existing `packages.offline-model-package` registered target additionally:

1. prepares both ordinary package releases through the public compiler seam;
2. requires the prepared release and exact lock canonical bytes to match the
   canonicalized content of the checked-in local-store fixtures;
3. drops the preparation values and decodes the checked-in lock;
4. replays from both an explicitly ambient-opened root and a caller-opened
   retained `Dir`;
5. reproduces the frozen release, resolution, Model, compilation, Run, and
   package-to-Run binding identities plus the analytic DC solution; and
6. rejects missing, digest-substituted, malformed, oversized, non-regular,
   special, symbolic-link, and path-replacement inputs while ignoring
   unrelated entries.

Low-level tests independently exercise the shared bounded directory reader and
the typed store errors.

## Alternatives considered

### Add a new lockfile type

Rejected. `ResolutionRecordV1` already is the exact canonical lock. A second
DTO would duplicate identity and validation semantics.

### Let the resolver open ambient paths

Rejected. Locator authority belongs to the adapter/caller. The resolver should
consume only the narrow `PackageStore` contract.

### Combine reads and installation in one store trait

Rejected for this slice. Read authority is enough for deterministic replay.
Atomic no-clobber installation adds temporary-file, durability, collision, and
concurrent-writer decisions and requires its own falsifying evidence.

### Introduce a universal resource or plugin registry

Rejected. Model Packages, execution providers, data adapters, and Studio
workflows have different payload and authority boundaries. This filesystem
adapter does not justify merging them.

## Compatibility

The package, resolution, Model, compilation, Run, and lineage wires are
unchanged. The pre-release Rust adapter replaces the ambiguous ambient `open`
constructor with `open_ambient` and adds `try_from_dir`. Store errors become
root/entry/resource typed. No published compatibility lifetime has begun.

## Nonclaims

This RFC does not define lock generation or update UX, store installation or
writes, archive extraction, directory discovery, workspace inference, a CLI,
registry or network access, version selection, garbage collection, signatures
or trust, an atomic multi-file snapshot, a broad cross-platform runtime claim,
Python or Studio package workflows (loading, authoring, or preparation),
execution-provider packaging, native code, or dynamic plugins.
