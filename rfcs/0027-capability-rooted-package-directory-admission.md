# RFC 0027: Capability-rooted package directory admission

- Status: Accepted; bounded v1 implementation
- Authors: Eqiora contributors
- Created: 2026-07-19
- Depends on: RFC 0022

## Summary

Eqiora may construct RFC 0022 `PackageSourcesV1` from one explicitly
supplied directory capability. The adapter reads only `package.json` and its
closed normalized inventory, without directory discovery, post-root symbolic
link traversal, or a second package meaning.

## Motivation

RFC 0022 deliberately begins with an admitted in-memory manifest and exact
file inventory. That is the correct semantic seam, but ordinary package trees
still need one safe route into it. Direct ambient `std::fs` calls spread across
the compiler would make authority, path interpretation, resource limits, and
concurrent mutation behavior implicit.

This RFC adds one concrete input adapter. It does not create a generic source
provider trait, package registry, workspace abstraction, or universal plugin.
A second provider family must demonstrate a real shared contract before such
an abstraction exists.

## Public API

The bounded Rust surface is:

```rust
pub struct PackageDirectory { /* retained capability */ }

impl PackageDirectory {
    pub fn try_from_dir(
        root: cap_std::fs::Dir,
    ) -> Result<Self, PackageDirectoryError>;

    pub fn open_ambient(
        root: impl Into<std::path::PathBuf>,
    ) -> Result<Self, PackageDirectoryError>;

    pub fn read_sources(
        &self,
    ) -> Result<PackageSourcesV1, PackageDirectoryError>;
}
```

`try_from_dir` acquires no ambient authority and validates that the supplied
handle denotes a directory. `open_ambient` is an explicit convenience
boundary: caller-selected ancestor resolution is ambient, while the root's
final component is opened without following a symbolic link. Once retained,
`read_sources` performs no ambient lookup and never consults the process
current working directory.

The adapter lives in `eqiora-package`. Semantic derivation remains in the
`eqiora-api` package facade; neither the compiler nor Semantic Kernel gains a
filesystem dependency.

## Exact read protocol

For one call to `read_sources`:

1. Open the fixed `package.json` final component handle-relative, no-follow,
   and nonblocking.
2. Require a regular file and reject its metadata length before allocating or
   reading beyond the manifest budget.
3. Decode the closed `PackageManifestV1` and use only its normalized inventory.
4. For each inventory path, open every intermediate component with
   `DirExt::open_dir_nofollow` and the final component with no-follow,
   nonblocking options.
5. Require a regular final file. Read through a fixed stack buffer, check the
   remaining budget before extending owned storage, and use a one-byte stack
   probe at the exact limit to distinguish EOF from growth.
6. Pass the manifest and owned bytes to `PackageSourcesV1`, which retains
   role, exact inventory, UTF-8 model-source, file-count, canonical-order, and
   aggregate validation authority.

No directory enumeration participates in the result. Unlisted files,
directory entry order, timestamps, permissions, and the retained root's former
ambient pathname cannot change the admitted bytes.

## Resource policy

V1 independently bounds:

| Resource | Limit and owner |
| --- | --- |
| Author manifest bytes | 16 MiB, directory adapter |
| One inventoried source file | 256 MiB, directory adapter |
| All inventoried source bytes | 256 MiB, directory adapter and source contract |
| Inventoried file count | 65,536, author manifest/source contract |

Metadata is only a preflight hint. A bounded manual read detects growth after
metadata without first growing the result vector past the accepted logical
limit. Allocation failure, root/entry I/O, nonregular entries, resource excess,
and package-contract rejection remain distinct typed errors.

## Portability amendment

Directory admission exposes host path aliases that an in-memory-only contract
could otherwise postpone. The v1 `NormalizedRelativePath` contract therefore
also rejects:

- Windows-reserved characters, including the NTFS alternate-stream colon;
- leading ASCII space and trailing ASCII space or period in a segment;
- Windows device stems `CON`, `PRN`, `AUX`, `NUL`, `COM0` through `COM9`,
  `LPT0` through `LPT9`, and the superscript-digit forms rejected by the
  retained directory implementation, including a stem with trailing space;
- two manifest entries that collide under ASCII case folding.

These are fail-closed admission changes in the pre-release package v1
contract. They do not change canonical bytes or digests for any previously
accepted maintained package fixture. Wider Unicode/filesystem equivalence is
not inferred from these checks.

The restrictions follow the
[Microsoft file naming contract](https://learn.microsoft.com/en-us/windows/win32/fileio/naming-a-file)
and the behavior of the pinned capability implementation. The workspace keeps
`cap-std` and `cap-fs-ext` 3.4.5 for the Rust 1.89 and current platform-support
line. The latest 4.0.2 API was reviewed, but upgrading that dependency is a
separate MSRV and platform audit.

## Concurrent mutation and identity

Retained root and component handles prevent a pathname replacement from
redirecting an already opened capability. They do not freeze file contents.
Concurrent write, truncate, hard-link mutation, or mutation between two file
reads may yield an error or a closed set of mixed-time bytes.

The adapter therefore claims no atomic multi-file filesystem snapshot. This is
safe for identity: RFC 0022 source identity covers exactly the owned manifest
and file bytes returned by this adapter, and compiler-derived preparation
replays those owned bytes rather than reopening paths.

## Verification

The registered `packages.offline-model-package` target must:

1. read `Eqiora.Electrical.Basic` and `org.example.parallel` through explicit
   directory capabilities;
2. produce the same admitted sources and release bytes as direct in-memory
   construction;
3. preserve the frozen package, resolution, Model, compilation, Run, and
   package-to-Run binding identities;
4. prove unlisted input is ignored and missing or nonregular inventory fails;
5. reject oversized manifest and source metadata before payload allocation;
6. on Unix, reject final and intermediate symbolic-link redirection and retain
   the original root across ambient pathname replacement.

Lower-level tests additionally cover a caller-supplied `Dir`, hardcoded
manifest symlinks, special-file nonblocking failure, injected aggregate-growth
limits, malformed manifests, invalid UTF-8, and every typed resource category.

## Nonclaims

This RFC does not define directory discovery or walking, `.gitignore`,
workspace inference, an atomic filesystem snapshot, editable vendoring,
lockfile generation, a CLI, registry/network access, version selection,
publication, signing/trust, build scripts, dynamic libraries or plugins,
Python/Studio package preparation, or cross-platform runtime verification.
