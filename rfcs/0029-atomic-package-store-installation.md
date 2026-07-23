# RFC 0029: Atomic package-store installation

- Status: Accepted; bounded v1 implementation
- Depends on: [RFC 0022](0022-exact-package-identity-and-resolution.md),
  [RFC 0028](0028-retained-local-package-store-replay.md)

## Summary

Eqiora installs one structurally valid `PackageReleaseV1` into one explicit
local store through a separate mutation-capable directory capability. Complete
canonical bytes are staged and synchronized before an atomic no-clobber hard
link publishes the source-digest entry. Installation stores untrusted content;
compiler admission and exact replay remain separate. The resolver and lock
remain unchanged.

This closes local release publication without turning the read-only
`PackageStore` into a package manager or a universal plugin registry.

## Motivation

RFC 0028 proves durable exact-lock replay, but intentionally leaves the store
read-only. Writing directly to `<source-digest>.json` would expose partial
bytes to a concurrent reader. Ordinary Rust rename replaces an existing
destination, which is incompatible with immutable content addressing. A
check-then-create sequence is also racy.

The Rust standard library documents that `rename` replaces an existing target,
while `hard_link` fails when the destination already exists and maps to
`linkat` on most Unix systems and `CreateHardLink` on Windows:

- <https://doc.rust-lang.org/std/fs/fn.rename.html>
- <https://doc.rust-lang.org/std/fs/fn.hard_link.html>

The package boundary needs the latter no-clobber property, same-directory
atomic visibility, and explicit failure when a filesystem cannot provide it.

## Decision

### Read and write authority remain separate

`DirectoryPackageInstaller` retains either a caller-opened `cap_std::fs::Dir`
or one root opened through the explicitly named `open_ambient` constructor. It
does not implement `PackageStore`. It cannot resolve, discover, select, delete,
overwrite, or update a lock.

`DirectoryPackageStore`, `ResolutionRecordV1`, `ExactResolver`, and
`PackagedModelDocument::compile_locked` retain their existing contracts.

### Canonical content precedes mutation

`install(release)` reconstructs the source-bundle digest and bounded canonical
release wire before creating a filesystem entry. Caller locator spelling,
staging names, permissions, and installation order never enter a digest.

Only a typed `PackageReleaseV1` can enter the public installation path. Raw
bytes, archives, and arbitrary filenames are not accepted.

`PackageReleaseV1` is a bounded content artifact, not a non-serializable proof
that one process invoked compiler-derived author preparation. Mirrors and
offline stores must be able to admit decoded release artifacts before choosing
whether to compile them. The installer therefore proves canonical bytes,
content identity, and atomic publication only. It does not confer semantic
trust. `prepare_package_release_v1` is the authoring path; every locked compile
still reconstructs and validates exact source semantics after loading. A
`PreparedRelease` wrapper at the storage layer would blur that authority and
would become meaningless after serialization.

### Publication is complete and no-clobber

The installer:

1. checks the exact digest entry through the ordinary bounded reader and
   returns `AlreadyPresent` without staging when its decoded release equals the
   input release;
2. creates one collision-resistant staging entry with create-new semantics in
   the retained root;
3. creates it private on Unix and never follows a final symbolic link;
4. writes and synchronizes all canonical bytes, then closes the staged file;
5. hard-links the staging file to `<source-digest>.json` without replacement;
   and
6. explicitly removes the staging name.

The stage and final entry are on the same filesystem because they share one
directory. A reader using the exact store contract observes either no final
name or a complete synchronized wire. The hard link never replaces an existing
directory entry.

On Unix the stage is created with mode `0600`, and the final hard link retains
that mode because both names identify one inode. V1 therefore targets a local
store used by the installing principal. Group/system-wide sharing and a
configurable final permission policy are separate contracts.

A failed post-commit staging cleanup does not turn a committed publication
into a pre-commit error. The must-use success receipt reports
`PackageStageCleanup::Deferred(error_kind)`, so a caller can diagnose the
residual state. The reserved staging name cannot match the exact digest grammar
and is never a resolver candidate. Garbage collection is a later authority and
policy contract.

### Existing content is classified through the reader

Before staging, and again after any failed hard-link attempt, the installer
clones only its retained root capability and uses
`DirectoryPackageStore::load_exact`. It decodes the existing release and
compares the typed `PackageReleaseV1` directly with the input release.

- same source digest and decoded release: `AlreadyPresent`;
- invalid, missing-after-conflict, non-regular, symbolic-link, oversized, or
  otherwise unreadable entry: typed failure;
- any different canonical content: `DigestCollision`.

There is no fallback overwrite, remove-and-retry, directory enumeration, or
trust based on the filename alone. Rechecking after every failed publish avoids
depending on one platform-specific `io::ErrorKind` to recognize a lost race.

### Concurrency and durability boundary

For cooperative concurrent writers sharing the root, equal installations
converge to one `Installed` and the remaining `AlreadyPresent` outcomes.
Different or dishonest occupied content fails closed. Another principal with
write authority can still rename staging or final entries; hostile same-root
mutation is outside v1.

`sync_all` precedes publication, so the linked file content has been submitted
to the filesystem durability boundary. V1 does not synchronize and prove the
parent directory entry across power loss. It claims atomic runtime visibility,
not crash-durable publication.

## Verification

Low-level tests prove:

1. the exact name is absent immediately before publication;
2. an injected staging-write failure never creates an exact entry and cleans
   the staging name;
3. equal repeats preserve exact canonical bytes and return `AlreadyPresent`;
4. two writers synchronized immediately before publication converge without
   replacement;
5. a commit-time cleanup failure is visible in the receipt and an equal retry
   is idempotent;
6. an equal or different occupant appearing after preflight is reclassified
   without replacement;
7. malformed and different occupied content fails closed; and
8. replacing the ambient root path cannot redirect publication.

The registered `packages.offline-model-package` case additionally:

1. decodes both checked-in exact release fixtures;
2. installs them into an empty explicit root;
3. proves installation without directory enumeration;
4. rejects occupied directory, symbolic-link, and FIFO entries without
   blocking; and
5. resolves the unchanged checked-in lock and reproduces the frozen package,
   Model, compilation, Run, binding, provenance, and analytic DC evidence; and
6. rejects a post-install substitution by its exact expected and actual source
   digests on the next ordinary replay.

## Alternatives considered

### Add write methods to `PackageStore`

Rejected. Read authority is sufficient for compilation and should not imply
mutation, cleanup, or publication authority.

### Write the final digest file with create-new

Rejected. It prevents overwrite but exposes a partially written final entry.

### Rename a staging file to the final name

Rejected for the portable v1 seam. Ordinary rename replaces an existing file;
platform-specific no-replace rename variants would require separate behavior
and fallback analysis.

### Use a universal package/provider/plugin installer

Rejected. Model Package releases, execution providers, data adapters, and
Studio workflows retain distinct payload and loading contracts.

## Compatibility

No wire, digest, lock, resolver, Model, compilation, Run, or binding contract
changes. The public Rust facade gains new types only. `PackageStore` remains
read-only and source compatible.

The runtime evidence is a local Linux filesystem. Other filesystems or targets
may reject hard links through a typed unsupported/publication failure rather
than weakening to partial or overwriting writes.

## Nonclaims

This RFC does not define lock generation/update UX, store overwrite/deletion,
staging garbage collection, crash-durable directory entries, hostile writers
sharing the root, atomic multi-package transactions, archive extraction,
package discovery, workspace inference, a CLI, registry/network access,
version selection, signatures or trust, broad cross-platform runtime,
permission-based immutability, network-filesystem behavior, Windows or macOS
runtime evidence, shared-principal store permissions, Python/Studio package
workflows, execution-provider packaging, native code, or dynamic plugins.
