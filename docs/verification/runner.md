# Verification runner

`eqiora-verify` turns the contracts below `verify/` into one deterministic
case index. Discovery, contract validation, evidence execution, and rendering
are separate phases; listing and checking never run numerical evidence.

The manifests and adjacent case READMEs are the source of truth for current
claims and non-claims. Project summaries link here instead of copying an
append-only capability list. A case enters the index only through a validated
`verify/<area>/<case>/case.toml` contract.

## Commands

```bash
# Compatibility gate: check everything, then run executable evidence fail-fast.
cargo run -p eqiora-verify -- verify

# Stable ID order, with no evidence execution.
cargo run -p eqiora-verify -- list

# Manifest-derived capability-to-evidence index (all cases are validated first).
cargo run -p eqiora-verify -- index
cargo run -p eqiora-verify -- index --capability convergence

# Validate manifests, declared artifacts, Cargo targets, and Python gate paths.
cargo run -p eqiora-verify -- check
cargo run -p eqiora-verify -- check --case solid.axial-bar

# Run all or one case. Fail-fast is the default.
cargo run -p eqiora-verify -- run
cargo run -p eqiora-verify -- run --case numerics.poisson-fem-fvm
cargo run -p eqiora-verify -- run --case numerics.cartesian-poisson-fem-fvm
cargo run -p eqiora-verify -- run --case numerics.cartesian-poisson-3d-fem-fvm
cargo run -p eqiora-verify -- run --case time.general-implicit-dae
cargo run -p eqiora-verify -- run --keep-going

# Validate the complete repository, then execute only host-CPU evidence.
cargo run -p eqiora-verify -- run --environment host-cpu
```

`case.toml` is also the only source for the capability-to-evidence index.
`index` projects each declared capability to its case ID, manifest, maturity,
reference strategy, and structured evidence target. It never scans prose or a
second hand-maintained registry. Entries are ordered by capability and case;
an exact filter still validates the complete repository before selection.
Cases may also declare stable, deduplicated `conformance_kits` consumed by two
or more downstream cases. The index exposes those kit IDs alongside every
capability entry; unknown manifest extensions cannot impersonate this typed
field.

`--format json` is global and may appear before or after the subcommand. JSON
stdout contains exactly one `eqiora.verification-report/v4` object for
`list`, `check`, and `run`, or one
`eqiora.capability-evidence-index/v3` object for `index`. Version 4 adds
`duration_ms` for each evidence target that started, measured with a monotonic
clock and reused by every case sharing that target. The field is absent for
cases whose target did not start or did not execute. Version 3 added the
selected evidence environment and the closed `host-cpu` /
`physical-mpi-cuda` target distinction; host-CPU target JSON retains its
previous shape. Human output appends the same whole-millisecond duration to
each executed case. Captured child stdout and stderr are stored in the
corresponding case fields and are never interleaved with the report:

```bash
cargo run -q -p eqiora-verify -- run --format json > report.json
cargo run -q -p eqiora-verify -- index --format json > capability-index.json
```

The process exits successfully only when repository validation succeeds, the
case filter resolves, and every selected evidence target passes. An explicit
environment selection still validates every manifest and artifact before
execution; targets for another environment remain visible as `not-selected`.
Without `--environment`, every executable target remains selected, so an
absent physical runtime still fails rather than silently skipping. In
fail-fast mode, later selected executable cases remain in the report with
outcome `skipped`.
`proposed` and `specified` cases are reported as `not-runnable`; `implemented`,
`verified`, and `validated` cases must declare and pass an evidence target.

## Trust boundary

A manifest cannot provide a shell fragment, working directory, or free-form
arguments. Its `[evidence]` table accepts one of two closed forms.

Workspace Cargo integration test:

```toml
[evidence]
package = "eqiora-numerics"
test = "poisson_fem_fvm"
table = "expected/convergence.csv" # optional
```

Evidence that can only be repeated on a selected physical topology declares
that environment explicitly:

```toml
[evidence]
package = "eqiora"
test = "fixed_reference_fsi_distributed_cuda_solve_mpi_2d"
features = ["mpi-cuda"]
environment = "physical-mpi-cuda"
```

The default is `host-cpu`. Environment values are a closed enum, not arbitrary
runner labels or host capabilities.

Repository-owned installed-wheel Python gate:

```toml
[evidence]
runner = "python-installed-wheel"
script = "tools/ci/python_torch_gate.py"
```

For Cargo, the runner first obtains the workspace package and integration-test
inventory from `cargo metadata --format-version=1 --no-deps`. It then invokes
the fixed argument vector
`cargo test --locked -p <package> --test <test>` directly. For Python it
invokes `PYTHON` (or `python3`) with the single declared script. The script must
be a normalized repository-relative regular `.py` file; no arguments are
accepted. Both runners use the repository root and never invoke a shell.

Cargo evidence for `physical-mpi-cuda` is ignored by generic Cargo suites.
Selecting that typed environment makes the runner append the fixed test-harness
arguments `-- --ignored`; the test still fails closed when its explicitly
selected physical topology is unavailable. This keeps ordinary compilation and
feature coverage separate from a hardware claim without adding free-form
manifest arguments.

Optional evidence artifacts must be normalized relative paths that resolve to
files inside the repository. Unknown runner identities or evidence keys,
duplicate capabilities, unknown statuses, missing artifacts/scripts,
duplicate case IDs, and paths whose ID does not match
`verify/<area>/<case>` are errors before any evidence starts.

## Determinism boundary

Cases are ordered by their declared ID, independent of directory enumeration.
The report does not contain timestamps or elapsed time. Child output is
evidence data and may vary with toolchains; consumers that compare report
bytes should normalize or omit those fields explicitly rather than treating
such variation as model meaning.
