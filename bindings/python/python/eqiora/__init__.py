"""Python ergonomics over Eqiora's canonical Rust implementation."""

from . import fluid, fsi, geometry, meshing, solid, trajectory

from ._eqiora import (
    __version__,
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
    FieldRef,
    InternalError,
    LinearSolveSummary,
    LinearizationState,
    Model,
    Parameter,
    ParameterRef,
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
    connect,
    derivative,
    div,
    grad,
    preview_realization,
    replay,
    submit as _submit,
    submit_realization as _submit_realization,
    submit_steady_stokes as _submit_steady_stokes,
    through,
    trace,
)

from . import diff

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
    "FieldRef",
    "InternalError",
    "LinearSolveSummary",
    "LinearizationState",
    "Model",
    "Parameter",
    "ParameterRef",
    "PhysicalDomain",
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
    "compile",
    "connect",
    "derivative",
    "div",
    "grad",
    "preview_realization",
    "replay",
    "run",
    "submit",
    "through",
    "trace",
    "diff",
    "fluid",
    "fsi",
    "geometry",
    "meshing",
    "solid",
    "trajectory",
]


_MISSING = object()


def run(
    model: Model,
    *,
    end_time=_MISSING,
    max_step=_MISSING,
    realization: Realization | None = None,
    plan=_MISSING,
):
    """Execute through the same native lifecycle returned by :func:`submit`."""

    return Run(
        _submit_native(
            "run()",
            model,
            end_time=end_time,
            max_step=max_step,
            realization=realization,
            plan=plan,
        )
    ).result()


def _submit_native(
    operation: str,
    model: Model,
    *,
    end_time,
    max_step,
    realization: Realization | None,
    plan: fluid.SteadyStokesPlan | None,
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
        return _submit_steady_stokes(model, plan)

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
    ) -> Result | ScalarEllipticResult | fluid.CircularHoleSteadyStokesResult:
        return self._native.result()

    async def _wait(
        self,
    ) -> Result | ScalarEllipticResult | fluid.CircularHoleSteadyStokesResult:
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
