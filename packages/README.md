# Model Packages

This directory contains ordinary, source-visible Eqiora Model Packages. A
package here receives no built-in name, search path, compiler branch, or trust
status. Verification resolves project-maintained and third-party-shaped
releases through the same exact content-addressed store contract.

Each `package.json` is a closed author manifest. It declares a canonical
package name and exact version, exact dependency identities, and the source
bundle inventory. Computed semantic and source digests remain release outputs;
they are not self-asserted fields in the author manifest. The source digest
covers that complete canonical manifest as well as the inventoried bytes, so
resolution-relevant alias spelling cannot collide in the content store.

The public Rust adapter may admit the manifest and exact inventory from one
explicitly retained directory capability. It reads only `package.json` and the
paths named there, without walking the tree or following post-root symbolic
links. The same source contract also accepts direct in-memory construction.
The public facade then derives semantic content with the ordinary compiler.
Given the caller-supplied complete exact dependency release closure, it derives
a lock from release identities and manifests and replays all source claims
under final exact namespaces before returning a release. This is an exact
input and release-preparation boundary, not filesystem discovery, an atomic
multi-file snapshot, or a publishing workflow.

The Rust seam is deliberately small:

```rust
# // Cargo.toml: eqiora = { features = ["package-filesystem"] }
use eqiora::package::{AuthorPackageDirectory, prepare_package_release_v1};

let sources = AuthorPackageDirectory::open_ambient(package_root)?.read_sources()?;
let release = prepare_package_release_v1(sources, &exact_dependency_releases)?;
```

Callers that already hold a sandboxed `cap_std::fs::Dir` use
`AuthorPackageDirectory::try_from_dir` and do not grant ambient authority to
the adapter. All three directory adapters in this document require the
opt-in `package-filesystem` facade feature; package identity, in-memory release
preparation, and exact resolution remain available without it.

Exact offline replay is a separate read-only boundary. The caller supplies the
already selected lock bytes and one explicit local-store capability; the
adapter never searches, installs, or updates a package:

```rust
use eqiora::package::{
    DirectoryPackageStore, PackagedModelDocument, ResolutionRecordV1,
};

let lock = ResolutionRecordV1::from_json(&lock_bytes)?;
let store = DirectoryPackageStore::try_from_dir(store_root)?;
let model = PackagedModelDocument::compile_locked(
    &store,
    &lock,
    "Main",
)?;
```

Package compilation always emits Eqiora's single current Model contract. It
has no artifact-generation selector. Persisted Model v1--v7 bytes are not
migrated by this path and reject when replayed as a current Model.

`DirectoryPackageStore::open_ambient` is the explicitly named convenience for
callers that choose to grant one root-path lookup. Both constructors retain the
opened root and read only `<source-bundle-digest>.json`, with no-follow,
nonblocking, regular-file, and bounded-allocation checks. The resolver still
revalidates every returned package identity and dependency edge.

Installation is a separate mutation-capable authority; it does not widen
`PackageStore` or select dependencies:

```rust
use eqiora::package::{DirectoryPackageInstaller, PackageStageCleanup};

let installer = DirectoryPackageInstaller::try_from_dir(installation_root)?;
let outcome = installer.install(&release)?;
if let PackageStageCleanup::Deferred(kind) = outcome.staging_cleanup() {
    eprintln!("package committed; staging cleanup was deferred: {kind:?}");
}
```

The installer canonicalizes the already prepared release before mutation,
creates a same-directory staging entry (mode `0600` on Unix), writes and
synchronizes the complete wire, closes it, then atomically adds
`<source-bundle-digest>.json` without replacement. Equal existing content
returns `AlreadyPresent`; an invalid or different occupant fails closed. The
must-use receipt reports deferred post-commit staging cleanup. The verified
contract covers one release at a time in a single-principal local store. It
does not update the exact lock, define shared-store permissions, claim
permission-based immutability, or make publication a package-selection
operation.

The initial library is intentionally small:

The Python distribution exposes `eqiora.vendor_standard_package(...)` for
copying `Eqiora.Fluid@0.3.0` or `Eqiora.Solid@0.3.0` and its exact dependency
closure into an ordinary local package project. It returns each vendored
release's semantic identity, source identity, and project-relative path for
the application manifest and `eqiora.toml`.

Top-level package directories retain the exact releases already consumed by
registered evidence. Later immutable releases live under
[`releases/<package>/<version>`](releases/) instead of rewriting those source
authorities in place. A dependency manifest always names the exact earlier or
later release identity; directory location never participates in selection.

- [`Eqiora.Electrical.Basic`](Eqiora.Electrical.Basic/) provides one scalar
  conserving connector and three ideal static components.
- [`Eqiora.Electrical.Circuits`](Eqiora.Electrical.Circuits/) depends exactly
  on `Basic` and exports a closed composed `ParallelDc` component with three
  typed scalar parameters.
- [`org.example.parallel`](org.example.parallel/) is a third-party-shaped
  analytic model used only to verify exact offline resolution, source
  re-canonicalization, hierarchy elaboration, and provenance.
- [`org.example.poisson`](org.example.poisson/) is a third-party-shaped scalar
  elliptic model used to verify exact package lineage through typed Realization
  and Run v2 artifacts.
- [`Eqiora.Electromechanical.DcDrive`](Eqiora.Electromechanical.DcDrive/)
  depends exactly on `Basic` and provides the bounded ideal linear motor and
  viscous-load components used by the sampled-drive evidence.
- [`Eqiora.Solid.LinearElasticity`](Eqiora.Solid.LinearElasticity/) provides
  an intrinsic-2D isotropic small-strain balance Component and a separate
  complete-exterior displacement/traction boundary Component over exact
  occurrence-bound Fields. Exact zero-displacement and zero-traction terminal
  Components add semantic boundary meaning without a numerical method. Its
  current exact release also adds first-order displacement/velocity dynamics
  and a velocity/traction interface over `Mechanics.Interfaces`; it owns no
  mass matrix, time method, or FSI policy. Immutable
  [`0.5.0`](releases/Eqiora.Solid.LinearElasticity/0.5.0/) adds only the
  corresponding three-dimensional dynamic law and interface.
- [`Eqiora.Solid 0.3.0`](releases/Eqiora.Solid/0.3.0/) is the standard starting
  point for 2D isotropic linear elasticity. It provides plane-strain and
  plane-stress models parameterized by Young's modulus and Poisson's ratio,
  typed material composition, composable Lamé-form parts, and fixed, free, or
  field-driven displacement and traction boundary conditions.
- [`Eqiora.Mechanics.Interfaces`](Eqiora.Mechanics.Interfaces/) provides one
  nominal power-conjugate velocity/traction boundary plus exact zero-velocity
  and zero-traction terminals. It is intentionally distinct from the solid
  package's displacement/traction virtual-work Connector. Immutable
  [`0.2.0`](releases/Eqiora.Mechanics.Interfaces/0.2.0/) adds exact 3D
  terminals without changing the connector or selecting a method.
- [`Eqiora.Mechanics.BoundaryLoads`](Eqiora.Mechanics.BoundaryLoads/) depends
  exactly on `Mechanics.Interfaces` and provides one normal-pressure terminal
  over a root-owned pressure Field. The package owns load meaning but no
  numerical boundary treatment.
- [`Eqiora.Fluid.Incompressible`](Eqiora.Fluid.Incompressible/) depends exactly
  on `Mechanics.Interfaces` and provides a steady incompressible Newtonian
  volume law plus a separate complete-exterior velocity/Cauchy-traction
  boundary Component. Packages select no mixed element, pressure constraint,
  scaling, solver, transfer, or coupling policy. Immutable
  [`0.3.0`](releases/Eqiora.Fluid.Incompressible/0.3.0/) adds a conservative
  transient 3D law and matching complete-exterior interface while retaining
  that separation.
- [`Eqiora.Fluid 0.3.0`](releases/Eqiora.Fluid/0.3.0/) is the standard starting
  point for 2D steady Stokes models. One import provides a curated model,
  separate balance and interface components, and no-slip, traction-free,
  normal-pressure, prescribed inward-normal-velocity, and field-driven vector
  velocity and traction boundary conditions.
- [`Eqiora.Fluid.InertialStokes`](Eqiora.Fluid.InertialStokes/) provides the
  distinct inertial incompressible Newtonian volume law used by the first
  fixed-reference FSI slice. It owns no boundary, time method, initial state,
  pressure reference, or coupling policy.
- [`org.example.dc-motor-control`](org.example.dc-motor-control/) is the
  third-party-shaped root that connects that drive to a proportional
  controller on one exact periodic clock.
- [`org.example.closed_circuit`](org.example.closed_circuit/) depends only on
  `Circuits` and verifies transitive exact component reuse without reaching
  through to `Basic`.

The `Basic -> Circuits -> org.example.closed_circuit` path is registered by
[`packages.composed-model-package`](../verify/packages/composed-model-package/README.md).
Its intermediate public component intentionally has no physical boundary
Ports. Cross-boundary connection-set union remains a separate semantic gate.

Store overwrite/deletion, lock update UX, filesystem project
discovery/walking, CLI authoring, workspace inference, multi-package atomic
transactions, staging garbage collection, directory-entry crash durability,
atomic multi-file snapshots, cross-platform runtime verification, registry
access, version-range solving, signing, build scripts, Python/Studio package
workflows (loading, authoring, or preparation), dynamic plugins, and
execution-provider packages are outside this package contract.
