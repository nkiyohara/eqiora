//! Bounded scalar composition preserves authored exact arithmetic order.
use super::*;

impl CalculusBuilder {
    /// Substitute checked scalar argument expressions into one closed scalar definition.
    /// The caller resolves lexical names and rejects recursive definition graphs.
    /// No polynomial normalization or executable reassociation is performed.
    pub fn apply_scalar(
        &mut self,
        definition: &PureOperatorDefinition,
        arguments: &[CalculusNodeId],
    ) -> Result<CalculusNodeId, PureOperatorError> {
        if arguments.len() != definition.formals.len() {
            return Err(PureOperatorError::ArityMismatch);
        }
        if !definition.result.is_invariant_scalar()
            || definition
                .formals
                .iter()
                .any(|formal| !formal.is_invariant_scalar())
        {
            return Err(PureOperatorError::FormalTypeMismatch);
        }
        let additional = definition
            .nodes
            .iter()
            .filter(|node| !matches!(node, CalculusNode::FormalComponent { .. }))
            .count();
        self.nodes
            .len()
            .checked_add(additional)
            .filter(|count| *count <= MAX_NODES)
            .ok_or(PureOperatorError::NodeLimit)?;
        for (argument, formal) in arguments.iter().zip(&definition.formals) {
            definition_index(*argument, self.nodes.len())?;
            let dimension = derive_symbolic_dimension(&self.formals, &self.nodes, *argument)?;
            validate_result_dimension(&self.formals, *formal, &dimension)?;
            if let Some(expected) = formal.scalar_domain()
                && domains::expression_domain(&self.formals, &self.nodes, *argument)?
                    != Some(expected)
            {
                return Err(PureOperatorError::FormalTypeMismatch);
            }
        }
        // Stage the bounded append so any failed depth/type check leaves the caller intact.
        let mut staged = Self {
            formals: self.formals.clone(),
            result: self.result,
            nodes: self.nodes.clone(),
            depths: self.depths.clone(),
        };
        let mut ids = Vec::with_capacity(definition.nodes.len());
        for node in &definition.nodes {
            let mapped = |id| {
                ids.get(definition_index(id, ids.len())?)
                    .copied()
                    .ok_or(PureOperatorError::InvalidNode)
            };
            let id = match node {
                CalculusNode::FormalComponent { formal, axes } if axes.is_empty() => {
                    arguments[usize::from(*formal)]
                }
                CalculusNode::Rational { value, dimension } => {
                    staged.push(CalculusNode::Rational {
                        value: *value,
                        dimension: *dimension,
                    })?
                }
                CalculusNode::Boolean(value) => staged.push(CalculusNode::Boolean(*value))?,
                CalculusNode::Compare(op, left, right) => {
                    staged.push(CalculusNode::Compare(*op, mapped(*left)?, mapped(*right)?))?
                }
                CalculusNode::Not(value) => staged.push(CalculusNode::Not(mapped(*value)?))?,
                CalculusNode::And(left, right) => {
                    staged.push(CalculusNode::And(mapped(*left)?, mapped(*right)?))?
                }
                CalculusNode::Or(left, right) => {
                    staged.push(CalculusNode::Or(mapped(*left)?, mapped(*right)?))?
                }
                CalculusNode::UnaryMath(function, value) => {
                    staged.push(CalculusNode::UnaryMath(*function, mapped(*value)?))?
                }
                CalculusNode::Select {
                    condition,
                    then_value,
                    else_value,
                } => staged.push(CalculusNode::Select {
                    condition: mapped(*condition)?,
                    then_value: mapped(*then_value)?,
                    else_value: mapped(*else_value)?,
                })?,
                CalculusNode::Require { condition, value } => {
                    staged.push(CalculusNode::Require {
                        condition: mapped(*condition)?,
                        value: mapped(*value)?,
                    })?
                }
                CalculusNode::Neg(value) => staged.push(CalculusNode::Neg(mapped(*value)?))?,
                CalculusNode::Add(left, right) => {
                    staged.push(CalculusNode::Add(mapped(*left)?, mapped(*right)?))?
                }
                CalculusNode::Mul(left, right) => {
                    staged.push(CalculusNode::Mul(mapped(*left)?, mapped(*right)?))?
                }
                _ => return Err(PureOperatorError::FormalTypeMismatch),
            };
            ids.push(id);
        }
        let root = ids[definition_index(definition.root, ids.len())?];
        dimensions::validate_profile(&staged.formals, staged.result, &staged.nodes)?;
        derive_symbolic_dimension(&staged.formals, &staged.nodes, root)?;
        *self = staged;
        Ok(root)
    }
}
