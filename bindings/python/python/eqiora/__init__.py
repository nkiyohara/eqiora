"""Python ergonomics over Eqiora's canonical Rust implementation."""

import os
from typing import NamedTuple

from . import fem, fluid, fsi, fvm, geometry, meshing, solid, solve, time, trajectory

from ._eqiora import (
    __version__,
    _check_package_conformance,
    Array,
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
    Domain,
    EqioraError,
    ExecutionError,
    Expression,
    Field,
    FieldOutput,
    FieldRef,
    InternalError,
    LinearSolveSummary,
    LinearizationState,
    Model,
    Parameter,
    ParameterRef,
    Plan,
    PhysicalDomain,
    Realization,
    Representation,
    Relation,
    Result,
    Revision,
    RunManifest,
    Run as _NativeRun,
    RunCancellation,
    RunProgress,
    RunStatus,
    ScalarElliptic,
    ScalarEllipticBalance,
    ScalarEllipticMethod,
    ScalarEllipticResult,
    ScalarEllipticRunCancellation,
    ScalarEllipticRunProgress,
    ScalarFieldLocation,
    ScalarFieldSummary,
    Series,
    StructuralSemanticFingerprint,
    ValidationError,
    ValueEdit,
    across,
    compile,
    compile_package,
    connect,
    derivative,
    div,
    grad,
    preview_realization,
    replay,
    submit as _submit,
    submit_realization as _submit_realization,
    submit_fixed_mesh_monolithic as _submit_fixed_mesh_monolithic,
    submit_linear_elasticity as _submit_linear_elasticity,
    submit_steady_stokes as _submit_steady_stokes,
    through,
    trace,
    _resolve_plan,
    _run_plan,
)

from . import diff


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
    "Domain",
    "EqioraError",
    "ExecutionError",
    "Expression",
    "Field",
    "FieldOutput",
    "FieldRef",
    "InternalError",
    "LinearSolveSummary",
    "LinearizationState",
    "Model",
    "PackageConformancePackage",
    "PackageConformanceReport",
    "Parameter",
    "ParameterRef",
    "PhysicalDomain",
    "Plan",
    "Realization",
    "Representation",
    "Relation",
    "Result",
    "Revision",
    "Run",
    "RunManifest",
    "RunCancellation",
    "RunProgress",
    "RunStatus",
    "ScalarElliptic",
    "ScalarEllipticBalance",
    "ScalarEllipticMethod",
    "ScalarEllipticResult",
    "ScalarEllipticRunCancellation",
    "ScalarEllipticRunProgress",
    "ScalarFieldLocation",
    "ScalarFieldSummary",
    "Series",
    "StructuralSemanticFingerprint",
    "ValidationError",
    "ValueEdit",
    "across",
    "check_package_conformance",
    "compile",
    "compile_package",
    "connect",
    "derivative",
    "div",
    "grad",
    "preview_realization",
    "replay",
    "resolve",
    "run",
    "submit",
    "through",
    "trace",
    "diff",
    "fem",
    "fluid",
    "fsi",
    "fvm",
    "geometry",
    "meshing",
    "solid",
    "solve",
    "time",
    "trajectory",
]


_MISSING = object()


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
    mesh: meshing.Mesh,
    spatial,
    solve: solve.Linear | solve.Newton,
    scaling=None,
    temporal=None,
) -> Plan:
    """Resolve an exact Model and caller-owned Mesh into an immutable common Plan."""

    return _resolve_plan(
        model,
        mesh=mesh,
        spatial=spatial,
        solve=solve,
        scaling=scaling,
        temporal=temporal,
    )


def run(plan: Plan) -> Result:
    """Execute solely from one immutable common Plan."""

    return _run_plan(plan)


def _submit_native(
    operation: str,
    model: Model,
    *,
    end_time,
    max_step,
    realization: Realization | None,
    plan,
) -> _NativeRun:
    """Validate one public request shape before crossing the native boundary."""

    has_end_time = end_time is not _MISSING
    has_max_step = max_step is not _MISSING

    if plan is not _MISSING:
        if realization is not None or has_end_time or has_max_step:
            raise TypeError(
                f"{operation} accepts plan alone; realization, end_time, and "
                "max_step belong to other execution forms"
            )
        if isinstance(plan, fluid.SteadyStokesPlan):
            return _submit_steady_stokes(model, plan)
        if isinstance(plan, fsi.FixedMeshMonolithicPlan):
            return _submit_fixed_mesh_monolithic(model, plan)
        if isinstance(plan, solid.LinearElasticityPlan):
            return _submit_linear_elasticity(model, plan)
        raise TypeError(f"{operation} received an unsupported Plan type")

    if realization is not None:
        if has_end_time or has_max_step:
            raise TypeError(
                f"{operation} accepts realization alone; end_time and max_step "
                "belong to the reference time-integration form"
            )
        return _submit_realization(model, realization)

    if not has_end_time or not has_max_step:
        raise TypeError(
            f"{operation} requires either realization=..., plan=..., or both "
            "end_time=... and max_step=..."
        )

    return _submit(model, end_time=end_time, max_step=max_step)


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
    def plan_key(self) -> str:
        return self._native.plan_key

    @property
    def adapter(self) -> str:
        return self._native.adapter

    def cancel(self) -> bool:
        return self._native.cancel()

    def result(
        self,
    ) -> Result | ScalarEllipticResult:
        return self._native.result()

    async def _wait(
        self,
    ) -> Result | ScalarEllipticResult:
        import asyncio

        while not self.done:
            await asyncio.sleep(0.01)
        return self.result()

    def __await__(self):
        return self._wait().__await__()

    def __repr__(self) -> str:
        return repr(self._native)


def submit(
    model: Model,
    *,
    end_time=_MISSING,
    max_step=_MISSING,
    realization: Realization | None = None,
    plan=_MISSING,
) -> Run:
    """Submit exactly one accepted temporal or spatial request shape."""

    return Run(
        _submit_native(
            "submit()",
            model,
            end_time=end_time,
            max_step=max_step,
            realization=realization,
            plan=plan,
        )
    )
