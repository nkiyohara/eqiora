//! The shape of what a scalar-elliptic run returns.
//!
//! A solved field is reported through a Cartesian projection with an explicit
//! value ordering, summarized into finite extrema, and accompanied by the
//! balance evidence that lets a caller check the result without re-solving.

use super::diagnostic::{capability_error, single};
use super::plan::{ScalarEllipticIntent, ScalarEllipticMethod, ScalarEllipticRunPlan};
use crate::ModelDocument;
use eqiora_artifact::RunManifestV2;
use eqiora_assembly::AssemblyReport;
use eqiora_core::entity::kinds;
use eqiora_core::{Diagnostic, DimExponents, Id};
use eqiora_execution::ExecutionReceipt;
use eqiora_meshing::MeshTopology;
use eqiora_numerics::{
    common::CartesianMesh, scalar::ResolvedScalarEllipticCartesianSolution,
    scalar::ScalarEllipticCartesianModel,
};
use eqiora_schema::kernel::KernelNode;
use eqiora_solver::SolveReport;
use std::time::Duration;

/// Location semantics of values summarized by an accepted spatial result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScalarFieldLocation {
    /// Continuous finite-element values at canonical mesh vertices.
    Vertex,
    /// Finite-volume algebraic values at canonical cell centres.
    CellCenter,
}

/// Canonical value order of one generated Cartesian Field projection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CartesianFieldOrder {
    /// The final logical axis varies fastest in the accepted value array.
    LastAxisFastest,
}

/// Method-neutral semantic layout of the primary scalar Field in one plan.
///
/// This application projection is derived during preview, before mesh or
/// result allocation. It contains no run identity, transport encoding, cache
/// policy, renderer state, or durable result artifact.
#[derive(Debug, Clone, PartialEq)]
pub struct CartesianScalarFieldProjection {
    field: Id<kinds::Field>,
    preferred_alias: Option<String>,
    value_dimension: DimExponents,
    domain: Id<kinds::Domain>,
    spatial_dimension: usize,
    bounds: [[f64; 2]; 3],
    location: ScalarFieldLocation,
    logical_shape: [usize; 3],
    value_count: usize,
}

impl CartesianScalarFieldProjection {
    /// Canonical scalar Field identity.
    #[must_use]
    pub const fn field(&self) -> Id<kinds::Field> {
        self.field
    }

    /// Deterministic non-semantic source alias, when the document retained one.
    #[must_use]
    pub fn preferred_alias(&self) -> Option<&str> {
        self.preferred_alias.as_deref()
    }

    /// Physical dimension of each Field value.
    #[must_use]
    pub const fn value_dimension(&self) -> DimExponents {
        self.value_dimension
    }

    /// Canonical Cartesian volume Domain identity.
    #[must_use]
    pub const fn domain(&self) -> Id<kinds::Domain> {
        self.domain
    }

    /// Number of coherent-SI Cartesian coordinate axes.
    #[must_use]
    pub const fn spatial_dimension(&self) -> usize {
        self.spatial_dimension
    }

    /// Coherent-SI coordinate bounds in canonical axis order.
    #[must_use]
    pub fn bounds(&self) -> &[[f64; 2]] {
        &self.bounds[..self.spatial_dimension]
    }

    /// Vertex or cell-centre association selected by the Realization.
    #[must_use]
    pub const fn location(&self) -> ScalarFieldLocation {
        self.location
    }

    /// Per-axis value extents in canonical Cartesian axis order.
    #[must_use]
    pub fn logical_shape(&self) -> &[usize] {
        &self.logical_shape[..self.spatial_dimension]
    }

    /// Number of values admitted before allocation.
    #[must_use]
    pub const fn value_count(&self) -> usize {
        self.value_count
    }

    /// Canonical flattening order of complete accepted values.
    #[must_use]
    pub const fn order(&self) -> CartesianFieldOrder {
        CartesianFieldOrder::LastAxisFastest
    }

    pub(super) fn matches_summary(&self, summary: ScalarFieldSummary) -> bool {
        self.location == summary.location()
            && self.spatial_dimension == summary.spatial_dimension()
            && self.logical_shape() == summary.logical_shape()
            && self.value_count == summary.value_count()
    }
}

/// Bounded scalar result summary; complete arrays stay on the data plane.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScalarFieldSummary {
    location: ScalarFieldLocation,
    spatial_dimension: usize,
    logical_shape: [usize; 3],
    value_count: usize,
    minimum: f64,
    maximum: f64,
}

impl ScalarFieldSummary {
    /// Vertex or cell-centre meaning of the summarized values.
    #[must_use]
    pub const fn location(self) -> ScalarFieldLocation {
        self.location
    }

    /// Number of physical Cartesian axes in this Field layout.
    #[must_use]
    pub const fn spatial_dimension(self) -> usize {
        self.spatial_dimension
    }

    /// Per-axis Field extents in canonical Cartesian axis order.
    #[must_use]
    pub fn logical_shape(&self) -> &[usize] {
        &self.logical_shape[..self.spatial_dimension]
    }

    /// Number of finite scalar values summarized.
    #[must_use]
    pub const fn value_count(self) -> usize {
        self.value_count
    }

    /// Minimum accepted field value.
    #[must_use]
    pub const fn minimum(self) -> f64 {
        self.minimum
    }

    /// Maximum accepted field value.
    #[must_use]
    pub const fn maximum(self) -> f64 {
        self.maximum
    }
}

/// Continuous conservation evidence independent from the linear residual.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScalarEllipticBalanceEvidence {
    boundary_total: f64,
    integrated_source: f64,
    relative_imbalance: f64,
}

impl ScalarEllipticBalanceEvidence {
    /// Recovered outward reaction or flux total.
    #[must_use]
    pub const fn boundary_total(self) -> f64 {
        self.boundary_total
    }

    /// Source integral represented by the discrete load/balance equations.
    #[must_use]
    pub const fn integrated_source(self) -> f64 {
        self.integrated_source
    }

    /// `|boundary + source| / (|boundary| + |source|)`.
    #[must_use]
    pub const fn relative_imbalance(self) -> f64 {
        self.relative_imbalance
    }
}

/// Successful result and exact producer/verifier evidence.
#[derive(Debug, PartialEq)]
pub struct ScalarEllipticRunResult {
    pub(super) plan: ScalarEllipticRunPlan,
    pub(super) elapsed: Duration,
    pub(super) field: ScalarFieldSummary,
    pub(super) field_values: Vec<f64>,
    pub(super) balance: ScalarEllipticBalanceEvidence,
    pub(super) assembly: AssemblyReport,
    pub(super) run_manifest: RunManifestV2,
    pub(super) receipt: ExecutionReceipt,
}

impl ScalarEllipticRunResult {
    /// Exact plan replayed immediately before allocation.
    #[must_use]
    pub const fn plan(&self) -> &ScalarEllipticRunPlan {
        &self.plan
    }

    /// Wall duration measured by this local application operation.
    #[must_use]
    pub const fn elapsed(&self) -> Duration {
        self.elapsed
    }

    /// Bounded primary field summary.
    #[must_use]
    pub const fn field(&self) -> ScalarFieldSummary {
        self.field
    }

    /// Complete accepted primary Field values in canonical location order.
    #[must_use]
    pub fn field_values(&self) -> &[f64] {
        &self.field_values
    }

    /// Consume this result into its complete primary Field values.
    #[must_use]
    pub fn into_field_values(self) -> Vec<f64> {
        self.field_values
    }

    /// Recovered continuous conservation evidence.
    #[must_use]
    pub const fn balance(&self) -> ScalarEllipticBalanceEvidence {
        self.balance
    }

    /// Accepted local assembly placement and shape evidence.
    #[must_use]
    pub const fn assembly(&self) -> AssemblyReport {
        self.assembly
    }

    /// Versioned Model, Realization, and actual execution provenance.
    #[must_use]
    pub const fn run_manifest(&self) -> &RunManifestV2 {
        &self.run_manifest
    }

    /// Independently accepted linear solve report.
    #[must_use]
    pub const fn solve(&self) -> &SolveReport {
        self.receipt.report()
    }

    /// Immutable deployment, operator, plan, and execution-DAG evidence.
    #[must_use]
    pub const fn receipt(&self) -> &ExecutionReceipt {
        &self.receipt
    }
}

pub(super) fn scalar_field_projection(
    document: &ModelDocument,
    model: &ScalarEllipticCartesianModel,
    intent: ScalarEllipticIntent,
    value_count: usize,
) -> Result<CartesianScalarFieldProjection, Diagnostic> {
    let spatial_dimension = model.dimension();
    if !(1..=3).contains(&spatial_dimension) {
        return Err(capability_error(format!(
            "scalar Field projection does not admit {spatial_dimension} Cartesian dimensions"
        )));
    }
    let field = model.field_id();
    let Some(KernelNode::Field(field_definition)) = document.program().node(field.erase()) else {
        return Err(capability_error(
            "scalar Field projection names a missing canonical Field",
        ));
    };
    let value_dimension = document
        .program()
        .value(field.erase())
        .map(|value| value.dim())
        .unwrap_or_else(|| field_definition.dimension());
    if value_dimension != field_definition.dimension() {
        return Err(capability_error(
            "scalar Field value dimension differs from its canonical definition",
        ));
    }
    let domain = model.domain_id();
    let preferred_alias = document
        .aliases()
        .iter()
        .find_map(|(name, &id)| (id == field.erase()).then(|| name.clone()));
    let mut bounds = [[0.0; 2]; 3];
    bounds[..spatial_dimension].copy_from_slice(model.bounds());
    let location = match intent.method {
        ScalarEllipticMethod::FiniteElement => ScalarFieldLocation::Vertex,
        ScalarEllipticMethod::FiniteVolume => ScalarFieldLocation::CellCenter,
    };
    let axis_extent = match location {
        ScalarFieldLocation::Vertex => intent.cells_per_axis.get().checked_add(1),
        ScalarFieldLocation::CellCenter => Some(intent.cells_per_axis.get()),
    }
    .ok_or_else(|| capability_error("scalar Field projection shape overflowed"))?;
    let mut logical_shape = [1; 3];
    logical_shape[..spatial_dimension].fill(axis_extent);
    let projected_count = logical_shape[..spatial_dimension]
        .iter()
        .try_fold(1_usize, |count, extent| count.checked_mul(*extent))
        .ok_or_else(|| capability_error("scalar Field projection value count overflowed"))?;
    if projected_count != value_count {
        return Err(capability_error(format!(
            "scalar Field projection describes {projected_count} values, but the plan admits {value_count}"
        )));
    }
    Ok(CartesianScalarFieldProjection {
        field,
        preferred_alias,
        value_dimension,
        domain,
        spatial_dimension,
        bounds,
        location,
        logical_shape,
        value_count,
    })
}

pub(super) fn summarize(
    solution: &ResolvedScalarEllipticCartesianSolution,
) -> Result<
    (
        ScalarFieldSummary,
        ScalarEllipticBalanceEvidence,
        AssemblyReport,
        SolveReport,
    ),
    Vec<Diagnostic>,
> {
    let (field, boundary, source, assembly, solve) = match solution {
        ResolvedScalarEllipticCartesianSolution::FiniteElement(solution) => {
            let field = summarize_field(
                solution.field().vertex_values(),
                solution.field().mesh(),
                ScalarFieldLocation::Vertex,
            )?;
            (
                field,
                solution.boundary_reaction_sum(),
                solution.integrated_source(),
                *solution.assembly_report(),
                solution.solve_report().clone(),
            )
        }
        ResolvedScalarEllipticCartesianSolution::FiniteVolume(solution) => {
            let field = summarize_field(
                solution.cell_values(),
                solution.mesh(),
                ScalarFieldLocation::CellCenter,
            )?;
            (
                field,
                solution.boundary_flux_sum(),
                solution.integrated_source(),
                *solution.assembly_report(),
                solution.solve_report().clone(),
            )
        }
    };
    let relative_imbalance =
        (boundary + source).abs() / (boundary.abs() + source.abs()).max(f64::MIN_POSITIVE);
    if !relative_imbalance.is_finite() {
        return Err(single(capability_error(
            "continuous balance evidence is non-finite",
        )));
    }
    Ok((
        field,
        ScalarEllipticBalanceEvidence {
            boundary_total: boundary,
            integrated_source: source,
            relative_imbalance,
        },
        assembly,
        solve,
    ))
}

pub(super) fn summarize_field(
    values: &[f64],
    mesh: &CartesianMesh,
    location: ScalarFieldLocation,
) -> Result<ScalarFieldSummary, Vec<Diagnostic>> {
    let spatial_dimension = mesh.topological_dimension();
    if !(1..=3).contains(&spatial_dimension) {
        return Err(single(capability_error(format!(
            "accepted Cartesian Field has unsupported dimension {spatial_dimension}"
        ))));
    }
    let mut logical_shape = [1_usize; 3];
    for (axis, extent) in logical_shape[..spatial_dimension].iter_mut().enumerate() {
        let cells = mesh.axis_cell_count(axis).ok_or_else(|| {
            single(capability_error(
                "accepted Cartesian Field is missing an axis extent",
            ))
        })?;
        *extent = match location {
            ScalarFieldLocation::Vertex => cells.checked_add(1).ok_or_else(|| {
                single(capability_error(
                    "accepted Cartesian vertex Field shape overflowed",
                ))
            })?,
            ScalarFieldLocation::CellCenter => cells,
        };
    }
    let expected_count = logical_shape[..spatial_dimension]
        .iter()
        .try_fold(1_usize, |count, extent| count.checked_mul(*extent))
        .ok_or_else(|| {
            single(capability_error(
                "accepted Cartesian Field shape overflowed",
            ))
        })?;
    if expected_count != values.len() {
        return Err(single(capability_error(format!(
            "accepted Cartesian Field shape describes {expected_count} values, but the solution contains {}",
            values.len()
        ))));
    }
    let (minimum, maximum) = finite_range(values)?;
    Ok(ScalarFieldSummary {
        location,
        spatial_dimension,
        logical_shape,
        value_count: values.len(),
        minimum,
        maximum,
    })
}

pub(super) fn finite_range(values: &[f64]) -> Result<(f64, f64), Vec<Diagnostic>> {
    let Some((&first, rest)) = values.split_first() else {
        return Err(single(capability_error(
            "accepted scalar field unexpectedly contains no values",
        )));
    };
    if !first.is_finite() {
        return Err(single(capability_error(
            "accepted scalar field contains a non-finite value",
        )));
    }
    rest.iter()
        .try_fold((first, first), |(minimum, maximum), &value| {
            value
                .is_finite()
                .then_some((minimum.min(value), maximum.max(value)))
                .ok_or_else(|| {
                    single(capability_error(
                        "accepted scalar field contains a non-finite value",
                    ))
                })
        })
}
