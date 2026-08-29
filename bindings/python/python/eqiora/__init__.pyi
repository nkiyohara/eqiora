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

from . import fluid as fluid
from . import formulation as formulation
from . import fem as fem
from . import fsi as fsi
from . import fvm as fvm
from . import geometry as geometry
from . import lang as lang
from . import meshing as meshing
from . import solid as solid
from . import solve as solve
from . import solve as solve_module
from . import time as time
from . import trajectory as trajectory
from .viewer import View as View

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
class DomainRef:
    """Exact canonical Domain selected from one immutable Model.

    Authority: ``crates/eqiora-python/src/model.rs::PyModelDomainRef``.
    """
    @property
    def model_digest(self) -> str: ...
    @property
    def id(self) -> str: ...
    def __eq__(self, other: object, /) -> bool: ...
    def __hash__(self) -> int: ...

@final
class InitialField:
    """Immutable exact-Field-bound coherent-SI initial coefficients.

    Authority: ``crates/eqiora-python/src/trajectory.rs::PyInitialField``.
    """
    def __new__(
        cls,
        field: FieldRef,
        /,
        *,
        vertex_values: object | None = None,
        cell_values: object | None = None,
    ) -> InitialField: ...
    @property
    def field(self) -> FieldRef: ...

@final
class FieldOutput:
    """Immutable coefficients for one exact Model Field on one exact Mesh.

    Authority: ``crates/eqiora-python/src/result/field_output.rs::PyFieldOutput``.
    """

    @property
    def field(self) -> FieldRef: ...
    @property
    def mesh(self) -> meshing.Mesh: ...
    @property
    def dimension(self) -> tuple[int, int, int, int, int, int, int]: ...
    @property
    def value_shape(self) -> tuple[int, ...]: ...
    @property
    def space(self) -> str: ...
    @property
    def associations(self) -> tuple[str, ...]: ...
    def values(self, association: str, /) -> Array: ...
    def coefficient_count(self, association: str, /) -> int: ...
    def logical_shape(self, association: str, /) -> tuple[int, ...]: ...

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
    @staticmethod
    def from_bytes(data: bytes) -> Model: ...
    @staticmethod
    def read(path: str | PathLike[str]) -> Model: ...
    def to_bytes(self) -> bytes: ...
    def write(self, path: str | PathLike[str]) -> None: ...
    def preview_value_edit(self, target: str, value: float) -> ValueEdit: ...
    def commit(self, edit: ValueEdit) -> Model: ...
    def parameter(self, selection: str) -> ParameterRef: ...
    def field(self, selection: str) -> FieldRef: ...
    def domain(self, selection: str) -> DomainRef: ...
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
    @property
    def domain_ids(self) -> list[str]: ...
    def __eq__(self, other: object, /) -> bool: ...
    def __hash__(self) -> int: ...

@final
class ScalarPlanView:
    """Scalar-elliptic field roles resolved from one Model.

    Authority: ``crates/eqiora-python/src/common_plan/capability_view.rs::PyScalarPlanView``.
    """
    @property
    def kind(self) -> str: ...
    @property
    def field(self) -> FieldRef: ...

@final
class ResolvedExecution:
    """Exact scalar, layout, schedule, provider, and placement selected for execution.

    Authority: ``crates/eqiora-python/src/common_plan/resolved_execution.rs::PyResolvedExecution``.
    """
    @property
    def scalar_type(self) -> str: ...
    @property
    def vector_layout(self) -> str: ...
    @property
    def schedule(self) -> str: ...
    @property
    def provider(self) -> str: ...
    @property
    def provider_version(self) -> str: ...
    @property
    def placement(self) -> str: ...
    @property
    def workers(self) -> int: ...
    def __repr__(self) -> str: ...

@final
class FormulationKind:
    """Closed mathematical Formulation families accepted by exact override.

    Authority: ``crates/eqiora-python/src/common_plan/capability_view.rs::PyFormulationKind``.
    """

    PrimalGalerkin: ClassVar[FormulationKind]
    MixedGalerkin: ClassVar[FormulationKind]
    IntegralConservative: ClassVar[FormulationKind]
    def __eq__(self, other: object, /) -> bool: ...
    def __hash__(self) -> int: ...

@final
class FormulationSelectionMode:
    """Whether resolution selected or admitted an exact Formulation.

    Authority: ``crates/eqiora-python/src/common_plan/capability_view.rs::PyFormulationSelectionMode``.
    """

    Automatic: ClassVar[FormulationSelectionMode]
    Exact: ClassVar[FormulationSelectionMode]
    def __eq__(self, other: object, /) -> bool: ...
    def __hash__(self) -> int: ...

@final
class FormulationView:
    """Effective mathematical form selected between Model and Realization.

    Authority: ``crates/eqiora-python/src/common_plan/capability_view.rs::PyFormulationView``.
    """
    @property
    def requested(self) -> FormulationSelectionMode: ...
    @property
    def effective(self) -> FormulationKind: ...
    @property
    def boundary_treatment(self) -> str: ...
    @property
    def rule_ids(self) -> list[str]: ...
    @property
    def selection_reason_codes(self) -> list[str]: ...
    def __repr__(self) -> str: ...

@final
class Plan:
    """Immutable common numerical Plan owning an exact Model and applicable resources.

    The Model alone determines the admitted physics. ``capability`` exposes
    capability-specific field roles and policies through one closed typed view;
    ``fields`` remains the capability-neutral exact FieldRef inventory.

    Authority: ``crates/eqiora-python/src/common_plan.rs::PyPlan``.
    """
    @staticmethod
    def from_bytes(data: bytes) -> Plan: ...
    def to_bytes(self) -> bytes: ...
    @property
    def identity(self) -> str: ...
    @property
    def model_id(self) -> str: ...
    @property
    def model_digest(self) -> str: ...
    @property
    def model_revision(self) -> int: ...
    @property
    def package_compilation_digest(self) -> str | None: ...
    @property
    def geometry_digest(self) -> str | None: ...
    @property
    def mesh_digest(self) -> str | None: ...
    @property
    def correspondence_digest(self) -> str | None: ...
    @property
    def production_digest(self) -> str | None: ...
    @property
    def realization_digest(self) -> str | None: ...
    @property
    def model(self) -> Model: ...
    @property
    def mesh(self) -> meshing.Mesh | None: ...
    @property
    def formulation(self) -> FormulationView | None: ...
    @property
    def capability(self) -> ScalarPlanView | time.OdePlanView | solid.ElasticityPlanView | fluid.IncompressibleFlowPlanView | fsi.FixedReferenceFsiPlanView: ...
    @property
    def fields(self) -> tuple[FieldRef, ...]: ...
    @property
    def spatial(self) -> fem.Q1 | fem.MiniP1 | fvm.CellCenteredTpfa | fvm.CellCentered | tuple[fem.ScopedSpatialPolicy, ...] | None: ...
    @property
    def solve(self) -> solve_module.ResolvedLinear | solve_module.ResolvedNewton | None: ...
    @property
    def requested_solve(self) -> solve_module.Linear | solve_module.Newton | None: ...
    @property
    def temporal(self) -> time.BackwardEuler | time.Tsitouras45 | None: ...
    @property
    def execution(self) -> ResolvedExecution: ...
    def __repr__(self) -> str: ...
    def __eq__(self, other: object, /) -> bool: ...
    def __hash__(self) -> int: ...

@final
class State:
    """Accepted physical state owned by one exact common Plan.

    Authority: ``crates/eqiora-python/src/trajectory.rs::PyState``.
    """

    def to_bytes(self) -> bytes: ...
    @staticmethod
    def from_bytes(plan: Plan, data: bytes) -> State: ...
    @staticmethod
    def initial(
        plan: Plan,
        /,
        *,
        fields: tuple[InitialField, ...] | None = None,
        time_s: float | None = None,
    ) -> State: ...
    @staticmethod
    def zero(plan: Plan, /, *, time_s: float = 0.0) -> State: ...
    @staticmethod
    def from_result(plan: Plan, result: Result, /, *, time_s: float) -> State: ...
    @property
    def digest(self) -> str: ...
    @property
    def step(self) -> int: ...
    @property
    def time_s(self) -> float: ...
    @property
    def fields(self) -> tuple[trajectory.FieldSnapshot, ...]: ...
    @property
    def field_refs(self) -> tuple[FieldRef, ...]: ...
    @property
    def state_space_identity(self) -> str: ...
    @property
    def mesh(self) -> meshing.Mesh | None: ...
    @property
    def model(self) -> Model | None: ...
    @property
    def source_plan_identity(self) -> str | None: ...
    @property
    def source_request_identity(self) -> str | None: ...
    @property
    def source_trajectory_identity(self) -> str | None: ...
    @property
    def source_kind(self) -> str | None: ...
    def field(self, field: FieldRef, /) -> trajectory.FieldSnapshot: ...
    def curl(self, field: FieldRef, /) -> trajectory.DerivedFieldSnapshot: ...
    def sample(
        self,
        field: FieldRef,
        /,
        *,
        at: tuple[float, float],
    ) -> trajectory.FieldSample: ...
    def boundary_force(
        self,
        selection: geometry.GeometrySelection,
        /,
    ) -> trajectory.BoundaryForce: ...
    def value(self, field: FieldRef, /) -> float: ...
    def __eq__(self, other: object, /) -> bool: ...
    def __hash__(self) -> int: ...

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
    def plan_identity(self) -> str: ...
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
    def plan_identity(self) -> str: ...
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
class Series:
    """Read-only field-local sampled series in SI units.

    Authority: ``crates/eqiora-python/src/result.rs::PySeries``.
    """

    @property
    def field(self) -> FieldRef | None: ...
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
    def to_bytes(self) -> bytes: ...
    @staticmethod
    def from_bytes(plan: Plan, data: bytes) -> Result: ...
    @property
    def fields(self) -> list[Series]: ...
    @property
    def solve(self) -> LinearSolveSummary: ...
    def output(self, field: FieldRef, /) -> FieldOutput: ...
    def boundary_force(
        self, selection: geometry.GeometrySelection, /
    ) -> trajectory.BoundaryForce: ...
    def boundary_flux(
        self, selection: geometry.GeometrySelection, /
    ) -> trajectory.BoundaryFlux: ...
    def series(self, field: FieldRef, /) -> Series: ...
    def mesh(self, field: FieldRef, /) -> meshing.Mesh: ...
    @property
    def trajectory(self) -> trajectory.Trajectory: ...

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
class TransientRunProgress:
    """Last fully accepted common transient step boundary.

    Authority: ``crates/eqiora-python/src/execution/evidence.rs::PyCommonTransientRunProgress``.
    """

    @property
    def accepted_steps(self) -> int: ...
    @property
    def maximum_steps(self) -> int: ...
    @property
    def model_time_s(self) -> float: ...

@final
class TransientRunCancellation:
    """Exact accepted common transient boundary where cancellation terminated.

    Authority: ``crates/eqiora-python/src/execution/evidence.rs::PyCommonTransientRunCancellation``.
    """

    @property
    def progress(self) -> TransientRunProgress: ...
    @property
    def request_identity(self) -> str: ...

_RunResultT = TypeVar("_RunResultT", bound=Result)

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
    def progress(self) -> TransientRunProgress | None: ...
    @property
    def cancellation(
        self,
    ) -> TransientRunCancellation | None: ...
    @property
    def done(self) -> bool: ...
    @property
    def model_id(self) -> str: ...
    @property
    def model_digest(self) -> str: ...
    @property
    def model_revision(self) -> int: ...
    @property
    def package_compilation_digest(self) -> str | None: ...
    @property
    def plan_key(self) -> str: ...
    @property
    def adapter(self) -> str: ...
    @property
    def adapter_version(self) -> str: ...
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
    *,
    path: str | PathLike[str] | None = None,
    source: str | lang.Source | None = None,
    filename: str | None = None,
    geometry: geometry.Geometry | None = None,
    parameters: dict[str, float | int] | None = None,
    component: str | None = None,
) -> Model:
    """Compile one source and its optional exact Geometry closure.

    Authority: ``bindings/python/python/eqiora/__init__.py::compile``.
    """

    ...

def compile_package(
    store_root: str | PathLike[str],
    resolution: bytes,
    *,
    geometry: geometry.Geometry,
    component: str,
    parameters: dict[str, float | int] | None = None,
) -> Model:
    """Compile one root-package Component against caller-owned Geometry.

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

def resolve(
    model: Model,
    *,
    mesh: meshing.Mesh | None = None,
    spatial: fem.Q1 | fem.MiniP1 | fvm.CellCenteredTpfa | fvm.CellCentered | tuple[fem.ScopedSpatialPolicy, ...] | None = None,
    formulation: FormulationKind | None = None,
    solve: solve.Linear | solve.Newton | None = None,
    scaling: fluid.IncompressibleScaling | None = None,
    temporal: time.BackwardEuler | time.Tsitouras45 | None = None,
) -> Plan:
    """Resolve an exact Model and typed numerical policies into a common Plan.

    Typed spatial and solve policies select numerics, never physics. The
    resolved Plan retains the exact caller Model and every applicable caller
    resource. Spatial paths retain their exact Mesh without regeneration;
    structural no-Mesh ODE paths reject spatial resources.

    Authority: ``bindings/python/python/eqiora/__init__.py::resolve``.
    """

    ...

def run(
    plan: Plan,
    *,
    state: State | None = None,
    until_s: float | None = None,
    output_times_s: tuple[float, ...] | None = None,
    steps: int | None = None,
    output_steps: tuple[int, ...] | None = None,
) -> Result:
    """Execute one steady or explicitly bounded transient common Plan synchronously.

    Authority: ``bindings/python/python/eqiora/__init__.py::run``.
    """

    ...

def submit(
    plan: Plan,
    *,
    state: State | None = None,
    until_s: float | None = None,
    output_times_s: tuple[float, ...] | None = None,
    steps: int | None = None,
    output_steps: tuple[int, ...] | None = None,
) -> Run[Result]:
    """Submit one steady or explicitly bounded transient common Plan.

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

from . import diff as diff

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
