# Native HDF5 file-image import verification

This case composes the independent bounded XDMF metadata adapter with the
native HDF5 adapter at the public workflow boundary. The caller supplies one
complete HDF5 file image. Eqiora grants no path, directory, URL, network, VOL,
or filter-plugin authority; it opens the bytes through the Core VFD, fixes the
native VOL, disables plugin loading for the serialized operation, and audits
the complete reachable object graph before the first dataset read.

The admitted profile is a bounded acyclic hard-link tree containing groups and
internally stored, unfiltered, non-virtual datasets with exact standard `u64`
or IEEE `f64` types. All immutable requests are preflighted before any value is
returned. The public integration test imports, persists, and exactly replays
the mesh, fields, source identity, `eqiora.xdmf-hdf5.file-image` composition
identity, and runtime stack. An
independent caller-resolved import proves equal numerical artifacts but a
different manifest, because native runtime provenance is part of identity.

The hostile corpus falsifies soft and external links, filter pipelines,
external raw storage, virtual datasets, compound or unlinked committed
datatypes, attributes, and hard-link aliases/cycles. Every hostile image still
contains the complete admitted request set, so omitting the whole-file audit
would make the test succeed rather than fail for an unrelated missing dataset.
Mutation and configured budget excess are also rejected before verified
lineage is issued.

This evidence does not claim temporal XDMF, multiple HDF5 source locators,
export, implicit path resolution, safe coexistence with foreign in-process HDF5
clients, containment of a hostile process environment established before HDF5
initialization, or bounds on defects/internal work in the native library. Full
hostile-native containment requires a future isolated worker.

Run:

```bash
cargo test --locked -p eqiora --features hdf5 --test xdmf_hdf5_native_import
cargo run --locked -p eqiora-verify -- run \
  --case artifacts.xdmf-hdf5-native-import
```
