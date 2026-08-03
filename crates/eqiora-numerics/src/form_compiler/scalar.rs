//! Private proof-carrying FEM derivation for the bounded Cartesian Q1 slice.

use std::collections::{BTreeMap, BTreeSet};

use eqiora_assembly::LocalContribution;
use eqiora_core::diagnostic::codes;
use eqiora_core::entity::kinds;
use eqiora_core::{Diagnostic, RawId};
use eqiora_graph::EdgeKind;
use eqiora_meshing::{AffineGeometryMap, GeometryMap, QuadratureRule, ReferenceCellFamily};
use eqiora_schema::kernel::typing::TypedResidual;
use eqiora_schema::kernel::{
    ActivationKind, BoundarySide, DomainKind, ExprDag, ExprId, ExprNode, KernelNode,
    RepresentationKind, SymbolRef,
};
use eqiora_sem::KernelProgram;

use crate::affine_fem::physical_gradient;
use crate::canonical::{boundary_parent, continuum_fields_on, lowering_error, relations_on};
use crate::discrete_space::{DiscreteSpace, HypercubeQ1Space};

const MAX_DAG_NODES: usize = 4_096;
const MAX_DERIVATIVE_ORDER: usize = 1;
const MAX_INTEGRAL_TERMS: usize = 8;
const MAX_QUADRATURE_POINTS: usize = 64;
const MAX_LOCAL_DOFS: usize = 32;
const MAX_TEMPORARIES: usize = 256;
const DERIVED_DERIVATIVE_ORDER: usize = 1;
const DERIVED_INTEGRAL_TERMS: usize = 2;
const Q1_TEMPORARIES: usize = 64;

const TEST_PAIRING: &str = "fem.derive.v1.test-pairing";
const DIVERGENCE_BY_PARTS: &str = "fem.derive.v1.divergence-by-parts";
const HOMOGENEOUS_ESSENTIAL_DISCHARGE: &str =
    "fem.derive.v1.boundary-discharge.essential-homogeneous";
const SOURCE_PAIRING: &str = "fem.derive.v1.source-pairing";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MatrixSlot {
    Test,
    Trial,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WeakTermSlot {
    TestPairing { test: MatrixSlot },
    Bilinear { test: MatrixSlot, trial: MatrixSlot },
    Boundary { test: MatrixSlot },
    Linear { test: MatrixSlot },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WeakSign {
    Positive,
    Negative,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CertificateEntry {
    rule_id: &'static str,
    relation: RawId,
    source_node: ExprId,
    slot: WeakTermSlot,
    sign: WeakSign,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DerivationCertificate {
    entries: Vec<CertificateEntry>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BoundaryRole {
    domain: RawId,
    relation: RawId,
    axis: usize,
    side: BoundarySide,
    trace_node: ExprId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DerivedScalarGalerkinForm {
    dimension: usize,
    domain: RawId,
    field: RawId,
    volume_relation: RawId,
    volume_nodes: VolumeNodes,
    boundary_roles: Vec<BoundaryRole>,
    parameters: Vec<RawId>,
    certificate: DerivationCertificate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct VolumeNodes {
    root: ExprId,
    divergence: ExprId,
    gradient: ExprId,
    source: ExprId,
}

#[derive(Debug, Clone, Copy)]
struct BoundaryNodes {
    trace: ExprId,
}

#[derive(Debug, Clone)]
struct QuadratureRecord {
    rule: QuadratureRule,
    declared_exactness: Option<usize>,
}

pub(crate) struct AdmittedScalarGalerkinForm<'a> {
    form: Option<&'a DerivedScalarGalerkinForm>,
    dimension: usize,
    quadrature: QuadratureRecord,
}

impl DerivedScalarGalerkinForm {
    pub(crate) fn admit_quadrature(
        &self,
        quadrature: &QuadratureRule,
    ) -> Result<AdmittedScalarGalerkinForm<'_>, Diagnostic> {
        let exactness = quadrature.polynomial_exactness().ok_or_else(|| {
            gate_error(
                self.volume_relation,
                "realization compatibility",
                "compiled Q1 source quadrature requires declared polynomial exactness",
            )
        })?;
        if quadrature.reference_cell().family() != ReferenceCellFamily::Hypercube
            || quadrature.reference_cell().dimension() != self.dimension
            || exactness < 3
            || quadrature.points().len() > MAX_QUADRATURE_POINTS
        {
            return Err(gate_error(
                self.volume_relation,
                "realization compatibility",
                format!(
                    "compiled Q1 requires a {}D hypercube rule with declared exactness >= 3 and at most {MAX_QUADRATURE_POINTS} points",
                    self.dimension
                ),
            ));
        }
        Ok(AdmittedScalarGalerkinForm {
            form: Some(self),
            dimension: self.dimension,
            quadrature: QuadratureRecord {
                rule: quadrature.clone(),
                declared_exactness: Some(exactness),
            },
        })
    }

    fn validate_certificate(&self) -> Result<(), Diagnostic> {
        let Some(test) = self.certificate.entries.first() else {
            return Err(certificate_error(
                self.volume_relation,
                "certificate is empty",
            ));
        };
        if test.rule_id != TEST_PAIRING
            || test.relation != self.volume_relation
            || test.source_node != self.volume_nodes.root
            || test.slot
                != (WeakTermSlot::TestPairing {
                    test: MatrixSlot::Test,
                })
            || test.sign != WeakSign::Positive
        {
            return Err(certificate_error(
                self.volume_relation,
                "test-pairing entry or slot is invalid",
            ));
        }
        let Some(divergence) = self.certificate.entries.get(1) else {
            return Err(certificate_error(
                self.volume_relation,
                "divergence-by-parts entry is absent",
            ));
        };
        if divergence.rule_id != DIVERGENCE_BY_PARTS
            || divergence.relation != self.volume_relation
            || divergence.source_node != self.volume_nodes.divergence
            || divergence.slot
                != (WeakTermSlot::Bilinear {
                    test: MatrixSlot::Test,
                    trial: MatrixSlot::Trial,
                })
            || divergence.sign != WeakSign::Positive
        {
            return Err(certificate_error(
                self.volume_relation,
                "divergence-by-parts sign or [test][trial] slot is invalid",
            ));
        }
        self.validate_boundary_certificate()?;
        let Some(source) = self.certificate.entries.last() else {
            return Err(certificate_error(
                self.volume_relation,
                "source-pairing entry is absent",
            ));
        };
        if source.rule_id != SOURCE_PAIRING
            || source.relation != self.volume_relation
            || source.source_node != self.volume_nodes.source
            || source.slot
                != (WeakTermSlot::Linear {
                    test: MatrixSlot::Test,
                })
            || source.sign != WeakSign::Positive
        {
            return Err(certificate_error(
                self.volume_relation,
                "source-pairing entry or slot is invalid",
            ));
        }
        Ok(())
    }

    fn validate_boundary_certificate(&self) -> Result<(), Diagnostic> {
        let boundary_count = 2 * self.dimension;
        if self.boundary_roles.len() != boundary_count
            || self.certificate.entries.len() != self.boundary_roles.len() + 3
        {
            return Err(certificate_error(
                self.volume_relation,
                format!(
                    "complete {}D essential-boundary discharge requires {boundary_count} entries",
                    self.dimension
                ),
            ));
        }
        for (entry, boundary) in self.certificate.entries[2..2 + boundary_count]
            .iter()
            .zip(&self.boundary_roles)
        {
            if entry.rule_id != HOMOGENEOUS_ESSENTIAL_DISCHARGE
                || entry.relation != boundary.relation
                || entry.source_node != boundary.trace_node
                || entry.slot
                    != (WeakTermSlot::Boundary {
                        test: MatrixSlot::Test,
                    })
                || entry.sign != WeakSign::Negative
            {
                return Err(certificate_error(
                    boundary.relation,
                    "essential-boundary discharge identity, slot, or sign is invalid",
                ));
            }
        }
        Ok(())
    }
}

pub(crate) fn compile_cartesian_q1_form(
    dimension: usize,
    quadrature: &QuadratureRule,
) -> Result<AdmittedScalarGalerkinForm<'static>, Diagnostic> {
    let space = HypercubeQ1Space::new(dimension)?;
    if quadrature.reference_cell().family() != ReferenceCellFamily::Hypercube
        || quadrature.reference_cell() != space.reference_cell()
    {
        return Err(Diagnostic::error(
            codes::INVALID_DISCRETIZATION,
            "compiled Cartesian Q1 form requires a dimension-matched hypercube quadrature rule",
        ));
    }
    Ok(AdmittedScalarGalerkinForm {
        form: None,
        dimension,
        quadrature: QuadratureRecord {
            rule: quadrature.clone(),
            declared_exactness: quadrature.polynomial_exactness(),
        },
    })
}

impl AdmittedScalarGalerkinForm<'_> {
    pub(crate) fn evaluate<S>(
        &self,
        geometry: &AffineGeometryMap,
        quadrature: &QuadratureRule,
        coefficient: f64,
        source: &S,
    ) -> Result<LocalContribution, Diagnostic>
    where
        S: Fn(&[f64]) -> f64 + ?Sized,
    {
        self.validate_realization(geometry, quadrature)?;
        let inverse = geometry.inverse_jacobian()?;
        let space = HypercubeQ1Space::new(self.dimension)?;
        let dof_count = space.local_dofs().len();
        let mut matrix = vec![0.0; dof_count * dof_count];
        let mut rhs = vec![0.0; dof_count];
        let mut physical = vec![0.0; self.dimension];
        for point in quadrature.points() {
            let basis = space.tabulate(&point.coordinates)?;
            geometry.map_point(&point.coordinates, &mut physical)?;
            let source_value = source(&physical);
            if !source_value.is_finite() {
                return Err(
                    self.realization_error("compiled Q1 source produced a non-finite value")
                );
            }
            let scale = point.weight * geometry.measure_scale();
            let gradients = (0..dof_count)
                .map(|dof| {
                    physical_gradient(
                        basis.gradient(dof).expect("Q1 tabulates every gradient"),
                        &inverse,
                        self.dimension,
                    )
                })
                .collect::<Vec<_>>();
            for test in 0..dof_count {
                rhs[test] += scale * source_value * basis.values()[test];
                for trial in 0..dof_count {
                    matrix[test * dof_count + trial] += scale
                        * coefficient
                        * gradients[test]
                            .iter()
                            .zip(&gradients[trial])
                            .map(|(left, right)| left * right)
                            .sum::<f64>();
                }
            }
        }
        LocalContribution::new(dof_count, dof_count, matrix, rhs)
    }

    fn validate_realization(
        &self,
        geometry: &AffineGeometryMap,
        quadrature: &QuadratureRule,
    ) -> Result<(), Diagnostic> {
        let space = HypercubeQ1Space::new(self.dimension)?;
        if space.reference_cell() != self.quadrature.rule.reference_cell()
            || geometry.reference_cell() != self.quadrature.rule.reference_cell()
            || geometry.physical_dimension() != self.dimension
            || quadrature != &self.quadrature.rule
            || quadrature.polynomial_exactness() != self.quadrature.declared_exactness
            || self.form.is_some() && space.local_dofs().len() > MAX_LOCAL_DOFS
        {
            return Err(self.realization_error(
                "compiled Q1 space, affine geometry, bit DOF order, or quadrature record drifted",
            ));
        }
        Ok(())
    }

    fn realization_error(&self, message: impl Into<String>) -> Diagnostic {
        let message = message.into();
        if let Some(form) = self.form {
            gate_error(form.volume_relation, "realization compatibility", message)
        } else {
            Diagnostic::error(codes::INVALID_DISCRETIZATION, message)
        }
    }
}

pub(crate) fn derive_candidate(
    program: &KernelProgram,
    domain: RawId,
) -> Result<Option<DerivedScalarGalerkinForm>, Diagnostic> {
    let Some(dimension) = cartesian_q1_dimension(program, domain) else {
        return Ok(None);
    };
    let fields = continuum_fields_on(program, domain);
    if fields.len() != 1 {
        return Err(role_error(
            domain,
            format!(
                "compiled Q1 requires exactly one scalar continuum unknown, found {}",
                fields.len()
            ),
        ));
    }
    let volume_relations = relations_on(program, domain);
    if volume_relations.len() != 1 {
        return Err(role_error(
            domain,
            format!(
                "compiled Q1 requires exactly one volume Relation, found {}",
                volume_relations.len()
            ),
        ));
    }
    let field = fields[0];
    let volume_relation = volume_relations[0];
    let boundaries = boundary_inventory(program, domain, field, dimension)?;
    let Some(boundary_roles) = boundaries else {
        return Ok(None);
    };
    let typed = typed_relation(program, volume_relation)?;
    validate_expression(&typed, volume_relation, field)?;
    let volume = recognize_volume(typed.expression(), volume_relation, field)?;
    validate_source_expression(typed.expression(), volume.source, volume_relation)?;
    validate_static_bounds(typed.expression(), volume_relation, dimension)?;
    let parameters =
        validate_closed_roles(program, domain, field, volume_relation, &boundary_roles)?;
    let certificate = build_certificate(volume_relation, volume, &boundary_roles, &parameters)?;
    let form = DerivedScalarGalerkinForm {
        dimension,
        domain,
        field,
        volume_relation,
        volume_nodes: volume,
        boundary_roles,
        parameters,
        certificate,
    };
    form.validate_certificate()?;
    Ok(Some(form))
}

pub(crate) fn typed_relation(
    program: &KernelProgram,
    relation: RawId,
) -> Result<TypedResidual<RawId>, Diagnostic> {
    let relation = relation.downcast::<kinds::Relation>().ok_or_else(|| {
        gate_error(
            relation,
            "static semantics",
            "selected owner is not a Relation",
        )
    })?;
    program
        .typed_relation_residual(relation)
        .map_err(|diagnostics| {
            diagnostics.into_iter().next().unwrap_or_else(|| {
                gate_error(
                    relation.erase(),
                    "static semantics",
                    "typed residual replay failed without a diagnostic",
                )
            })
        })
}

fn cartesian_q1_dimension(program: &KernelProgram, domain: RawId) -> Option<usize> {
    let Some(KernelNode::Domain(definition)) = program.node(domain) else {
        return None;
    };
    if !matches!(definition.kind(), DomainKind::CartesianBox { .. }) {
        return None;
    }
    let bounds = program.resolved_cartesian_bounds(definition.id()).ok()?;
    let dimensions = bounds.len();
    (1..=3).contains(&dimensions).then_some(dimensions)
}

fn boundary_inventory(
    program: &KernelProgram,
    parent: RawId,
    field: RawId,
    dimension: usize,
) -> Result<Option<Vec<BoundaryRole>>, Diagnostic> {
    let mut by_side = BTreeMap::new();
    for node in program.nodes() {
        let KernelNode::Domain(domain) = node else {
            continue;
        };
        let DomainKind::CartesianBoundary { axis, side } = domain.kind() else {
            continue;
        };
        if boundary_parent(program, domain.id().erase()) != Some(parent) {
            continue;
        }
        let relations = relations_on(program, domain.id().erase());
        if relations.len() != 1 {
            return Err(role_error(
                domain.id().erase(),
                format!(
                    "compiled Q1 boundary requires exactly one Relation, found {}",
                    relations.len()
                ),
            ));
        }
        let relation = relations[0];
        let typed = typed_relation(program, relation)?;
        let Some(nodes) = recognize_homogeneous_trace(typed.expression(), relation, field)? else {
            return Ok(None);
        };
        validate_expression(&typed, relation, field)?;
        let role = BoundaryRole {
            domain: domain.id().erase(),
            relation,
            axis: *axis,
            side: *side,
            trace_node: nodes.trace,
        };
        if by_side.insert((*axis, *side), role).is_some() {
            return Err(role_error(
                domain.id().erase(),
                "compiled Q1 boundary side is ambiguous",
            ));
        }
    }
    let expected = (0..dimension)
        .flat_map(|axis| [(axis, BoundarySide::Lower), (axis, BoundarySide::Upper)])
        .collect::<Vec<_>>();
    if by_side.len() != expected.len() || expected.iter().any(|side| !by_side.contains_key(side)) {
        return Err(role_error(
            parent,
            format!(
                "compiled Q1 requires one homogeneous essential Relation on every {dimension}D box side"
            ),
        ));
    }
    Ok(Some(expected.iter().map(|side| by_side[side]).collect()))
}

fn validate_expression(
    typed: &TypedResidual<RawId>,
    owner: RawId,
    field: RawId,
) -> Result<(), Diagnostic> {
    let expression = typed.expression();
    if expression.nodes().len() > MAX_DAG_NODES {
        return Err(bounds_error(
            owner,
            format!(
                "expression has {} nodes, exceeding {MAX_DAG_NODES}",
                expression.nodes().len()
            ),
        ));
    }
    for node in expression.nodes() {
        let admitted = match node {
            ExprNode::Constant(_)
            | ExprNode::Neg(_)
            | ExprNode::Add(_, _)
            | ExprNode::Sub(_, _)
            | ExprNode::Mul(_, _)
            | ExprNode::PowI(_, _)
            | ExprNode::SpatialCoordinate(_)
            | ExprNode::UnaryMath(eqiora_schema::kernel::UnaryMathFunction::Sin, _)
            | ExprNode::Gradient(_)
            | ExprNode::Divergence(_)
            | ExprNode::Trace(_) => true,
            ExprNode::Symbol(SymbolRef::Field(id)) => id.erase() == field,
            ExprNode::Symbol(SymbolRef::Parameter(_)) => true,
            _ => false,
        };
        if !admitted {
            return Err(gate_error(
                owner,
                "role assignment",
                "residual contains a symbol or expression kind outside the closed compiler subset",
            ));
        }
    }
    require_closed_dag(expression, owner)
}

fn require_closed_dag(expression: &ExprDag, owner: RawId) -> Result<(), Diagnostic> {
    if expression.roots().len() != 1 {
        return Err(certificate_error(owner, "compiled Q1 requires one root"));
    }
    let mut reached = vec![false; expression.nodes().len()];
    let mut pending = vec![expression.roots()[0]];
    while let Some(value) = pending.pop() {
        let index = usize::try_from(value.index())
            .expect("ExprDag identities are portable platform indices");
        if reached[index] {
            continue;
        }
        reached[index] = true;
        push_operands(
            expression
                .node(value)
                .expect("ExprDag owns every referenced node"),
            &mut pending,
        );
    }
    if reached.contains(&false) {
        return Err(certificate_error(
            owner,
            "expression contains an unconsumed strong-form node",
        ));
    }
    Ok(())
}

fn push_operands(node: &ExprNode, pending: &mut Vec<ExprId>) {
    match node {
        ExprNode::Neg(value)
        | ExprNode::PowI(value, _)
        | ExprNode::UnaryMath(_, value)
        | ExprNode::Gradient(value)
        | ExprNode::Divergence(value)
        | ExprNode::Trace(value) => pending.push(*value),
        ExprNode::Add(left, right) | ExprNode::Sub(left, right) | ExprNode::Mul(left, right) => {
            pending.push(*right);
            pending.push(*left);
        }
        _ => {}
    }
}

fn recognize_volume(
    expression: &ExprDag,
    owner: RawId,
    field: RawId,
) -> Result<VolumeNodes, Diagnostic> {
    let root = expression.roots()[0];
    let Some(ExprNode::Sub(operator, source)) = expression.node(root) else {
        return Err(certificate_error(
            owner,
            "volume residual must be exactly `-div(k grad(u)) - source`",
        ));
    };
    let Some(ExprNode::Neg(divergence)) = expression.node(*operator) else {
        return Err(certificate_error(
            owner,
            "volume residual must begin with negative divergence",
        ));
    };
    let Some(ExprNode::Divergence(flux)) = expression.node(*divergence) else {
        return Err(certificate_error(
            owner,
            "negative operator must consume one divergence node",
        ));
    };
    let gradients = gradient_nodes(expression, *flux, field);
    if gradients.len() != 1 {
        return Err(certificate_error(
            owner,
            "constitutive flux must contain exactly one gradient of the unknown",
        ));
    }
    Ok(VolumeNodes {
        root,
        divergence: *divergence,
        gradient: gradients[0],
        source: *source,
    })
}

fn gradient_nodes(expression: &ExprDag, value: ExprId, field: RawId) -> Vec<ExprId> {
    match expression.node(value) {
        Some(ExprNode::Gradient(argument))
            if matches!(
                expression.node(*argument),
                Some(ExprNode::Symbol(SymbolRef::Field(id))) if id.erase() == field
            ) =>
        {
            vec![value]
        }
        Some(ExprNode::Mul(left, right)) => {
            let mut nodes = gradient_nodes(expression, *left, field);
            nodes.extend(gradient_nodes(expression, *right, field));
            nodes
        }
        _ => Vec::new(),
    }
}

fn validate_source_expression(
    expression: &ExprDag,
    source: ExprId,
    owner: RawId,
) -> Result<(), Diagnostic> {
    let mut pending = vec![source];
    let mut reached = vec![false; expression.nodes().len()];
    while let Some(value) = pending.pop() {
        let index = usize::try_from(value.index()).expect("ExprDag indices fit usize");
        if reached[index] {
            continue;
        }
        reached[index] = true;
        let node = expression.node(value).expect("ExprDag owns every operand");
        if !matches!(
            node,
            ExprNode::Constant(_)
                | ExprNode::Symbol(SymbolRef::Parameter(_))
                | ExprNode::Neg(_)
                | ExprNode::Add(_, _)
                | ExprNode::Sub(_, _)
                | ExprNode::Mul(_, _)
                | ExprNode::PowI(_, _)
                | ExprNode::SpatialCoordinate(_)
                | ExprNode::UnaryMath(eqiora_schema::kernel::UnaryMathFunction::Sin, _)
        ) {
            return Err(certificate_error(
                owner,
                "source term is not an unknown-independent scalar spatial expression",
            ));
        }
        push_operands(node, &mut pending);
    }
    Ok(())
}

fn recognize_homogeneous_trace(
    expression: &ExprDag,
    owner: RawId,
    field: RawId,
) -> Result<Option<BoundaryNodes>, Diagnostic> {
    if expression.roots().len() != 1 {
        return Err(certificate_error(
            owner,
            "boundary Relation requires one residual root",
        ));
    }
    let root = expression.roots()[0];
    let (trace, value) = match expression.node(root) {
        Some(ExprNode::Trace(argument)) => (*argument, None),
        Some(ExprNode::Sub(trace, value)) => {
            let Some(ExprNode::Trace(argument)) = expression.node(*trace) else {
                return Ok(None);
            };
            (*argument, Some(*value))
        }
        _ => return Ok(None),
    };
    if !matches!(
        expression.node(trace),
        Some(ExprNode::Symbol(SymbolRef::Field(id))) if id.erase() == field
    ) || value.is_some_and(|value| !is_literal_zero(expression, value))
    {
        return Ok(None);
    }
    let trace_node = match expression.node(root) {
        Some(ExprNode::Trace(_)) => root,
        Some(ExprNode::Sub(trace, _)) => *trace,
        _ => unreachable!("boundary shape was matched above"),
    };
    Ok(Some(BoundaryNodes { trace: trace_node }))
}

fn is_literal_zero(expression: &ExprDag, value: ExprId) -> bool {
    matches!(
        expression.node(value),
        Some(ExprNode::Constant(quantity)) if quantity.value().to_bits() == 0.0_f64.to_bits()
    )
}

fn validate_static_bounds(
    expression: &ExprDag,
    owner: RawId,
    dimension: usize,
) -> Result<(), Diagnostic> {
    if DERIVED_DERIVATIVE_ORDER > MAX_DERIVATIVE_ORDER
        || DERIVED_INTEGRAL_TERMS > MAX_INTEGRAL_TERMS
        || Q1_TEMPORARIES > MAX_TEMPORARIES
        || expression.nodes().len() > MAX_DAG_NODES
    {
        return Err(bounds_error(
            owner,
            "derived Q1 work exceeds one or more frozen compiler bounds",
        ));
    }
    let dofs = HypercubeQ1Space::new(dimension)?.local_dofs().len();
    if dofs > MAX_LOCAL_DOFS {
        return Err(bounds_error(
            owner,
            format!("Q1 local DOFs exceed {MAX_LOCAL_DOFS}"),
        ));
    }
    Ok(())
}

fn validate_closed_roles(
    program: &KernelProgram,
    domain: RawId,
    field: RawId,
    volume_relation: RawId,
    boundaries: &[BoundaryRole],
) -> Result<Vec<RawId>, Diagnostic> {
    let relations = std::iter::once(volume_relation)
        .chain(boundaries.iter().map(|role| role.relation))
        .collect::<BTreeSet<_>>();
    let scoped_fields = program
        .nodes()
        .filter_map(|node| match node {
            KernelNode::Field(value)
                if program.edges().iter().any(|edge| {
                    edge.from() == value.id().erase()
                        && edge.to() == domain
                        && edge.kind() == EdgeKind::DefinedOn
                }) =>
            {
                Some(value.id().erase())
            }
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    if scoped_fields != BTreeSet::from([field]) {
        return Err(role_error(
            domain,
            "compiled Q1 scope must contain exactly the admitted scalar unknown",
        ));
    }
    let representations = representations_of(program, field);
    if representations.len() != 1
        || !matches!(
            program.node(*representations.first().expect("length checked above")),
            Some(KernelNode::Representation(value))
                if value.kind() == RepresentationKind::Continuum
        )
    {
        return Err(role_error(
            field,
            "compiled Q1 unknown requires exactly one continuum Representation",
        ));
    }
    let boundary_domains = program
        .edges()
        .iter()
        .filter(|edge| edge.kind() == EdgeKind::BoundaryOf && edge.to() == domain)
        .map(|edge| edge.from())
        .collect::<BTreeSet<_>>();
    let admitted_boundary_domains = boundaries
        .iter()
        .map(|boundary| boundary.domain)
        .collect::<BTreeSet<_>>();
    if boundary_domains != admitted_boundary_domains {
        return Err(role_error(
            domain,
            "compiled Q1 boundary inventory leaves a reachable Domain unconsumed",
        ));
    }
    let activations = continuous_activations(program, &relations)?;
    let parameters = relation_parameters(program, &relations)?;
    for relation in &relations {
        validate_relation_dependencies(program, *relation)?;
    }
    debug_assert_eq!(activations.len(), relations.len());
    Ok(parameters.into_iter().collect())
}

fn representations_of(program: &KernelProgram, field: RawId) -> BTreeSet<RawId> {
    program
        .edges()
        .iter()
        .filter(|edge| edge.from() == field && edge.kind() == EdgeKind::DefinedOn)
        .filter_map(|edge| match program.node(edge.to()) {
            Some(KernelNode::Representation(_)) => Some(edge.to()),
            _ => None,
        })
        .collect()
}

fn validate_relation_dependencies(
    program: &KernelProgram,
    relation: RawId,
) -> Result<(), Diagnostic> {
    let typed = typed_relation(program, relation)?;
    let expression_symbols = typed
        .expression()
        .nodes()
        .iter()
        .filter_map(|node| match node {
            ExprNode::Symbol(SymbolRef::Field(value)) => Some(value.erase()),
            ExprNode::Symbol(SymbolRef::Parameter(value)) => Some(value.erase()),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    let graph_dependencies = program
        .edges()
        .iter()
        .filter(|edge| edge.from() == relation && edge.kind() == EdgeKind::DependsOn)
        .map(|edge| edge.to())
        .collect::<BTreeSet<_>>();
    if expression_symbols != graph_dependencies {
        return Err(role_error(
            relation,
            "Relation dependency inventory does not exactly match its admitted symbols",
        ));
    }
    Ok(())
}

fn continuous_activations(
    program: &KernelProgram,
    relations: &BTreeSet<RawId>,
) -> Result<BTreeSet<RawId>, Diagnostic> {
    let mut activations = BTreeSet::new();
    for relation in relations {
        let selected = program
            .edges()
            .iter()
            .filter(|edge| edge.kind() == EdgeKind::Activates && edge.to() == *relation)
            .filter_map(|edge| match program.node(edge.from()) {
                Some(KernelNode::Activation(value)) => Some(value),
                _ => None,
            })
            .collect::<Vec<_>>();
        if selected.len() != 1
            || !matches!(selected[0].kind(), ActivationKind::Continuous)
            || !activations.insert(selected[0].id().erase())
        {
            return Err(role_error(
                *relation,
                "compiled Q1 requires one distinct continuous Activation per Relation",
            ));
        }
    }
    Ok(activations)
}

fn relation_parameters(
    program: &KernelProgram,
    relations: &BTreeSet<RawId>,
) -> Result<BTreeSet<RawId>, Diagnostic> {
    let mut parameters = BTreeSet::new();
    for relation in relations {
        let typed = typed_relation(program, *relation)?;
        parameters.extend(typed.expression().nodes().iter().filter_map(|node| {
            if let ExprNode::Symbol(SymbolRef::Parameter(parameter)) = node {
                Some(parameter.erase())
            } else {
                None
            }
        }));
    }
    Ok(parameters)
}

fn build_certificate(
    volume_relation: RawId,
    volume: VolumeNodes,
    boundaries: &[BoundaryRole],
    _parameters: &[RawId],
) -> Result<DerivationCertificate, Diagnostic> {
    let mut entries = Vec::with_capacity(boundaries.len() + 3);
    entries.push(CertificateEntry {
        rule_id: TEST_PAIRING,
        relation: volume_relation,
        source_node: volume.root,
        slot: WeakTermSlot::TestPairing {
            test: MatrixSlot::Test,
        },
        sign: WeakSign::Positive,
    });
    entries.push(CertificateEntry {
        rule_id: DIVERGENCE_BY_PARTS,
        relation: volume_relation,
        source_node: volume.divergence,
        slot: WeakTermSlot::Bilinear {
            test: MatrixSlot::Test,
            trial: MatrixSlot::Trial,
        },
        sign: WeakSign::Positive,
    });
    entries.extend(boundaries.iter().map(|boundary| CertificateEntry {
        rule_id: HOMOGENEOUS_ESSENTIAL_DISCHARGE,
        relation: boundary.relation,
        source_node: boundary.trace_node,
        slot: WeakTermSlot::Boundary {
            test: MatrixSlot::Test,
        },
        sign: WeakSign::Negative,
    }));
    entries.push(CertificateEntry {
        rule_id: SOURCE_PAIRING,
        relation: volume_relation,
        source_node: volume.source,
        slot: WeakTermSlot::Linear {
            test: MatrixSlot::Test,
        },
        sign: WeakSign::Positive,
    });
    let certificate = DerivationCertificate { entries };
    if volume.gradient == volume.source
        || boundaries
            .iter()
            .any(|role| role.relation == volume_relation)
    {
        return Err(certificate_error(
            volume_relation,
            "certificate node or relation identities overlap across weak terms",
        ));
    }
    Ok(certificate)
}

fn gate_error(owner: RawId, gate: &str, message: impl Into<String>) -> Diagnostic {
    lowering_error(
        owner,
        format!("FEM form compiler {gate} gate: {}", message.into()),
    )
}

fn role_error(owner: RawId, message: impl Into<String>) -> Diagnostic {
    gate_error(owner, "role assignment", message)
}

fn certificate_error(owner: RawId, message: impl Into<String>) -> Diagnostic {
    gate_error(owner, "derivation certificate", message)
}

fn bounds_error(owner: RawId, message: impl Into<String>) -> Diagnostic {
    gate_error(owner, "bounded compilation", message)
}

#[cfg(test)]
mod tests;
