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
cargo run -p eqiora-verify -- run --jobs 1
cargo run -p eqiora-verify -- run --case numerics.poisson-fem-fvm
cargo run -p eqiora-verify -- run --case numerics.cartesian-poisson-fem-fvm
cargo run -p eqiora-verify -- run --case numerics.cartesian-poisson-3d-fem-fvm
cargo run -p eqiora-verify -- run --case time.general-implicit-dae
cargo run -p eqiora-verify -- run --keep-going

# Validate the complete repository, then execute one environment/runner-kind
# intersection. Either filter may also be supplied alone.
cargo run -p eqiora-verify -- run --environment host-cpu --runner-kind cargo
cargo run -p eqiora-verify -- run --environment host-cpu --runner-kind python-installed-wheel
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
stdout contains exactly one `eqiora.verification-report/v5` object for
`list`, `check`, and `run`, or one
`eqiora.capability-evidence-index/v3` object for `index`. Version 5 adds the
optional `selected_runner_kind` report field and the closed `cargo` /
`python-installed-wheel` selector. It is orthogonal to
`selected_environment`; when both are present, a target must match both.
Version 4 added
`duration_ms` for each evidence target that started, measured with a monotonic
clock and reused by every case sharing that target. The field is always present
and carries `null` for a case whose target did not start or did not execute,
matching how every other optional field on the report encodes absence. Version 3 added the
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
case filter resolves, and every selected evidence target passes. Explicit
environment and runner-kind selections still validate every manifest, Cargo
target, and artifact before execution. A target excluded by either filter
remains visible as `not-selected`, with a message naming its required
environment or runner kind. An empty intersection succeeds after validation.
Without either filter, every executable target remains selected, so an absent
physical runtime still fails rather than silently skipping. In
fail-fast mode with `--jobs 1`, later selected executable cases remain in the
report with outcome `skipped`, matching serial execution before `--jobs` was
introduced. `--jobs` defaults to the host's available parallelism. With more
than one job, persistent workers pull targets from a shared queue so a free
slot starts the next target without waiting for the other running targets.
The runner stops launching new targets after the first test failure, lets the
at most `jobs - 1` already-launched targets finish, and reports their results;
it never kills a running child. Consequently, raising `--jobs` can turn a case
that serial fail-fast would report as `skipped` into a real `passed` or
`failed` result.
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
inventory from `cargo metadata --format-version=1 --no-deps`. After complete
validation, case selection, and both optional filters, selected Cargo targets
are partitioned by their exact package and sorted declared feature set. A
Python-only runner-kind selection therefore constructs and builds no Cargo
group. Each group is compiled once with the fixed form
`cargo test --locked -p <package> --no-run --message-format=json`, one `--test`
selector per target, and that group's exact `--features` value when non-empty.
Features from different manifests are never unioned. The runner reads Cargo's
compiler-artifact messages and executes each emitted test binary directly.
For Python it invokes `PYTHON` (or `python3`) with the single declared script.
The script must be a normalized repository-relative regular `.py` file; no
arguments are accepted. Both runners use the repository root and never invoke
a shell.

All Cargo groups are built before test execution. A group build failure is
reported against every case whose target belongs to that exact group and names
the group; it is not treated as a test failure for fail-fast scheduling, so
targets from successfully built groups still run. A test-binary failure is
attributed only to cases declaring that exact target. Cases sharing a target
reuse its one captured result as before.

Cargo evidence for `physical-mpi-cuda` is ignored by generic Cargo suites.
Selecting that typed environment makes the runner pass the fixed test-harness
argument `--ignored` directly to the produced binary; the test still fails
closed when its explicitly selected physical topology is unavailable. This
keeps ordinary compilation and feature coverage separate from a hardware claim
without adding free-form manifest arguments.

Optional evidence artifacts must be normalized relative paths that resolve to
files inside the repository. Unknown runner identities or evidence keys,
duplicate capabilities, unknown statuses, missing artifacts/scripts,
duplicate case IDs, and paths whose ID does not match
`verify/<area>/<case>` are errors before any evidence starts.

## Determinism boundary

Cases are ordered by their declared ID, independent of directory enumeration.
Target completion order never changes case order, result attribution, captured
streams, or the exit status. The report contains no timestamps; it does contain
each started target's monotonic elapsed duration. Child output and duration are
evidence data and may vary with toolchains and host load; consumers that
compare report bytes should normalize or omit those fields explicitly rather
than treating such variation as model meaning.
