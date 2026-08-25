"""Python ergonomics over Eqiora's canonical Rust implementation.

Authority: ``bindings/python/python/eqiora/__init__.py``.
"""

from collections.abc import Generator, Iterator, Sequence
import os
from os import PathLike
from typing import (
    Any,
    ClassVar,
    Generic,
    Literal,
    NamedTuple,
    Never,
    Protocol,
    Self,
    TypeVar,
    final,
    overload,
)

import numpy as np
import numpy.typing as npt

from . import diff as diff
from . import fluid as fluid
from . import fsi as fsi
from . import geometry as geometry
from . import geometry as _geometry
from . import meshing as meshing
from . import solid as solid
from . import trajectory as trajectory

_Float64Array = npt.NDArray[np.float64]

__version__: str

class _DLPackProducer(Protocol):
    def __dlpack_device__(self) -> tuple[int, int]: ...
    def __dlpack__(
        self,
        *,
        stream: object | None = ...,
        max_version: tuple[int, int] | None = ...,
        dl_device: tuple[int, int] | None = ...,
        copy: bool | None = ...,
    ) -> object: ...

@final
class Diagnostic:
    """Immutable lossless projection of a current Rust diagnostic.

    Authority: ``crates/eqiora-python/src/error.rs::PyDiagnostic``.
    """

    @property
    def source(self) -> str: ...
    @property
    def code(self) -> str: ...
    @property
    def severity(self) -> str: ...
    @property
    def message(self) -> str: ...
    @property
    def graph_path(self) -> list[str] | None: ...
    @property
    def source_span(self) -> tuple[str, int, int] | None: ...
    @property
    def suggestion(self) -> str | None: ...

class EqioraError(Exception):
    """Base failure for an Eqiora operation rejected with diagnostics.

    Authority: ``crates/eqiora-python/src/error.rs::EqioraError``.
    """

    category: str
    diagnostics: tuple[Diagnostic, ...]

class ValidationError(EqioraError):
    """Failure caused by a model or request violating a typed contract.

    Authority: ``crates/eqiora-python/src/error.rs::ValidationError``.
    """

    ...

class CompatibilityError(EqioraError):
    """Failure caused by an incompatible versioned or persisted value.

    Authority: ``crates/eqiora-python/src/error.rs::CompatibilityError``.
    """

    ...

class CapabilityError(EqioraError):
    """Failure caused by an adapter lacking a required capability.

    Authority: ``crates/eqiora-python/src/error.rs::CapabilityError``.
    """

    ...

class ExecutionError(EqioraError):
    """Failure of an admitted execution.

    Authority: ``crates/eqiora-python/src/error.rs::ExecutionError``.
    """

    ...

class CancellationError(EqioraError):
    """Failure reporting cancellation of an Eqiora operation.

    Authority: ``crates/eqiora-python/src/error.rs::CancellationError``.
    """

    ...

class InternalError(EqioraError):
    """Internal failure that does not expose implementation details.

    Authority: ``crates/eqiora-python/src/error.rs::InternalError``.
    """

    ...

class PackageConformancePackage(NamedTuple):
    """Identity fields for one package in a conformance report.

    Authority: ``bindings/python/python/eqiora/__init__.py::check_package_conformance``.
    """

    name: str
    version: str
    semantic_digest: str
    source_digest: str

class PackageConformanceReport(NamedTuple):
    """Structural-conformance report for one locked package closure.

    Authority: ``bindings/python/python/eqiora/__init__.py::check_package_conformance``.
    """

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

@final
class Dimension:
    """SI base-dimension exponents in M, L, T, I, Θ, N, J order.

    Authority: ``crates/eqiora-python/src/modeling.rs::PyDimension``.
    """

    def __new__(
        cls,
        *,
        mass: int = 0,
        length: int = 0,
        time: int = 0,
        current: int = 0,
        temperature: int = 0,
        amount: int = 0,
        luminous_intensity: int = 0,
    ) -> Self: ...
    @property
    def exponents(self) -> tuple[int, int, int, int, int, int, int]: ...
    def __eq__(self, other: object, /) -> bool: ...
    def __ne__(self, other: object, /) -> bool: ...

@final
class BoundarySide:
    """Closed orientation of one Cartesian boundary domain.

    Authority: ``crates/eqiora-python/src/modeling.rs::PyBoundarySide``.
    """

    Lower: ClassVar[BoundarySide]
    Upper: ClassVar[BoundarySide]
    def __eq__(self, other: object, /) -> bool: ...
    def __hash__(self) -> int: ...

@final
class Domain:
    """Immutable draft-local Cartesian volume or oriented boundary.

    Authority: ``crates/eqiora-python/src/modeling.rs::PyDomain``.
    """

    @staticmethod
    def box(name: str, *bounds: tuple[float, float]) -> Domain: ...
    def boundary(
        self,
        name: str,
        *,
        axis: int,
        side: BoundarySide,
    ) -> Domain: ...
    @property
    def name(self) -> str: ...
    @property
    def bounds(self) -> list[tuple[float, float]] | None: ...
    @property
    def parent(self) -> Domain | None: ...
    @property
    def axis(self) -> int | None: ...
    @property
    def side(self) -> BoundarySide | None: ...
    def __eq__(self, other: object, /) -> bool: ...
    def __hash__(self) -> int: ...

@final
class Representation:
    """Immutable continuum representation declaration.

    Authority: ``crates/eqiora-python/src/modeling.rs::PyRepresentation``.
    """

    @staticmethod
    def continuum(name: str) -> Representation: ...
    @property
    def name(self) -> str: ...
    def __eq__(self, other: object, /) -> bool: ...
    def __hash__(self) -> int: ...

@final
class Expression:
    """Immutable symbolic expression whose shape and support Rust infers.

    Authority: ``crates/eqiora-python/src/modeling.rs::PyExpression``.
    """

    def __neg__(self) -> Expression: ...
    def __add__(self, right: _ExpressionLike, /) -> Expression: ...
    def __radd__(self, left: _ExpressionLike, /) -> Expression: ...
    def __sub__(self, right: _ExpressionLike, /) -> Expression: ...
    def __rsub__(self, left: _ExpressionLike, /) -> Expression: ...
    def __mul__(self, right: _ExpressionLike, /) -> Expression: ...
    def __rmul__(self, left: _ExpressionLike, /) -> Expression: ...
    def __truediv__(self, right: _ExpressionLike, /) -> Expression: ...
    def __rtruediv__(self, left: _ExpressionLike, /) -> Expression: ...
    def __bool__(self) -> bool: ...

@final
class Field:
    """Immutable scalar field declaration.

    Authority: ``crates/eqiora-python/src/modeling.rs::PyField``.
    """

    def __new__(
        cls,
        name: str,
        *,
        domain: Domain | None = None,
        representation: Representation | None = None,
        dimension: Dimension | None = None,
        initial: float = 0.0,
    ) -> Self: ...
    @property
    def name(self) -> str: ...
    @property
    def dimension(self) -> Dimension: ...
    @property
    def initial(self) -> float: ...
    @property
    def domain(self) -> Domain | None: ...
    @property
    def representation(self) -> Representation | None: ...
    def __neg__(self) -> Expression: ...
    def __add__(self, right: _ExpressionLike, /) -> Expression: ...
    def __radd__(self, left: _ExpressionLike, /) -> Expression: ...
    def __sub__(self, right: _ExpressionLike, /) -> Expression: ...
    def __rsub__(self, left: _ExpressionLike, /) -> Expression: ...
    def __mul__(self, right: _ExpressionLike, /) -> Expression: ...
    def __rmul__(self, left: _ExpressionLike, /) -> Expression: ...
    def __truediv__(self, right: _ExpressionLike, /) -> Expression: ...
    def __rtruediv__(self, left: _ExpressionLike, /) -> Expression: ...
    def __bool__(self) -> bool: ...

@final
class Parameter:
    """Immutable scalar parameter declaration.

    Authority: ``crates/eqiora-python/src/modeling.rs::PyParameter``.
    """

    def __new__(
        cls,
        name: str,
        *,
        value: float,
        dimension: Dimension | None = None,
    ) -> Self: ...
    @property
    def name(self) -> str: ...
    @property
    def dimension(self) -> Dimension: ...
    @property
    def value(self) -> float: ...
    def __neg__(self) -> Expression: ...
    def __add__(self, right: _ExpressionLike, /) -> Expression: ...
    def __radd__(self, left: _ExpressionLike, /) -> Expression: ...
    def __sub__(self, right: _ExpressionLike, /) -> Expression: ...
    def __rsub__(self, left: _ExpressionLike, /) -> Expression: ...
    def __mul__(self, right: _ExpressionLike, /) -> Expression: ...
    def __rmul__(self, left: _ExpressionLike, /) -> Expression: ...
    def __truediv__(self, right: _ExpressionLike, /) -> Expression: ...
    def __rtruediv__(self, left: _ExpressionLike, /) -> Expression: ...
    def __bool__(self) -> bool: ...

@final
class PhysicalDomain:
    """Immutable nominal scalar physical-domain declaration.

    Authority: ``crates/eqiora-python/src/modeling.rs::PyPhysicalDomain``.
    """

    def __new__(
        cls,
        name: str,
        *,
        across_dimension: Dimension,
        through_dimension: Dimension,
    ) -> Self: ...
    @property
    def name(self) -> str: ...
    @property
    def across_dimension(self) -> Dimension: ...
    @property
    def through_dimension(self) -> Dimension: ...

@final
class ConservingPort:
    """Immutable scalar conserving-port declaration.

    Authority: ``crates/eqiora-python/src/modeling.rs::PyConservingPort``.
    """

    def __new__(cls, name: str, *, domain: PhysicalDomain) -> Self: ...
    @property
    def name(self) -> str: ...
    @property
    def domain(self) -> PhysicalDomain: ...

@final
class Connection:
    """Immutable anonymous conserving connection declaration.

    Authority: ``crates/eqiora-python/src/modeling.rs::PyConnection``.
    """

    ...

_ExpressionLike = Expression | Field | Parameter | float

@final
class Relation:
    """Immutable continuous implicit relation declaration.

    Authority: ``crates/eqiora-python/src/modeling.rs::PyRelation``.
    """

    @overload
    def __new__(
        cls,
        name: str,
        *,
        domain: Domain | None = None,
        residual: _ExpressionLike,
        residuals: None = None,
    ) -> Self: ...
    @overload
    def __new__(
        cls,
        name: str,
        *,
        domain: Domain | None = None,
        residual: None = None,
        residuals: Sequence[_ExpressionLike],
    ) -> Self: ...
    @property
    def name(self) -> str: ...
    @property
    def residual(self) -> Expression: ...
    @property
    def residuals(self) -> list[Expression]: ...
    @property
    def domain(self) -> Domain | None: ...

@final
class Array:
    """Immutable dense one-dimensional CPU ``float64`` result buffer.

    Authority: ``crates/eqiora-python/src/array.rs::PyArrayBuffer``.
    """

    def __getitem__(self, index: int, /) -> float: ...
    def numpy(self, *, copy: bool | None = None) -> _Float64Array: ...
    def __array__(
        self, dtype: object | None = None, copy: bool | None = None
    ) -> _Float64Array: ...
    def __dlpack_device__(self) -> tuple[int, int]: ...
    def __dlpack__(
        self,
        *,
        stream: int | None = None,
        max_version: tuple[int, int] | None = None,
        dl_device: tuple[int, int] | None = None,
        copy: bool | None = None,
    ) -> object: ...
    @property
    def device(self) -> str: ...
    @property
    def device_id(self) -> int: ...
    @property
    def dtype(self) -> str: ...
    @property
    def byte_order(self) -> str: ...
    @property
    def shape(self) -> tuple[int]: ...
    @property
    def strides(self) -> tuple[int]: ...
    @property
    def c_contiguous(self) -> bool: ...
    @property
    def aligned(self) -> bool: ...
    @property
    def readonly(self) -> bool: ...
    @property
    def ownership(self) -> str: ...
    @property
    def origin_copy_occurred(self) -> bool: ...
    def __len__(self) -> int: ...

@final
class Revision:
    """Exact identity of one immutable canonical model artifact.

    Authority: ``crates/eqiora-python/src/model.rs::PyRevision``.
    """

    @property
    def model_id(self) -> str: ...
    @property
    def digest(self) -> str: ...
    @property
    def number(self) -> int: ...
    def __eq__(self, other: object, /) -> bool: ...
    def __hash__(self) -> int: ...

@final
class StructuralSemanticFingerprint:
    """Alpha-normalized comparison evidence, not exact model identity.

    Authority: ``crates/eqiora-python/src/model.rs::PyStructuralSemanticFingerprint``.
    """

    @property
    def generation(self) -> str: ...
    @property
    def digest(self) -> str: ...
    def __eq__(self, other: object, /) -> bool: ...
    def __hash__(self) -> int: ...

@final
class ValueEdit:
    """Immutable exact-base value edit prepared by the Rust facade.

    Authority: ``crates/eqiora-python/src/model.rs::PyValueEdit``.
    """

    @property
    def key(self) -> str: ...
    @property
    def base_digest(self) -> str: ...
    @property
    def base_revision(self) -> int: ...
    @property
    def target_id(self) -> str: ...
    def __eq__(self, other: object, /) -> bool: ...
    def __hash__(self) -> int: ...

@final
class ParameterRef:
    """Exact canonical parameter selected from one immutable model.

    Authority: ``crates/eqiora-python/src/model.rs::PyModelParameterRef``.
    """

    @property
    def model_digest(self) -> str: ...
    @property
    def id(self) -> str: ...
    def __eq__(self, other: object, /) -> bool: ...
    def __hash__(self) -> int: ...

@final
class FieldRef:
    """Exact canonical field selected from one immutable model.

    Authority: ``crates/eqiora-python/src/model.rs::PyModelFieldRef``.
    """

    @property
    def model_digest(self) -> str: ...
    @property
    def id(self) -> str: ...
    def __eq__(self, other: object, /) -> bool: ...
    def __hash__(self) -> int: ...

@final
class Model:
    """Immutable canonical model artifact, admitted when semantically closed.

    Authority: ``crates/eqiora-python/src/model.rs::PyModel``.
    """

    @staticmethod
    def define(
        name: str,
        *declarations: _ModelDeclaration,
    ) -> Model: ...
    def to_json(self) -> bytes: ...
    def preview_value_edit(self, target: str, value: float) -> ValueEdit: ...
    def commit(self, edit: ValueEdit) -> Model: ...
    def parameter(self, selection: str) -> ParameterRef: ...
    def field(self, selection: str) -> FieldRef: ...
    def structurally_equivalent(self, other: Model) -> bool: ...
    @property
    def digest(self) -> str: ...
    @property
    def package_compilation_digest(self) -> str | None: ...
    @property
    def structural_fingerprint(self) -> StructuralSemanticFingerprint: ...
    @property
    def revision(self) -> Revision: ...
    @property
    def model_id(self) -> str: ...
    @property
    def field_ids(self) -> list[str]: ...
    @property
    def parameter_ids(self) -> list[str]: ...
    def __eq__(self, other: object, /) -> bool: ...
    def __hash__(self) -> int: ...

@final
class ScalarEllipticMethod:
    """Numerical family for one bounded scalar-elliptic request.

    Authority: ``crates/eqiora-python/src/realization.rs::PyScalarEllipticMethod``.
    """

    FiniteElement: ClassVar[ScalarEllipticMethod]
    FiniteVolume: ClassVar[ScalarEllipticMethod]
    def __eq__(self, other: object, /) -> bool: ...
    def __hash__(self) -> int: ...

@final
class ScalarElliptic:
    """Unbound typed scalar-elliptic request, not realization identity.

    Authority: ``crates/eqiora-python/src/realization.rs::PyScalarElliptic``.
    """

    def __new__(
        cls,
        *,
        method: ScalarEllipticMethod,
        cells_per_axis: int,
        realization_revision: int = 1,
    ) -> Self: ...
    @property
    def method(self) -> ScalarEllipticMethod: ...
    @property
    def cells_per_axis(self) -> int: ...
    @property
    def realization_revision(self) -> int: ...
    def __eq__(self, other: object, /) -> bool: ...
    def __hash__(self) -> int: ...

@final
class Realization:
    """Exact model-bound capability-admitted portable realization.

    Authority: ``crates/eqiora-python/src/realization.rs::PyRealization``.
    """

    @property
    def digest(self) -> str: ...
    @property
    def model_digest(self) -> str: ...
    @property
    def realization_revision(self) -> int: ...
    @property
    def method(self) -> ScalarEllipticMethod: ...
    @property
    def cells_per_axis(self) -> int: ...
    @property
    def workers(self) -> int: ...
    @property
    def cell_count(self) -> int: ...
    @property
    def field_value_count(self) -> int: ...
    @property
    def spatial_dimension(self) -> int: ...
    @property
    def field_logical_shape(self) -> tuple[int, ...]: ...
    @property
    def field_bounds(self) -> tuple[tuple[float, float], ...]: ...
    def to_json(self) -> bytes: ...
    def __eq__(self, other: object, /) -> bool: ...
    def __hash__(self) -> int: ...

@final
class ScalarFieldLocation:
    """Vertex or cell-centre meaning of a scalar field summary.

    Authority: ``crates/eqiora-python/src/realization.rs::PyScalarFieldLocation``.
    """

    Vertex: ClassVar[ScalarFieldLocation]
    CellCenter: ClassVar[ScalarFieldLocation]
    def __eq__(self, other: object, /) -> bool: ...
    def __hash__(self) -> int: ...

@final
class ScalarFieldSummary:
    """Bounded accepted field summary; arrays remain on the data plane.

    Authority: ``crates/eqiora-python/src/realization.rs::PyScalarFieldSummary``.
    """

    @property
    def location(self) -> ScalarFieldLocation: ...
    @property
    def spatial_dimension(self) -> int: ...
    @property
    def logical_shape(self) -> tuple[int, ...]: ...
    @property
    def value_count(self) -> int: ...
    @property
    def minimum(self) -> float: ...
    @property
    def maximum(self) -> float: ...

@final
class ScalarEllipticBalance:
    """Accepted continuous scalar-elliptic balance evidence.

    Authority: ``crates/eqiora-python/src/realization.rs::PyScalarEllipticBalance``.
    """

    @property
    def boundary_total(self) -> float: ...
    @property
    def integrated_source(self) -> float: ...
    @property
    def relative_imbalance(self) -> float: ...

@final
class ConvergenceReason:
    """Accepted reason for linear-solve convergence.

    Authority: ``crates/eqiora-python/src/realization.rs::PyConvergenceReason``.
    """

    InitialResidualSatisfied: ClassVar[ConvergenceReason]
    ResidualToleranceSatisfied: ClassVar[ConvergenceReason]
    def __eq__(self, other: object, /) -> bool: ...
    def __hash__(self) -> int: ...

@final
class IncompressibleFlowScales:
    """Characteristic coherent-SI scales for incompressible-flow realization.

    Authority: ``crates/eqiora-python/src/execution_policy.rs::PyIncompressibleFlowScales``.
    """

    def __new__(
        cls,
        *,
        length_m: float,
        velocity_m_per_s: float,
        pressure_pa: float,
    ) -> IncompressibleFlowScales: ...
    @property
    def length_m(self) -> float: ...
    @property
    def velocity_m_per_s(self) -> float: ...
    @property
    def pressure_pa(self) -> float: ...
    def __eq__(self, other: object, /) -> bool: ...
    def __hash__(self) -> int: ...
    def __repr__(self) -> str: ...

@final
class LinearSolve:
    """Complete backend-neutral policy for one linear solve.

    Authority: ``crates/eqiora-python/src/execution_policy.rs::PyLinearSolve``.
    """

    def __new__(
        cls,
        *,
        algorithm: Literal[
            "conjugate-gradient", "minimum-residual", "bicgstab", "sparse-lu"
        ],
        preconditioner: Literal["identity", "jacobi"],
        reduction: Literal["reproducible", "fast"],
        relative_tolerance: float,
        absolute_tolerance: float,
        maximum_iterations: int,
    ) -> LinearSolve: ...
    @property
    def algorithm(self) -> str: ...
    @property
    def preconditioner(self) -> str: ...
    @property
    def reduction(self) -> str: ...
    @property
    def relative_tolerance(self) -> float: ...
    @property
    def absolute_tolerance(self) -> float: ...
    @property
    def maximum_iterations(self) -> int: ...
    def __eq__(self, other: object, /) -> bool: ...
    def __hash__(self) -> int: ...
    def __repr__(self) -> str: ...

@final
class LinearSolveSummary:
    """Bounded projection of an independently accepted linear-solve report.

    Authority: ``crates/eqiora-python/src/realization.rs::PyLinearSolveSummary``.
    """

    @property
    def backend(self) -> str: ...
    @property
    def adapter(self) -> str: ...
    @property
    def verification_adapter(self) -> str: ...
    @property
    def orientation(self) -> str: ...
    @property
    def algorithm(self) -> str: ...
    @property
    def preconditioner(self) -> str: ...
    @property
    def reduction(self) -> str: ...
    @property
    def relative_tolerance(self) -> float: ...
    @property
    def absolute_tolerance(self) -> float: ...
    @property
    def maximum_iterations(self) -> int: ...
    @property
    def reason(self) -> ConvergenceReason: ...
    @property
    def completed_iterations(self) -> int: ...
    @property
    def initial_residual_norm(self) -> float: ...
    @property
    def reported_residual_norm(self) -> float: ...
    @property
    def true_residual_norm(self) -> float: ...
    @property
    def residual_target(self) -> float: ...

@final
class DifferentiationMode:
    """Primal, JVP, or VJP occurrence kind.

    Authority: ``crates/eqiora-python/src/differentiation.rs::PyDifferentiationMode``.
    """

    Primal: ClassVar[DifferentiationMode]
    Jvp: ClassVar[DifferentiationMode]
    Vjp: ClassVar[DifferentiationMode]
    def __eq__(self, other: object, /) -> bool: ...
    def __hash__(self) -> int: ...

@final
class DerivativeImplementation:
    """Source of the derivative action used by an occurrence.

    Authority: ``crates/eqiora-python/src/differentiation.rs::PyDerivativeImplementation``.
    """

    AnalyticAssembled: ClassVar[DerivativeImplementation]
    def __eq__(self, other: object, /) -> bool: ...
    def __hash__(self) -> int: ...

@final
class LinearizationState:
    """Whether an accepted linearization was established or reused.

    Authority: ``crates/eqiora-python/src/differentiation.rs::PyLinearizationState``.
    """

    Established: ClassVar[LinearizationState]
    Reused: ClassVar[LinearizationState]
    def __eq__(self, other: object, /) -> bool: ...
    def __hash__(self) -> int: ...

@final
class DifferentiationEvidence:
    """Typed in-memory provenance for one differentiation occurrence.

    Authority: ``crates/eqiora-python/src/differentiation.rs::PyDifferentiationEvidence``.
    """

    @property
    def model_digest(self) -> str: ...
    @property
    def realization_digest(self) -> str: ...
    @property
    def input_ids(self) -> list[str]: ...
    @property
    def output_id(self) -> str: ...
    @property
    def mode(self) -> DifferentiationMode: ...
    @property
    def implementation(self) -> DerivativeImplementation: ...
    @property
    def linearization_state(self) -> LinearizationState: ...
    @property
    def state_system_fingerprint(self) -> str: ...
    @property
    def primal_residual_norm(self) -> float: ...
    @property
    def residual_tolerance(self) -> float: ...
    @property
    def primal_solve(self) -> LinearSolveSummary: ...
    @property
    def derivative_solve(self) -> LinearSolveSummary | None: ...

@final
class DifferentiablePrimal:
    """Accepted complete primary field from a primal evaluation.

    Authority: ``crates/eqiora-python/src/differentiation.rs::PyDifferentiablePrimal``.
    """

    @property
    def output(self) -> Array: ...
    @property
    def evidence(self) -> DifferentiationEvidence: ...

@final
class DifferentiableJvp:
    """Accepted primary field and its forward tangent.

    Authority: ``crates/eqiora-python/src/differentiation.rs::PyDifferentiableJvp``.
    """

    @property
    def output(self) -> Array: ...
    @property
    def tangent(self) -> Array: ...
    @property
    def evidence(self) -> DifferentiationEvidence: ...

@final
class DifferentiableVjp:
    """Accepted primary field and its reverse input cotangent.

    Authority: ``crates/eqiora-python/src/differentiation.rs::PyDifferentiableVjp``.
    """

    @property
    def output(self) -> Array: ...
    @property
    def input_cotangent(self) -> Array: ...
    @property
    def evidence(self) -> DifferentiationEvidence: ...

@final
class DifferentiableEvaluation:
    """Immutable accepted evaluation at one numerical parameter point.

    Authority: ``crates/eqiora-python/src/differentiation.rs::PyDifferentiableEvaluation``.
    """

    @property
    def point(self) -> Array: ...
    def primal(self) -> DifferentiablePrimal: ...
    def jvp(
        self, tangent: Array | _Float64Array | _DLPackProducer
    ) -> DifferentiableJvp: ...
    def vjp(
        self, cotangent: Array | _Float64Array | _DLPackProducer
    ) -> DifferentiableVjp: ...

@final
class DifferentiableProgram:
    """Immutable program over one fixed input-coordinate set.

    Authority: ``crates/eqiora-python/src/differentiation.rs::PyDifferentiableProgram``.
    """

    @property
    def model_digest(self) -> str: ...
    @property
    def realization_digest(self) -> str: ...
    @property
    def input_ids(self) -> list[str]: ...
    @property
    def output_id(self) -> str: ...
    @property
    def input_shape(self) -> tuple[int]: ...
    @property
    def output_shape(self) -> tuple[int]: ...
    @property
    def dtype(self) -> str: ...
    @property
    def device(self) -> str: ...
    @property
    def derivative_contract(self) -> str: ...
    def evaluate(
        self, parameters: Array | _Float64Array | _DLPackProducer
    ) -> DifferentiableEvaluation: ...
    def primal(self) -> DifferentiablePrimal: ...
    def jvp(
        self, tangent: Array | _Float64Array | _DLPackProducer
    ) -> DifferentiableJvp: ...
    def vjp(
        self, cotangent: Array | _Float64Array | _DLPackProducer
    ) -> DifferentiableVjp: ...

@final
class RunManifest:
    """Persisted exact run-v2 manifest linked to an accepted realization.

    Authority: ``crates/eqiora-python/src/realization.rs::PyRunManifest``.
    """

    @staticmethod
    def from_json(data: bytes, *, realization: Realization) -> RunManifest: ...
    @property
    def digest(self) -> str: ...
    @property
    def model_digest(self) -> str: ...
    @property
    def realization_digest(self) -> str: ...
    @property
    def semantic_revision(self) -> int: ...
    @property
    def output_digests(self) -> list[str]: ...
    @property
    def adapter(self) -> str: ...
    @property
    def adapter_version(self) -> str: ...
    @property
    def solver_backend(self) -> str: ...
    @property
    def solver_backend_version(self) -> str: ...
    @property
    def workers(self) -> int: ...
    @property
    def reduction(self) -> str: ...
    def to_json(self) -> bytes: ...
    def __eq__(self, other: object, /) -> bool: ...
    def __hash__(self) -> int: ...

@final
class ScalarEllipticResult:
    """Accepted scalar-elliptic result with producer/verifier evidence.

    Authority: ``crates/eqiora-python/src/realization.rs::PyScalarEllipticResult``.
    """

    @property
    def realization(self) -> Realization: ...
    @property
    def run_manifest(self) -> RunManifest: ...
    @property
    def elapsed_seconds(self) -> float: ...
    @property
    def field(self) -> ScalarFieldSummary: ...
    @property
    def values(self) -> Array: ...
    @property
    def balance(self) -> ScalarEllipticBalance: ...
    @property
    def solve(self) -> LinearSolveSummary: ...
    @property
    def output_fingerprint(self) -> str: ...

@final
class Series:
    """Read-only field-local sampled series in SI units.

    Authority: ``crates/eqiora-python/src/result.rs::PySeries``.
    """

    @property
    def id(self) -> str: ...
    @property
    def name(self) -> str | None: ...
    @property
    def dimension(self) -> tuple[int, int, int, int, int, int, int]: ...
    @property
    def time(self) -> Array: ...
    @property
    def values(self) -> Array: ...
    def __len__(self) -> int: ...
    def __iter__(self) -> Iterator[tuple[float, float]]: ...

@final
class Result:
    """Accepted execution occurrence with typed output relationships.

    Authority: ``crates/eqiora-python/src/result.rs::PyRunResult``.
    """

    @property
    def model_id(self) -> str: ...
    @property
    def model_digest(self) -> str: ...
    @property
    def model_revision(self) -> int: ...
    @property
    def plan_key(self) -> str: ...
    @property
    def adapter(self) -> str: ...
    @property
    def adapter_version(self) -> str: ...
    @property
    def elapsed_seconds(self) -> float: ...
    @property
    def fields(self) -> list[Series]: ...
    @property
    def snapshots(self) -> tuple[trajectory.FieldSnapshot, ...]: ...
    def field(self, field: FieldRef, /) -> trajectory.FieldSnapshot: ...
    def mesh(self, field: FieldRef, /) -> meshing.Mesh: ...
    @property
    def trajectory(self) -> trajectory.Trajectory: ...
    def run_manifest(self) -> RunManifest: ...
    def __len__(self) -> int: ...
    def __getitem__(self, key: str, /) -> Series: ...

@final
class RunStatus:
    """Monotone public state of one native execution occurrence.

    Authority: ``crates/eqiora-python/src/execution/evidence.rs::PyRunStatus``.
    """

    Created: ClassVar[RunStatus]
    Validating: ClassVar[RunStatus]
    Queued: ClassVar[RunStatus]
    Running: ClassVar[RunStatus]
    Cancelling: ClassVar[RunStatus]
    Cancelled: ClassVar[RunStatus]
    Completed: ClassVar[RunStatus]
    Failed: ClassVar[RunStatus]
    def __eq__(self, other: object, /) -> bool: ...
    def __hash__(self) -> int: ...

@final
class RunProgress:
    """Last coalesced fully accepted semantic-execution boundary.

    Authority: ``crates/eqiora-python/src/execution/evidence.rs::PyRunProgress``.
    """

    @property
    def model_time(self) -> float: ...
    @property
    def end_time(self) -> float: ...
    @property
    def accepted_steps(self) -> int: ...
    @property
    def maximum_steps(self) -> int: ...

@final
class RunCancellation:
    """Exact accepted boundary where cooperative cancellation terminated.

    Authority: ``crates/eqiora-python/src/execution/evidence.rs::PyRunCancellation``.
    """

    @property
    def progress(self) -> RunProgress: ...
    @property
    def elapsed_seconds(self) -> float: ...
    @property
    def plan_key(self) -> str: ...

@final
class ScalarEllipticRunProgress:
    """Last fully accepted scalar-elliptic application phase.

    Authority: ``crates/eqiora-python/src/execution/evidence.rs::PyScalarEllipticRunProgress``.
    """

    PlanReplayed: ClassVar[ScalarEllipticRunProgress]
    SystemFinalized: ClassVar[ScalarEllipticRunProgress]
    SolutionAccepted: ClassVar[ScalarEllipticRunProgress]
    def __eq__(self, other: object, /) -> bool: ...
    def __hash__(self) -> int: ...

@final
class ScalarEllipticRunCancellation:
    """Exact scalar-elliptic phase where cancellation terminated.

    Authority: ``crates/eqiora-python/src/execution/evidence.rs::PyScalarEllipticRunCancellation``.
    """

    @property
    def progress(self) -> ScalarEllipticRunProgress: ...
    @property
    def elapsed_seconds(self) -> float: ...
    @property
    def plan_key(self) -> str: ...

_RunResultT = TypeVar(
    "_RunResultT",
    Result,
    ScalarEllipticResult,
)

class Run(Generic[_RunResultT]):
    """Awaitable owner of one native execution occurrence.

    Authority: ``bindings/python/python/eqiora/__init__.py::Run``.
    """

    def __init__(self, native: Never) -> None: ...
    @property
    def status(self) -> RunStatus: ...
    @property
    def history(self) -> tuple[RunStatus, ...]: ...
    @property
    def progress(self) -> RunProgress | ScalarEllipticRunProgress | None: ...
    @property
    def cancellation(
        self,
    ) -> RunCancellation | ScalarEllipticRunCancellation | None: ...
    @property
    def done(self) -> bool: ...
    @property
    def model_id(self) -> str: ...
    @property
    def model_digest(self) -> str: ...
    @property
    def model_revision(self) -> int: ...
    @property
    def plan_key(self) -> str: ...
    @property
    def adapter(self) -> str: ...
    def cancel(self) -> bool: ...
    def result(self) -> _RunResultT: ...
    def __await__(self) -> Generator[Any, None, _RunResultT]: ...

_ModelDeclaration = (
    Domain
    | Representation
    | Field
    | Parameter
    | PhysicalDomain
    | ConservingPort
    | Relation
    | Connection
)

def across(port: ConservingPort) -> Expression:
    """Return the across variable of a scalar conserving port.

    Authority: ``crates/eqiora-python/src/modeling.rs::across``.
    """

    ...

def compile(
    source: str,
    *,
    filename: str = "<memory>",
) -> Model:
    """Compile one model through the canonical Rust pipeline.

    Authority: ``crates/eqiora-python/src/lib.rs::compile``.
    """

    ...

def bind_component(
    source: str,
    *,
    component: str,
    geometry: _geometry.Geometry,
    supports: dict[str, _geometry.GeometrySelection],
    parameters: dict[str, float],
    model: str = "Main",
    filename: str = "<memory>",
) -> Model:
    """Bind abstract Component supports to one exact Geometry.

    Authority: ``crates/eqiora-python/src/lib.rs::bind_component``.
    """

    ...

def compile_package(
    store_root: str | PathLike[str],
    resolution: bytes,
    *,
    entry_model: str,
) -> Model:
    """Compile one root-local model from a selected locked package store.

    Authority: ``crates/eqiora-python/src/package.rs::compile_package``.
    """

    ...

def check_package_conformance(
    store_root: str | os.PathLike[str],
    resolution_bytes: bytes,
    *,
    entry_model: str,
    profile: str,
) -> PackageConformanceReport:
    """Check one exact locked package closure by deterministic replay.

    Authority: ``crates/eqiora-python/src/package.rs::_check_package_conformance``.
    """

    ...

def connect(*ports: ConservingPort) -> Connection:
    """Build an anonymous conserving connection declaration.

    Authority: ``crates/eqiora-python/src/modeling.rs::connect``.
    """

    ...

def derivative(field: Field) -> Expression:
    """Return the time derivative of a field.

    Authority: ``crates/eqiora-python/src/modeling.rs::derivative``.
    """

    ...

def div(value: _ExpressionLike) -> Expression:
    """Return the spatial divergence of a symbolic expression.

    Authority: ``crates/eqiora-python/src/modeling.rs::div``.
    """

    ...

def grad(value: _ExpressionLike) -> Expression:
    """Return the spatial gradient of a symbolic expression.

    Authority: ``crates/eqiora-python/src/modeling.rs::grad``.
    """

    ...

def preview_realization(model: Model, request: ScalarElliptic) -> Realization:
    """Resolve a request before numerical allocation.

    Authority: ``crates/eqiora-python/src/realization.rs::preview_realization``.
    """

    ...

def replay(data: bytes) -> Model:
    """Replay one canonical artifact through the current model contract.

    Authority: ``crates/eqiora-python/src/lib.rs::replay``.
    """

    ...

def resolve(
    model: Model,
    /,
    *,
    mesh: meshing.Mesh,
    scales: IncompressibleFlowScales,
    solve: LinearSolve,
) -> fluid.SteadyStokesPlan:
    """Resolve the capability recognized from a Model using explicit policies.

    The current admitted composition recognizes bounded two-dimensional steady
    Stokes. The Model remains the sole authority for equations and boundary
    laws.

    Authority: ``crates/eqiora-python/src/steady_stokes.rs::resolve_model``.
    """

    ...

@overload
def run(
    model: Model,
    *,
    end_time: float,
    max_step: float,
    realization: None = None,
) -> Result:
    """Execute through the lifecycle returned by :func:`submit`.

    Authority: ``bindings/python/python/eqiora/__init__.py::run``.
    """

    ...

@overload
def run(model: Model, *, realization: Realization) -> ScalarEllipticResult:
    """Execute through the lifecycle returned by :func:`submit`.

    Authority: ``bindings/python/python/eqiora/__init__.py::run``.
    """

    ...

@overload
def run(
    model: Model,
    *,
    plan: fluid.SteadyStokesPlan,
) -> Result:
    """Execute through the lifecycle returned by :func:`submit`.

    Authority: ``bindings/python/python/eqiora/__init__.py::run``.
    """

    ...

@overload
def run(
    model: Model,
    *,
    plan: solid.LinearElasticityPlan,
) -> Result:
    """Execute through the lifecycle returned by :func:`submit`.

    Authority: ``bindings/python/python/eqiora/__init__.py::run``.
    """

    ...

@overload
def run(
    model: Model,
    *,
    plan: fsi.FixedMeshMonolithicPlan,
) -> Result:
    """Execute through the lifecycle returned by :func:`submit`.

    Authority: ``bindings/python/python/eqiora/__init__.py::run``.
    """

    ...

@overload
def submit(
    model: Model,
    *,
    end_time: float,
    max_step: float,
    realization: None = None,
) -> Run[Result]:
    """Submit one accepted temporal, spatial, or plan request shape.

    Authority: ``bindings/python/python/eqiora/__init__.py::submit``.
    """

    ...

@overload
def submit(model: Model, *, realization: Realization) -> Run[ScalarEllipticResult]:
    """Submit one accepted temporal, spatial, or plan request shape.

    Authority: ``bindings/python/python/eqiora/__init__.py::submit``.
    """

    ...

@overload
def submit(
    model: Model,
    *,
    plan: fluid.SteadyStokesPlan,
) -> Run[Result]:
    """Submit one accepted temporal, spatial, or plan request shape.

    Authority: ``bindings/python/python/eqiora/__init__.py::submit``.
    """

    ...

@overload
def submit(
    model: Model,
    *,
    plan: solid.LinearElasticityPlan,
) -> Run[Result]:
    """Submit one accepted temporal, spatial, or plan request shape.

    Authority: ``bindings/python/python/eqiora/__init__.py::submit``.
    """

    ...

@overload
def submit(
    model: Model,
    *,
    plan: fsi.FixedMeshMonolithicPlan,
) -> Run[Result]:
    """Submit one accepted temporal, spatial, or plan request shape.

    Authority: ``bindings/python/python/eqiora/__init__.py::submit``.
    """

    ...

def through(port: ConservingPort) -> Expression:
    """Return the through variable of a scalar conserving port.

    Authority: ``crates/eqiora-python/src/modeling.rs::through``.
    """

    ...

def trace(value: _ExpressionLike) -> Expression:
    """Return the boundary trace of a symbolic expression.

    Authority: ``crates/eqiora-python/src/modeling.rs::trace``.
    """

    ...

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
    "IncompressibleFlowScales",
    "LinearSolve",
    "LinearSolveSummary",
    "LinearizationState",
    "Model",
    "PackageConformancePackage",
    "PackageConformanceReport",
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
    "bind_component",
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
    "fluid",
    "fsi",
    "geometry",
    "meshing",
    "solid",
    "trajectory",
]
