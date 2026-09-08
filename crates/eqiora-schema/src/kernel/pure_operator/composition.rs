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
                CalculusNode::Rational(value) => staged.push(CalculusNode::Rational(*value))?,
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
        derive_symbolic_dimension(&staged.formals, &staged.nodes, root)?;
        *self = staged;
        Ok(root)
    }
}
