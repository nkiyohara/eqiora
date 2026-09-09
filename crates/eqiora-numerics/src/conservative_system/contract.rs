use eqiora_artifact::{AcceptedModelArtifact, CanonicalModelArtifact, ModelArtifactReference};
use eqiora_core::{
    Diagnostic, DimExponents, DynQuantity, RawId, ScalarDomain, ValueType, diagnostic::codes,
};
use eqiora_graph::EdgeKind;
use eqiora_realization::{MeshArtifactReference, Space};
use eqiora_schema::kernel::{BoundarySide, DomainKind, KernelNode};
use eqiora_sem::KernelProgram;

use super::RusanovPolicy;

const MAX_WIDTH: usize = 16;
const MAX_CELLS: usize = 100_000;
const MAX_FACES: usize = 400_000;
const MAX_WORK_VALUES: usize = 8_000_000;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Component {
    pub(crate) field: RawId,
    pub(crate) value_type: ValueType,
}

/// Input interpretation is explicit, not inferred from shape or equal numbers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CellValueMeaning {
    Average,
    IntegratedInventory,
    PointSample,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SemanticContract {
    pub(super) model: ModelArtifactReference,
    pub(super) domain: RawId,
    pub(super) components: Vec<Component>,
    pub(super) bounds: Vec<[f64; 2]>,
    pub(super) boundaries: Vec<RawId>,
}

impl SemanticContract {
    /// Called only after the semantic adapter has recognized its actual equations.
    pub(super) fn from_program(
        program: &KernelProgram,
        domain: RawId,
        fields: &[RawId],
        boundaries: &[RawId],
    ) -> Result<Self, Diagnostic> {
        if fields.is_empty() || fields.len() > MAX_WIDTH {
            return Err(invalid("conservative width exceeds admitted bound"));
        }
        let domain_id = domain
            .downcast()
            .ok_or_else(|| invalid("wrong Domain entity kind"))?;
        let bounds = program
            .resolved_cartesian_bounds(domain_id)?
            .iter()
            .map(|bound| [bound.lower().value(), bound.upper().value()])
            .collect::<Vec<_>>();
        let mut components = reserve(fields.len())?;
        for (index, field) in fields.iter().enumerate() {
            let Some(KernelNode::Field(definition)) = program.node(*field) else {
                return Err(invalid("missing conservative Field"));
            };
            if fields[..index].contains(field)
                || !program.edges().iter().any(|edge| {
                    edge.from() == *field
                        && edge.to() == domain
                        && edge.kind() == EdgeKind::DefinedOn
                })
            {
                return Err(invalid(
                    "duplicate or foreign conservative Field occurrence",
                ));
            }
            components.push(Component {
                field: *field,
                value_type: definition.value_type().clone(),
            });
        }
        for (index, boundary) in boundaries.iter().enumerate() {
            let Some(KernelNode::Domain(definition)) = program.node(*boundary) else {
                return Err(invalid("missing boundary Domain"));
            };
            let DomainKind::CartesianBoundary { axis, side } = definition.kind() else {
                return Err(invalid("boundary is not a Cartesian side"));
            };
            if *axis != index / 2
                || *side
                    != if index % 2 == 0 {
                        BoundarySide::Lower
                    } else {
                        BoundarySide::Upper
                    }
                || !program.edges().iter().any(|edge| {
                    edge.from() == *boundary
                        && edge.to() == domain
                        && edge.kind() == EdgeKind::BoundaryOf
                })
            {
                return Err(invalid("boundary order or parent occurrence differs"));
            }
        }
        let model = AcceptedModelArtifact::from_program(program)?.artifact_reference()?;
        Ok(Self {
            model,
            domain,
            components,
            bounds,
            boundaries: boundaries.to_vec(),
        })
    }

    pub(super) fn validate<const D: usize>(&self) -> Result<(), Diagnostic> {
        if !(1..=2).contains(&D) || self.bounds.len() != D || self.boundaries.len() != 2 * D {
            return Err(invalid(
                "conservative action supports exact 1D or 2D Cartesian geometry",
            ));
        }
        if self.components.is_empty()
            || self.components.len() > MAX_WIDTH
            || self.components.iter().any(|component| {
                component.value_type.scalar_domain() != ScalarDomain::Real
                    || !component.value_type.shape().is_scalar()
            })
        {
            return Err(invalid(
                "conservative components require bounded distinct real scalar Fields",
            ));
        }
        Ok(())
    }

    pub(crate) fn components(&self) -> &[Component] {
        &self.components
    }
    pub(crate) fn boundaries(&self) -> &[RawId] {
        &self.boundaries
    }
    pub(crate) fn model(&self) -> &ModelArtifactReference {
        &self.model
    }
}

/// Crate-private semantic seam: no user callback or registry enters the face loop.
pub(crate) trait ConservativePhysics<const D: usize>: Clone {
    fn contract(&self) -> &SemanticContract;
    fn admit(&self, state: &[f64]) -> Result<(), Diagnostic>;
    /// Physical flux per unit area; the normal carries orientation, never area.
    fn normal_flux_and_wave_bound(
        &self,
        state: &[f64],
        normal: &[f64; D],
    ) -> Result<(Vec<DynQuantity>, DynQuantity), Diagnostic>;
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct Identity {
    pub(super) semantic: SemanticContract,
    pub(super) mesh: MeshArtifactReference,
    pub(super) policy: RusanovPolicy,
    pub(super) space: Space,
}

pub(super) fn validate_work<const D: usize>(
    width: usize,
    cells: usize,
    faces: usize,
) -> Result<(), Diagnostic> {
    if !(1..=2).contains(&D)
        || width == 0
        || width > MAX_WIDTH
        || cells == 0
        || cells > MAX_CELLS
        || faces == 0
        || faces > MAX_FACES
    {
        return Err(invalid(
            "conservative action width/cell/face resource bound exceeded",
        ));
    }
    // Numerical f64 slots: geometry, state + inventory, two simultaneous receipts
    // during replay, and bounded boundary/sampling buffers. At each face the two
    // receipts use 4k flux/integral entries + 2D normals + four area/wave entries;
    // geometry adds D+2. At each cell state/inventory and two residual/rate pairs
    // use 6k, while geometry adds D+1. Container/index metadata is bounded by the
    // separate width/cell/face caps; already admitted Mesh/Model inputs retain
    // their existing resource owners rather than being charged as f64 buffers.
    let scalars = faces
        .checked_mul(4 * width + 3 * D + 6)
        .and_then(|n| {
            cells
                .checked_mul(6 * width + D + 1)
                .and_then(|c| n.checked_add(c))
        })
        .and_then(|n| n.checked_add((8 + 2 * D) * width + 2 * D * (5 + 2 * D)))
        .ok_or_else(|| invalid("conservative work shape overflow"))?;
    if scalars > MAX_WORK_VALUES {
        return Err(invalid("conservative action allocation budget exceeded"));
    }
    Ok(())
}

pub(super) fn reserve<T>(count: usize) -> Result<Vec<T>, Diagnostic> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| invalid("conservative action allocation exceeds platform capacity"))?;
    Ok(values)
}
pub(super) const fn length_dimension() -> DimExponents {
    DimExponents::from_integers([0, 1, 0, 0, 0, 0, 0]).expect("bounded")
}
pub(super) const fn velocity_dimension() -> DimExponents {
    DimExponents::from_integers([0, 1, -1, 0, 0, 0, 0]).expect("bounded")
}
pub(super) fn invalid(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::NUMERICAL_SOLVE_FAILED, message)
}
