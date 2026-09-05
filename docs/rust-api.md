# Rust API

Use the `eqiora` crate as the application entry point. It exposes the same
canonical Rust implementation used by the Python SDK; language-specific
convenience APIs are not necessarily one-to-one.

## Install the alpha

Initial crates.io publication is in progress. The commands below target the
release version and become usable after the facade and its dependencies are published.

In a new Cargo project:

```console
cargo add eqiora@=0.1.0-alpha.7
```

Or add this dependency to `Cargo.toml`:

```toml
[dependencies]
eqiora = "=0.1.0-alpha.7"
```

The initial source-distribution smoke check targets Linux x86-64 with the
default features. The workspace declares Rust 1.89 as its minimum supported
version; the release packaging check uses Rust 1.98.0. Source distributions
require a Rust toolchain and linker. This does not establish support for every
platform or optional native backend.

## Compile a model

Put this in `src/main.rs`, then run `cargo run`:

```rust
use eqiora::api::ModelDocument;

fn main() {
    let model = ModelDocument::compile(
        "decay.eqi",
        r#"model decay {
            field x: 1 = 1;
            parameter rate: 1 / s = 1;
            relation flow continuous {
                derivative(x) + rate * x = 0;
            }
        }"#,
    )
    .expect("the model must compile");

    println!("model digest: {}", model.digest().expect("canonical digest"));
}
```

This example compiles and validates a model and reports its canonical identity.
It does not integrate the ODE or assert a numerical solution. See the
[capability matrix](capability-matrix.md) for the exact supported execution
paths and their independently registered evidence.

## Optional features

The default feature is `package-filesystem`, which enables filesystem-backed
model-package operations. Existing feature boundaries are retained:

| Feature | Purpose and boundary |
| --- | --- |
| `rayon`, `faer` | Optional threaded CPU and linear algebra integrations. |
| `gmsh` | Gmsh-format facade exports; disabling this feature does not remove every transitive Gmsh-format dependency. Automatic Gmsh meshing also requires its external executable. |
| `vtu`, `xdmf`, `hdf5` | Optional data-format operations; `hdf5` enables `xdmf` and the native HDF5 dependency. |
| `cad-truck` | The bounded Rust-native CAD adapter. |
| `diffsol` | Optional adaptive integration backend. |
| `mpi`, `cuda`, `mpi-cuda` | Environment-specific distributed/GPU adapters requiring their matching native setup and evidence. |

Enable only the features needed by the chosen supported path, for example
`eqiora = { version = "=0.1.0-alpha.7", features = ["faer"] }`.
Default-feature packaging is not verification of these optional environments.

## Compatibility

This is an alpha, pre-1.0 API. Breaking corrections may appear in subsequent
releases; pin the exact version when reproducing a result. Current Rust and
Python APIs converge together without retaining obsolete aliases or compatibility
shims unless an explicit stable interoperability promise requires them.
Published release artifacts remain historical records. See the
[pre-1.0 policy](development/ai-authored-platform-strategy.md#pre-10-api-convergence).
