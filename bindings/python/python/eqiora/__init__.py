"""Python ergonomics over Eqiora's canonical Rust implementation."""

import os
from typing import NamedTuple

from . import (
    fem,
    fluid,
    formulation,
    fsi,
    fvm,
    geometry,
    lang,
    meshing,
    solid,
    solve,
    time,
    trajectory,
)

from ._eqiora import (
    __version__,
    _check_package_conformance,
    Array,
    AuthoredFormulation,
    BoundarySide,
    CancellationError,
    CapabilityError,
    CompatibilityError,
    Connection,
    ConservingPort,
    ConvergenceReason,
    DerivativeImplementation,
    Diagnostic,
    DifferentiableEvaluation,
    DifferentiableJvp,
    DifferentiablePrimal,
    DifferentiableProgram,
    DifferentiableVjp,
    DifferentiationEvidence,
    DifferentiationMode,
    Dimension,
    DomainRef,
    Domain,
    EqioraError,
    ExecutionError,
    Expression,
    Field,
    FieldOutput,
    FieldRef,
    FormulationView,
    FormulationKind,
    FormulationSelectionMode,
    InitialField,
    InternalError,
    LinearSolveSummary,
    LinearizationState,
    Model,
    Parameter,
    ParameterRef,
    Plan,
    PhysicalDomain,
    PropertyBinding,
    Representation,
    Relation,
    Result,
    Revision,
    ResolvedExecution,
    ScalarPlanView,
    Run as _NativeRun,
    RunStatus,
    Series,
    State,
    TransientRunCancellation,
    TransientRunProgress,
    StructuralSemanticFingerprint,
    ValidationError,
    ValueEdit,
    across,
    compile as _compile,
    compile_package,
    connect,
    derivative,
    div,
    grad,
    submit_plan as _submit_plan,
    through,
    trace,
    _resolve_plan,
)

from . import diff
from .viewer import View


class PackageConformancePackage(NamedTuple):
    name: str
    version: str
    semantic_digest: str
    source_digest: str


class PackageConformanceReport(NamedTuple):
    profile: str
    eqiora_version: str
    compiler: str
    compiler_version: str
    semantic_canonicalization_version: int
    source_bundle_version: int
    resolution_version: int
    root_package: PackageConformancePackage
    packages: tuple[PackageConformancePackage, ...]
    entry_model: str
    resolution_digest: str
    package_compilation_digest: str
    model_id: str
    model_revision: int
    model_digest: str
    deterministic_replay_agreement: bool


__all__ = [
    "__version__",
    "Array",
    "AuthoredFormulation",
    "BoundarySide",
    "CancellationError",
    "CapabilityError",
    "CompatibilityError",
    "Connection",
    "ConservingPort",
    "ConvergenceReason",
    "DerivativeImplementation",
    "Diagnostic",
    "DifferentiableEvaluation",
    "DifferentiableJvp",
    "DifferentiablePrimal",
    "DifferentiableProgram",
    "DifferentiableVjp",
    "DifferentiationEvidence",
    "DifferentiationMode",
    "Dimension",
    "DomainRef",
    "Domain",
    "EqioraError",
    "ExecutionError",
    "Expression",
    "Field",
    "FieldOutput",
    "FieldRef",
    "FormulationView",
    "FormulationKind",
    "FormulationSelectionMode",
    "InitialField",
    "InternalError",
    "LinearSolveSummary",
    "LinearizationState",
    "Model",
    "PackageConformancePackage",
    "PackageConformanceReport",
    "Parameter",
    "ParameterRef",
    "PhysicalDomain",
    "PropertyBinding",
    "Plan",
    "Representation",
    "Relation",
    "Result",
    "Revision",
    "ResolvedExecution",
    "ScalarPlanView",
    "Run",
    "RunStatus",
    "Series",
    "State",
    "TransientRunCancellation",
    "TransientRunProgress",
    "StructuralSemanticFingerprint",
    "ValidationError",
    "ValueEdit",
    "View",
    "across",
    "check_package_conformance",
    "compile",
    "compile_package",
    "connect",
    "derivative",
    "div",
    "grad",
    "lang",
    "resolve",
    "run",
    "submit",
    "through",
    "trace",
    "diff",
    "fem",
    "fluid",
    "formulation",
    "fsi",
    "fvm",
    "geometry",
    "meshing",
    "solid",
    "solve",
    "time",
    "trajectory",
]


def compile(
    *,
    path=None,
    source=None,
    filename=None,
    geometry=None,
    parameters=None,
    component=None,
):
    """Compile text, a path, or one :class:`eqiora.lang.Source` canonically."""

    if isinstance(source, lang.Source):
        text = source.to_eqi()
        if source._requires_package_compilation():
            raise lang.SourceError(
                "a property-bearing Source requires an exact Model Package; "
                "emit it with to_eqi() or write_eqi() and compile the locked package"
            )
        source = text
        if filename is None:
            filename = "<python-source>"
    return _compile(
        path=path,
        source=source,
        filename=filename,
        geometry=geometry,
        parameters=parameters,
        component=component,
    )


def check_package_conformance(
    store_root: str | os.PathLike[str],
    resolution_bytes: bytes,
    *,
    entry_model: str,
    profile: str,
) -> PackageConformanceReport:
    """Check one exact locked package closure for structural conformance."""

    native = _check_package_conformance(
        store_root,
        resolution_bytes,
        entry_model=entry_model,
        profile=profile,
    )
    return PackageConformanceReport(
        *native[:7],
        PackageConformancePackage(*native[7]),
        tuple(PackageConformancePackage(*package) for package in native[8]),
        *native[9:],
    )


def resolve(
    model: Model,
    *,
    mesh: meshing.Mesh | None = None,
    spatial=None,
    formulation: FormulationKind | None = None,
    solve: solve.Linear | solve.Newton | None = None,
    scaling=None,
    temporal=None,
) -> Plan:
    """Resolve an exact Model and caller-owned Mesh into an immutable common Plan."""

    return _resolve_plan(
        model,
        mesh=mesh,
        spatial=spatial,
        formulation=formulation,
        solve=solve,
        scaling=scaling,
        temporal=temporal,
    )


def run(
    plan: Plan,
    *,
    state: State | None = None,
    until_s: float | None = None,
    output_times_s: tuple[float, ...] | None = None,
    steps: int | None = None,
    output_steps: tuple[int, ...] | None = None,
) -> Result:
    """Execute one accepted common request synchronously."""

    return submit(
        plan,
        state=state,
        until_s=until_s,
        output_times_s=output_times_s,
        steps=steps,
        output_steps=output_steps,
    ).result()


class Run:
    """Awaitable owner of one native execution occurrence."""

    __slots__ = ("_native",)

    def __init__(self, native: _NativeRun) -> None:
        self._native = native

    @property
    def status(self):
        return self._native.status

    @property
    def history(self):
        return tuple(self._native.history)

    @property
    def progress(self):
        return self._native.progress

    @property
    def cancellation(self):
        return self._native.cancellation

    @property
    def done(self) -> bool:
        return self._native.done

    @property
    def model_id(self) -> str:
        return self._native.model_id

    @property
    def model_digest(self) -> str:
        return self._native.model_digest

    @property
    def model_revision(self) -> int:
        return self._native.model_revision

    @property
    def package_compilation_digest(self) -> str | None:
        return self._native.package_compilation_digest

    @property
    def plan_key(self) -> str:
        return self._native.plan_key

    @property
    def adapter(self) -> str:
        return self._native.adapter

    @property
    def adapter_version(self) -> str:
        return self._native.adapter_version

    def cancel(self) -> bool:
        return self._native.cancel()

    def result(
        self,
    ) -> Result:
        return self._native.result()

    async def _wait(
        self,
    ) -> Result:
        import asyncio

        while not self.done:
            await asyncio.sleep(0.01)
        return self.result()

    def __await__(self):
        return self._wait().__await__()

    def __repr__(self) -> str:
        return repr(self._native)


def submit(
    plan: Plan,
    *,
    state: State | None = None,
    until_s: float | None = None,
    output_times_s: tuple[float, ...] | None = None,
    steps: int | None = None,
    output_steps: tuple[int, ...] | None = None,
) -> Run:
    """Submit exactly one steady or transient common request shape."""

    return Run(
        _submit_plan(
            plan,
            state=state,
            until_s=until_s,
            output_times_s=output_times_s,
            steps=steps,
            output_steps=output_steps,
        )
    )
