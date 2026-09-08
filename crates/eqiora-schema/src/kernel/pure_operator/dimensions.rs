//! Dimension proofs modulo explicit scalar/tensor formal constraints.
use super::*;

pub(super) fn derive_symbolic_dimension(
    formals: &[PureValueClass],
    nodes: &[CalculusNode],
    root: CalculusNodeId,
) -> Result<FormalDimensionMonomial, PureOperatorError> {
    let formal_count = formals.len();
    let mut dimensions: Vec<Box<[u16]>> = Vec::with_capacity(nodes.len());
    for node in nodes {
        let dimension = match node {
            CalculusNode::Rational(_) | CalculusNode::KroneckerDelta(_, _) => {
                vec![0; formal_count].into_boxed_slice()
            }
            CalculusNode::FormalComponent { formal, .. } => {
                let formal_index = usize::from(*formal);
                if formal_index >= formal_count {
                    return Err(PureOperatorError::InvalidFormal(*formal));
                }
                let mut exponents = vec![0; formal_count];
                exponents[formal_index] = 1;
                exponents.into_boxed_slice()
            }
            CalculusNode::Neg(value) => {
                dimensions[definition_index(*value, dimensions.len())?].clone()
            }
            CalculusNode::Add(left, right) => {
                let left = dimensions[definition_index(*left, dimensions.len())?].clone();
                let right = dimensions[definition_index(*right, dimensions.len())?].clone();
                if normalized(formals, &left)? != normalized(formals, &right)? {
                    return Err(PureOperatorError::AdditiveDimensionMismatch);
                }
                left
            }
            CalculusNode::Mul(left, right) => {
                let left = &dimensions[definition_index(*left, dimensions.len())?];
                let right = &dimensions[definition_index(*right, dimensions.len())?];
                let mut exponents = Vec::with_capacity(formal_count);
                for (left, right) in left.iter().zip(right) {
                    let exponent = left
                        .checked_add(*right)
                        .filter(|exponent| *exponent <= MAX_FORMAL_EXPONENT)
                        .ok_or(PureOperatorError::FormalExponentLimit)?;
                    exponents.push(exponent);
                }
                exponents.into_boxed_slice()
            }
        };
        dimensions.push(dimension);
    }
    Ok(FormalDimensionMonomial {
        exponents: dimensions[definition_index(root, dimensions.len())?].clone(),
    })
}

pub(super) fn instantiate_dimension<I>(
    monomial: &FormalDimensionMonomial,
    arguments: &[ExpressionType<I>],
) -> Result<eqiora_core::DimExponents, PureOperatorError> {
    let mut result = eqiora_core::DimExponents::DIMENSIONLESS;
    for (argument, exponent) in arguments.iter().zip(monomial.exponents()) {
        let term = argument
            .dimension()
            .pow(i32::from(*exponent), 1)
            .ok_or(PureOperatorError::ResultDimensionOverflow)?;
        result = result
            .mul(term)
            .ok_or(PureOperatorError::ResultDimensionOverflow)?;
    }
    Ok(result)
}

fn normalized(
    formals: &[PureValueClass],
    exponents: &[u16],
) -> Result<(DimExponents, Vec<u16>), PureOperatorError> {
    let mut fixed = DimExponents::DIMENSIONLESS;
    let mut free = Vec::with_capacity(formals.len());
    for (formal, exponent) in formals.iter().zip(exponents) {
        if let Some(dimension) = formal.dimension() {
            let term = dimension
                .pow(i32::from(*exponent), 1)
                .ok_or(PureOperatorError::ResultDimensionOverflow)?;
            fixed = fixed
                .mul(term)
                .ok_or(PureOperatorError::ResultDimensionOverflow)?;
            free.push(0);
        } else {
            free.push(*exponent);
        }
    }
    Ok((fixed, free))
}

pub(super) fn validate_result_dimension(
    formals: &[PureValueClass],
    result: PureValueClass,
    monomial: &FormalDimensionMonomial,
) -> Result<(), PureOperatorError> {
    if let Some(expected) = result.dimension() {
        let (fixed, free) = normalized(formals, monomial.exponents())?;
        if fixed != expected || free.iter().any(|power| *power != 0) {
            return Err(PureOperatorError::AdditiveDimensionMismatch);
        }
    }
    Ok(())
}
