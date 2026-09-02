# Exact offline Model Package compilation

This case opens explicit directory capabilities for
`Eqiora.Electrical.Basic` and the third-party-shaped
`org.example.parallel`. The public Rust adapter reads only `package.json` and
the normalized closed inventory it names, producing the same admitted sources
and release bytes as direct in-memory construction. The public package
facade then derives each release's semantic content through the ordinary
compiler; this preparation operation accepts no author-supplied semantic
payload. It derives the exact lock from the two releases and inserts them
into the same in-memory content-addressed store, and compiles the locked root.
No package is privileged as standard or discovered by a search path.

The store-focused product checks publish freshly prepared releases into an
empty root through `DirectoryPackageInstaller`. Complete canonical
bytes are written to create-new staging entries (mode `0600` on Unix),
synchronized, closed, and atomically hard-linked to their exact source-digest
names. The read-only
store then replays the derived exact lock. Repeating an equal release is
idempotent. Replacing an
accepted entry afterward must fail the next ordinary replay with the exact
expected and actual source digests. This registered case is a single-principal
local-store proof; the lower-level contract separately forces a two-writer race
at the commit boundary.

After the explicit root is retained, all access is handle-relative. Every
post-root path component is opened without following symbolic links, final
opens are nonblocking, and regular-file and resource checks precede admission.
Unlisted files and directory enumeration order are irrelevant because the
adapter never walks the tree. The exact bytes actually read enter the ordinary
source identity; the operation does not claim an atomic snapshot across the
manifest and multiple source files. Concurrent mutation may therefore yield a
closed set of mixed-time bytes or an error, but it cannot produce an identity
for bytes other than those returned to source admission.

Release preparation receives a caller-supplied complete exact dependency
closure. It derives nodes from release identities and source digests and edges
from the closed manifests. Before returning the root release, it replays that
candidate under its final content-addressed namespace and re-derives every
dependency's semantic content from source. Missing, duplicate, unreachable,
or semantically dishonest inputs cannot return a root release.
The registered target also prepares a three-package root-to-middle-to-leaf
closure in both dependency input orders and requires identical root release
bytes and a three-node, two-edge exact lock.

The retained store path opens each requested final entry without following a
symbolic link and in nonblocking mode, rejects non-regular entries, checks
metadata before fallible allocation, caps the read, and probes one byte beyond
the bound to detect growth. Missing, digest-substituted, malformed, oversized,
special, redirected, or ambient-path-replacement entries fail closed before a
Model is returned. A read is not an atomic filesystem snapshot and still
requires the ordinary release and resolution identity checks.

Installation likewise requires no directory enumeration. The exact final name
is absent before publication; injected stage-write failure leaves no accepted
entry. Malformed or different occupied content, directories, symbolic links,
and a real FIFO fail closed through the existing bounded reader. This proves
one-release runtime atomic visibility, not a multi-package transaction or
power-loss durability of the directory entry.

Before elaboration, the application boundary parses every exact model source,
resolves package-local and direct imported declarations, and reconstructs the
compiler-owned canonical declaration set. It must equal each release's
semantic content exactly. Only then may the root `Main` model flatten through
the ordinary component hierarchy and cross the current Model and Transaction
owner.

The accepted compilation record binds the canonical Model digest to the root
identity, exact resolution digest, both source-bundle digests, and explicit
compiler/canonicalization versions. Definition spans for imported component
members identify `Eqiora.Electrical.Basic`; instance and binding spans identify
`org.example.parallel`, even though both packages may use the same relative
source path. The relation is checked; its incidental digest values are not an
expected lineage.

The resulting closed network is the same 14-by-14 static affine scalar
electrical problem used by the local hierarchy evidence. Independent faer
BiCGSTAB execution must recover 12 V at the high junction, 6 A and 3 A through
the two resistors, -9 A at the source positive terminal, zero ground potential,
and an accepted original-DAG residual norm.

Only after those analytic values, the original-DAG residual, and the
package-qualified source provenance are accepted does the case construct an
output-less `RunManifestV1`. The Run binds the canonical current Model digest
and semantic revision to the faer executor and exact execution, initial-guess,
and solver settings. A separate
`PackageRunBindingV1` binds that canonical Run identity to the accepted package
compilation record; the case round-trips the binding through its closed wire
format and replays it against the original resolution, Model, and Run.

The binding is a lineage edge, not execution evidence by itself. Execution is
supported here by the registered test's ordering and numerical acceptance. The
Run is deliberately output-less because the current v1 artifact family has no
general numerical-result format; import-provenance array DTOs are not reused as
result artifacts. A negative path supplies an invalid initial-state dimension
and confirms that solve failure cannot reach binding construction.

Run:

```bash
cargo test --locked -p eqiora --test offline_model_package
cargo run -p eqiora-verify -- run --case packages.offline-model-package
```

This evidence does not claim store overwrite/deletion, lock generation or
update UX, multi-package atomic transactions, staging garbage collection,
directory-entry crash durability, hostile same-root writers, filesystem
project discovery or walking, workspace inference, an
atomic multi-file filesystem snapshot, cross-platform runtime verification, a
package CLI, registry or network access, SemVer ranges, signatures, publisher
trust, package build scripts, modules,
wildcard/selective/re-exported imports, imported Model references, dynamic
plugins, execution-provider packages, Python or Studio package workflows
(loading, authoring, or preparation), a broad component library, transients,
hybrid control, MPI, CUDA, or a durable
public diagnostic-provenance wire. It also does not claim a typed Realization
for the current Model, Run v2, or a general numerical-result artifact.
