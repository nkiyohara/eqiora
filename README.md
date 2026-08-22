# Eqiora

Eqiora is an open-source computational engineering platform that represents
models as one typed network of mathematical relations, then carries that
meaning through numerical realization to auditable evidence.

> **Alpha — `0.1.0a2`.** Eqiora currently provides carefully bounded,
> executable slices of its intended system. It is research software, not a
> safety control or a complete multiphysics product. Every supported claim and
> explicit nonclaim is indexed in the
> [capability matrix](docs/capability-matrix.md) and the reproducible
> [`verify/`](verify/) catalogue.

## Start with Python

The alpha distribution supports ordinary-GIL CPython 3.11–3.14 on
manylinux x86-64:

```console
python -m pip install eqiora==0.1.0a2
```

Build and run an immutable decay model:

```python
import eqiora

state = eqiora.Field("state", initial=1.0)
rate = eqiora.Parameter(
    "rate",
    value=1.0,
    dimension=eqiora.Dimension(time=-1),
)
decay = eqiora.Relation(
    "decay",
    residual=eqiora.derivative(state) + rate * state,
)
model = eqiora.Model.define("decay", state, rate, decay)

result = eqiora.run(model, end_time=1.0, max_step=0.01)
print(result["state"].values.numpy(copy=False)[-1])
```

The [five-minute guide](https://eqiora.org/get-started/) continues through
structured diagnostics, array ownership, asynchronous runs, bounded PyTorch
and JAX integrations, and optional Matplotlib presentation of accepted
results.

For a bounded local file check, the installed `eqiora` binary accepts
`eqiora check <MODEL_PATH>`. It reads one UTF-8 regular file, prints only a
structural comparison fingerprint when the current Model is accepted, and
prints bounded normalized diagnostics when compilation rejects it. The
command does not execute the Model, write an artifact, accept stdin or
multiple files, expose JSON, or make Python or Studio a CLI subprocess client.

Local agents can separately compile/check one in-memory Eqiora source through
the `eqiora-mcp` subprocess. It exposes exactly one bounded MCP `2026-07-28`
tool over newline-delimited stdio and returns either structured compiler
diagnostics or the current Model descriptor and comparison fingerprint. It
does not execute a model, transport scientific results, persist an artifact,
or provide remote, Python, or Studio integration. Python remains the first
execution API and can serve the initial gallery directly; a future Studio
client can consume the same Rust-owned model semantics through its own
independently verified projection.

## One model, two layers

Eqiora treats block diagrams, state charts, PDEs, and acausal physical
networks as views of the same small semantic kernel. A canonical model is a
network of typed relations, activations, and signal or conserving
connections. Numerical choices—mesh, discretization, solver, schedule, CPU,
GPU, or distributed execution—belong to a separate **Realization**.

That separation is enforced by one traceable path:

```text
meaning → lowered contract → realization → adapter → evidence
```

Source, Python, Studio, and future visual editors therefore create
transactions against one Rust-owned model semantics; none is a second
authority. Optimized adapters may widen execution, but only registered
falsifiers and evidence widen a public capability claim.

## What this alpha proves

The release includes bounded, reproducible vertical slices for the semantic
kernel and language, reference hybrid execution, scalar Operator IR,
one-to-three-dimensional scalar elliptic FEM/FVM paths, selected host/CUDA/MPI
adapters, implicit differentiation, versioned artifacts, Python model
construction and execution, and a thin Studio projection. The exact domain,
platform, method, and maturity of each slice are recorded in the
[capability matrix](docs/capability-matrix.md); the
[architecture guide](docs/architecture.md) explains their boundaries.

Important nonclaims include:

- no stable-1.0 compatibility promise;
- no macOS, Windows, free-threaded Python, GPU wheel, or bundled MPI package;
- no complete CFD, FSI, CAD, controls, or physical-component catalogue;
- no general high-order, adaptive, mixed/tensor-field, or arbitrary-DAE path;
- no claim of being a complete Simulink, Simscape, or commercial CAE
  replacement;
- no certification for safety-critical or production engineering decisions.

## Project

- [Website and documentation](https://eqiora.org)
- [Python package](https://pypi.org/project/eqiora/)
- [Capabilities](docs/capability-matrix.md)
- [Published benchmarks](docs/benchmarks.md)
- [Architecture](docs/architecture.md)
- [Roadmap](docs/roadmap.md)
- [Contributing](CONTRIBUTING.md)
- [Security](SECURITY.md)
- [Governance](GOVERNANCE.md)
- [Release policy](docs/development/python-release-policy.md)

Eqiora is developed in public under the
[Apache License 2.0](LICENSE). Contributions require a
[Developer Certificate of Origin](CONTRIBUTING.md#developer-certificate-of-origin)
sign-off.
