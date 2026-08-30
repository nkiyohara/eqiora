use super::*;

pub(crate) fn invalid_realization(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_REALIZATION, message)
}

pub(crate) fn invalid_parameter_binding(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_LINEARIZATION, message)
}

pub(crate) fn unique_continuum_field(
    program: &KernelProgram,
    domain: RawId,
) -> Result<RawId, Diagnostic> {
    let fields = continuum_fields_on(program, domain);
    if fields.len() == 1 {
        Ok(fields[0])
    } else {
        Err(lowering_error(
            domain,
            format!(
                "default scalar elliptic lowering requires one continuum Field, found {}",
                fields.len()
            ),
        ))
    }
}

pub(crate) fn continuum_fields_on(program: &KernelProgram, domain: RawId) -> Vec<RawId> {
    program
        .nodes()
        .filter_map(|node| match node {
            KernelNode::Field(field)
                if has_edge(program, field.id().erase(), domain, EdgeKind::DefinedOn)
                    && field_has_continuum_representation(program, field.id().erase()) =>
            {
                Some(field.id().erase())
            }
            _ => None,
        })
        .collect()
}

pub(crate) fn field_has_continuum_representation(program: &KernelProgram, field: RawId) -> bool {
    program.edges().iter().any(|edge| {
        edge.from() == field
            && edge.kind() == EdgeKind::DefinedOn
            && matches!(
                program.node(edge.to()),
                Some(KernelNode::Representation(representation))
                    if representation.kind() == RepresentationKind::Continuum
            )
    })
}

pub(crate) fn relations_on(program: &KernelProgram, domain: RawId) -> Vec<RawId> {
    program
        .edges()
        .iter()
        .filter(|edge| edge.kind() == EdgeKind::AppliesOn && edge.to() == domain)
        .map(|edge| edge.from())
        .collect()
}

pub(crate) fn unique_relation_on(
    program: &KernelProgram,
    domain: RawId,
) -> Result<RawId, Diagnostic> {
    let relations = relations_on(program, domain);
    if relations.len() == 1 {
        Ok(relations[0])
    } else {
        Err(lowering_error(
            domain,
            format!(
                "default scalar elliptic lowering requires one Relation on this Domain, found {}",
                relations.len()
            ),
        ))
    }
}

pub(crate) fn lower_volume_relation(
    program: &KernelProgram,
    relation: RawId,
    field: RawId,
    coordinate_dimension: usize,
) -> Result<(ScalarSpatialExpression, ScalarSpatialExpression), Diagnostic> {
    let expression = relation_expression(program, relation)?;
    let root = unique_root(expression, relation)?;
    let (operator, source) = match expression.node(root) {
        Some(ExprNode::Sub(operator, source)) => (*operator, Some(*source)),
        _ => (root, None),
    };
    let divergence = match expression.node(operator) {
        Some(ExprNode::Neg(value)) => *value,
        _ => {
            return Err(lowering_error(
                relation,
                "volume residual must start with `-div(...)`",
            ));
        }
    };
    let flux = match expression.node(divergence) {
        Some(ExprNode::Divergence(value)) => *value,
        _ => {
            return Err(lowering_error(
                relation,
                "volume residual must contain physical divergence",
            ));
        }
    };
    let coefficient = lower_flux_coefficient(
        program,
        expression,
        flux,
        field,
        relation,
        coordinate_dimension,
    )?;
    let source = match source {
        Some(source) => {
            spatial_expression::lower(program, expression, source, relation, coordinate_dimension)?
        }
        None => ScalarSpatialExpression::constant(coordinate_dimension, 0.0),
    };
    Ok((coefficient, source))
}

pub(crate) fn lower_cartesian_boundary_relation(
    program: &KernelProgram,
    relation: RawId,
    field: RawId,
    volume_coefficient: &ScalarSpatialExpression,
    coordinate_dimension: usize,
) -> Result<ScalarEllipticCartesianBoundary, Diagnostic> {
    let expression = relation_expression(program, relation)?;
    let root = unique_root(expression, relation)?;
    let (operator, value) = match expression.node(root) {
        Some(ExprNode::Sub(operator, value)) => (*operator, Some(*value)),
        _ => (root, None),
    };
    let value = match value {
        Some(value) => {
            spatial_expression::lower(program, expression, value, relation, coordinate_dimension)?
        }
        None => ScalarSpatialExpression::constant(coordinate_dimension, 0.0),
    };
    match expression.node(operator) {
        Some(ExprNode::Trace(trace_operand)) if is_field(expression, *trace_operand, field) => {
            Ok(ScalarEllipticCartesianBoundary::Essential(value))
        }
        Some(ExprNode::NormalComponent(flux)) => {
            let coefficient = lower_flux_coefficient(
                program,
                expression,
                *flux,
                field,
                relation,
                coordinate_dimension,
            )?;
            if coefficient != *volume_coefficient {
                return Err(lowering_error(
                    relation,
                    "boundary flux coefficient is not structurally identical to the volume constitutive flux",
                ));
            }
            Ok(ScalarEllipticCartesianBoundary::Natural(value))
        }
        _ => Err(lowering_error(
            relation,
            "boundary residual must be `trace(field) - value` or `normal(flux) - value`",
        )),
    }
}

pub(crate) fn lower_constant_boundary_1d(
    boundary: &ScalarEllipticCartesianBoundary,
    owner: RawId,
) -> Result<ScalarBoundaryCondition1d, Diagnostic> {
    let value = boundary.value().constant_value().ok_or_else(|| {
        lowering_error(
            owner,
            "scalar elliptic 1D realization currently requires spatially constant boundary data",
        )
    })?;
    Ok(match boundary {
        ScalarEllipticCartesianBoundary::Essential(_) => {
            ScalarBoundaryCondition1d::Essential(value)
        }
        ScalarEllipticCartesianBoundary::Natural(_) => ScalarBoundaryCondition1d::Natural(value),
    })
}

pub(crate) fn collect_parameter_coordinates(
    coefficient: &ScalarSpatialExpression,
    source: &ScalarSpatialExpression,
    boundaries: &BTreeMap<(usize, BoundarySide), ScalarEllipticCartesianBoundary>,
) -> (Vec<Id<kinds::Parameter>>, Vec<f64>) {
    let mut fields = Vec::new();
    let mut values = Vec::new();
    for expression in std::iter::once(coefficient)
        .chain(std::iter::once(source))
        .chain(
            boundaries
                .values()
                .map(ScalarEllipticCartesianBoundary::value),
        )
    {
        for (field, value) in expression
            .parameter_fields()
            .iter()
            .zip(expression.parameter_values())
        {
            if let Some(index) = fields.iter().position(|existing| existing == field) {
                debug_assert_eq!(values[index], *value);
            } else {
                fields.push(*field);
                values.push(*value);
            }
        }
    }
    (fields, values)
}

pub(crate) fn lower_flux_coefficient(
    program: &KernelProgram,
    expression: &ExprDag,
    value: ExprId,
    field: RawId,
    owner: RawId,
    coordinate_dimension: usize,
) -> Result<ScalarSpatialExpression, Diagnostic> {
    if let Some(ExprNode::Gradient(argument)) = expression.node(value)
        && is_field(expression, *argument, field)
    {
        return Ok(ScalarSpatialExpression::constant(coordinate_dimension, 1.0));
    }
    let Some(ExprNode::Mul(left, right)) = expression.node(value) else {
        return Err(lowering_error(
            owner,
            "constitutive flux must be a scalar coefficient times `grad(field)`",
        ));
    };
    if contains_gradient_of(expression, *left, field) {
        let coefficient = lower_flux_coefficient(
            program,
            expression,
            *left,
            field,
            owner,
            coordinate_dimension,
        )?;
        let factor =
            lower_spatial_factor(program, expression, *right, owner, coordinate_dimension)?;
        Ok(coefficient.multiply(factor))
    } else if contains_gradient_of(expression, *right, field) {
        let factor = lower_spatial_factor(program, expression, *left, owner, coordinate_dimension)?;
        let coefficient = lower_flux_coefficient(
            program,
            expression,
            *right,
            field,
            owner,
            coordinate_dimension,
        )?;
        Ok(factor.multiply(coefficient))
    } else {
        Err(lowering_error(
            owner,
            "constitutive flux does not contain `grad(field)`",
        ))
    }
}

pub(crate) fn lower_spatial_factor(
    program: &KernelProgram,
    expression: &ExprDag,
    value: ExprId,
    owner: RawId,
    coordinate_dimension: usize,
) -> Result<ScalarSpatialExpression, Diagnostic> {
    spatial_expression::lower(program, expression, value, owner, coordinate_dimension)
}

pub(crate) fn contains_gradient_of(expression: &ExprDag, value: ExprId, field: RawId) -> bool {
    match expression.node(value) {
        Some(ExprNode::Gradient(argument)) => is_field(expression, *argument, field),
        Some(ExprNode::Mul(left, right)) => {
            contains_gradient_of(expression, *left, field)
                || contains_gradient_of(expression, *right, field)
        }
        _ => false,
    }
}

pub(crate) fn is_field(expression: &ExprDag, value: ExprId, field: RawId) -> bool {
    matches!(
        expression.node(value),
        Some(ExprNode::Symbol(SymbolRef::Field(id))) if id.erase() == field
    )
}

pub(crate) fn relation_expression(
    program: &KernelProgram,
    relation: RawId,
) -> Result<&ExprDag, Diagnostic> {
    match program.node(relation) {
        Some(KernelNode::Relation(relation)) => Ok(relation.residuals()),
        _ => Err(lowering_error(
            relation,
            "AppliesOn source has no Relation definition",
        )),
    }
}

pub(crate) fn unique_root(expression: &ExprDag, owner: RawId) -> Result<ExprId, Diagnostic> {
    if expression.roots().len() == 1 {
        Ok(expression.roots()[0])
    } else {
        Err(lowering_error(
            owner,
            "default scalar elliptic Relation requires exactly one residual root",
        ))
    }
}

pub(crate) fn boundary_parent(program: &KernelProgram, boundary: RawId) -> Option<RawId> {
    let parents = program
        .edges()
        .iter()
        .filter(|edge| edge.from() == boundary && edge.kind() == EdgeKind::BoundaryOf)
        .map(|edge| edge.to())
        .collect::<Vec<_>>();
    (parents.len() == 1).then(|| parents[0])
}

pub(crate) fn has_edge(program: &KernelProgram, from: RawId, to: RawId, kind: EdgeKind) -> bool {
    program
        .edges()
        .iter()
        .any(|edge| edge.from() == from && edge.to() == to && edge.kind() == kind)
}

pub(crate) fn lowering_error(owner: RawId, message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_SPATIAL_LOWERING, message).with_graph_path(GraphPath::new([
        owner.kind().graph().name().to_owned(),
        format!("{:?}", owner.kind()),
        owner.to_string(),
    ]))
}

pub(crate) fn model_lowering_error(
    program: &KernelProgram,
    message: impl Into<String>,
) -> Diagnostic {
    Diagnostic::error(codes::INVALID_SPATIAL_LOWERING, message).with_graph_path(GraphPath::new([
        "ontology-view".to_owned(),
        "eqiora.model/v1".to_owned(),
        program.model().to_string(),
    ]))
}
