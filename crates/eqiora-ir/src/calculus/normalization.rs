use std::collections::BTreeMap;

use eqiora_schema::kernel::typing::ExpressionType;

use super::ExactRational;
use super::expansion::{ScalarCalculus, ScalarCalculusAtom, ScalarCalculusNode};
use super::{CalculusError, calculus_index, hash, push_rational, push_u16, push_u32};

const COMPONENT_DOMAIN: &[u8] = b"eqiora.scalar-calculus/v1\0";
const NORMAL_FORM_DOMAIN: &[u8] = b"eqiora.exact-polynomial-normal-form/v1\0";
const MAX_NORMAL_TERMS: usize = 16_384;

impl<I: Clone + Eq> ScalarCalculus<I> {
    /// Produce a replayable exact-polynomial classification proof.
    ///
    /// The result is for admission/equivalence only. It does not authorize
    /// reassociation of executable floating-point instructions.
    pub fn normalize(&self) -> Result<NormalizationProof<I>, CalculusError> {
        let normal_form = ExactPolynomial::from_calculus(self)?;
        Ok(NormalizationProof {
            rule: NormalizationRuleId::EXACT_POLYNOMIAL_V1,
            before_digest: hash(&canonical_component_bytes(self)),
            after_digest: hash(&normal_form.canonical_bytes()),
            argument_types: self.argument_types.clone(),
            result_type: self.result_type.clone(),
            normal_form,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Monomial {
    coefficient: ExactRational,
    atoms: Vec<ScalarCalculusAtom>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ExactPolynomial {
    terms: Vec<Monomial>,
}

impl ExactPolynomial {
    fn from_calculus<I>(calculus: &ScalarCalculus<I>) -> Result<Self, CalculusError> {
        let mut values: Vec<BTreeMap<Vec<ScalarCalculusAtom>, ExactRational>> =
            Vec::with_capacity(calculus.nodes().len());
        for node in calculus.nodes() {
            let polynomial = match node {
                ScalarCalculusNode::Rational(value) => BTreeMap::from([(Vec::new(), *value)]),
                ScalarCalculusNode::FormalComponent(atom) => {
                    BTreeMap::from([(vec![atom.clone()], ExactRational::integer(1))])
                }
                ScalarCalculusNode::Neg(value) => values
                    .get(calculus_index(*value, values.len())?)
                    .ok_or(CalculusError::InvalidNode)?
                    .iter()
                    .map(|(atoms, coefficient)| Ok((atoms.clone(), coefficient.checked_neg()?)))
                    .collect::<Result<_, CalculusError>>()?,
                ScalarCalculusNode::Add(left, right) => {
                    let mut result = values
                        .get(calculus_index(*left, values.len())?)
                        .ok_or(CalculusError::InvalidNode)?
                        .clone();
                    for (atoms, coefficient) in values
                        .get(calculus_index(*right, values.len())?)
                        .ok_or(CalculusError::InvalidNode)?
                    {
                        let sum = result
                            .get(atoms)
                            .copied()
                            .unwrap_or(ExactRational::integer(0))
                            .checked_add(*coefficient)?;
                        if sum.is_zero() {
                            result.remove(atoms);
                        } else {
                            result.insert(atoms.clone(), sum);
                        }
                    }
                    result
                }
                ScalarCalculusNode::Mul(left, right) => {
                    let left = values
                        .get(calculus_index(*left, values.len())?)
                        .ok_or(CalculusError::InvalidNode)?;
                    let right = values
                        .get(calculus_index(*right, values.len())?)
                        .ok_or(CalculusError::InvalidNode)?;
                    if left.len().saturating_mul(right.len()) > MAX_NORMAL_TERMS {
                        return Err(CalculusError::NormalizationLimit);
                    }
                    let mut result = BTreeMap::new();
                    for (left_atoms, left_coefficient) in left {
                        for (right_atoms, right_coefficient) in right {
                            let mut atoms = left_atoms.clone();
                            atoms.extend(right_atoms.iter().cloned());
                            atoms.sort();
                            let coefficient = left_coefficient.checked_mul(*right_coefficient)?;
                            let sum = result
                                .get(&atoms)
                                .copied()
                                .unwrap_or(ExactRational::integer(0))
                                .checked_add(coefficient)?;
                            if sum.is_zero() {
                                result.remove(&atoms);
                            } else {
                                result.insert(atoms, sum);
                            }
                        }
                    }
                    result
                }
            };
            if polynomial.len() > MAX_NORMAL_TERMS {
                return Err(CalculusError::NormalizationLimit);
            }
            values.push(polynomial);
        }
        let root = values
            .get(calculus_index(calculus.root(), values.len())?)
            .ok_or(CalculusError::InvalidNode)?;
        Ok(Self {
            terms: root
                .iter()
                .map(|(atoms, coefficient)| Monomial {
                    coefficient: *coefficient,
                    atoms: atoms.clone(),
                })
                .collect(),
        })
    }

    fn canonical_bytes(&self) -> Vec<u8> {
        let mut bytes = NORMAL_FORM_DOMAIN.to_vec();
        push_u32(&mut bytes, self.terms.len());
        for term in &self.terms {
            push_rational(&mut bytes, term.coefficient);
            push_u32(&mut bytes, term.atoms.len());
            for atom in &term.atoms {
                push_u16(&mut bytes, atom.formal());
                push_u32(&mut bytes, atom.component().len());
                for component in atom.component() {
                    bytes.extend_from_slice(&component.to_be_bytes());
                }
            }
        }
        bytes
    }
}

/// Versioned proof rule identifier understood by the independent checker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NormalizationRuleId(u16);

impl NormalizationRuleId {
    /// Exact rational commutative-polynomial classification, version 1.
    pub const EXACT_POLYNOMIAL_V1: Self = Self(1);

    /// Construct a raw ID for decoding and fail-closed compatibility checks.
    #[must_use]
    pub const fn from_raw(value: u16) -> Self {
        Self(value)
    }

    /// Raw portable rule code.
    #[must_use]
    pub const fn raw(self) -> u16 {
        self.0
    }
}

/// Replayable proof that one ordered component has an admitted exact normal
/// form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizationProof<I> {
    rule: NormalizationRuleId,
    before_digest: [u8; 32],
    after_digest: [u8; 32],
    argument_types: Vec<ExpressionType<I>>,
    result_type: ExpressionType<I>,
    normal_form: ExactPolynomial,
}

impl<I: Eq> NormalizationProof<I> {
    /// Rule used by this proof.
    #[must_use]
    pub const fn rule(&self) -> NormalizationRuleId {
        self.rule
    }

    /// Digest of the exact ordered source component.
    #[must_use]
    pub const fn before_digest(&self) -> [u8; 32] {
        self.before_digest
    }

    /// Digest of the admitted exact normal form.
    #[must_use]
    pub const fn after_digest(&self) -> [u8; 32] {
        self.after_digest
    }

    /// Replay this proof from the exact ordered source.
    ///
    /// # Errors
    /// Unknown rule versions, altered source, altered proof results, and
    /// bounded-normalization failures are rejected.
    pub fn verify(&self, source: &ScalarCalculus<I>) -> Result<(), CalculusError> {
        if self.rule != NormalizationRuleId::EXACT_POLYNOMIAL_V1 {
            return Err(CalculusError::UnsupportedProofRule(self.rule.0));
        }
        if self.argument_types != source.argument_types || self.result_type != source.result_type {
            return Err(CalculusError::ProofTypeMismatch);
        }
        let before_digest = hash(&canonical_component_bytes(source));
        if before_digest != self.before_digest {
            return Err(CalculusError::ProofSourceMismatch);
        }
        let replayed = ExactPolynomial::from_calculus(source)?;
        let after_digest = hash(&replayed.canonical_bytes());
        if replayed != self.normal_form || after_digest != self.after_digest {
            return Err(CalculusError::ProofResultMismatch);
        }
        Ok(())
    }

    /// Whether two checked proofs classify to the same exact mathematical
    /// normal form.
    ///
    /// Callers must verify each proof against its source first. Equality here
    /// does not authorize floating-point instruction reassociation.
    #[must_use]
    pub fn same_normal_form(&self, other: &Self) -> bool {
        self.rule == other.rule
            && self.argument_types == other.argument_types
            && self.result_type == other.result_type
            && self.after_digest == other.after_digest
            && self.normal_form == other.normal_form
    }
}

fn canonical_component_bytes<I>(component: &ScalarCalculus<I>) -> Vec<u8> {
    let mut bytes = COMPONENT_DOMAIN.to_vec();
    push_u32(&mut bytes, component.result_component().len());
    for coordinate in component.result_component() {
        bytes.extend_from_slice(&coordinate.to_be_bytes());
    }
    push_u32(&mut bytes, component.nodes().len());
    for node in component.nodes() {
        match node {
            ScalarCalculusNode::Rational(value) => {
                bytes.push(0);
                push_rational(&mut bytes, *value);
            }
            ScalarCalculusNode::FormalComponent(atom) => {
                bytes.push(1);
                push_u16(&mut bytes, atom.formal());
                push_u32(&mut bytes, atom.component().len());
                for coordinate in atom.component() {
                    bytes.extend_from_slice(&coordinate.to_be_bytes());
                }
            }
            ScalarCalculusNode::Neg(value) => {
                bytes.push(2);
                bytes.extend_from_slice(&value.index().to_be_bytes());
            }
            ScalarCalculusNode::Add(left, right) => {
                bytes.push(3);
                bytes.extend_from_slice(&left.index().to_be_bytes());
                bytes.extend_from_slice(&right.index().to_be_bytes());
            }
            ScalarCalculusNode::Mul(left, right) => {
                bytes.push(4);
                bytes.extend_from_slice(&left.index().to_be_bytes());
                bytes.extend_from_slice(&right.index().to_be_bytes());
            }
        }
    }
    bytes.extend_from_slice(&component.root().index().to_be_bytes());
    bytes
}

#[cfg(test)]
mod tests {
    use eqiora_core::{DimExponents, ValueShape};
    use eqiora_schema::kernel::ValueFrame;
    use eqiora_schema::kernel::typing::{ExpressionType, SpatialSupport};

    use super::*;
    use crate::calculus::{
        CalculusBuilder, CalculusNode, FormalTypeRule, OperatorExpansionExt,
        PureOperatorDefinition, ResultAxis, ResultTypeRule,
    };

    fn volume_tensor(domain: &str, dimension: DimExponents) -> ExpressionType<&str> {
        ExpressionType::shaped(
            dimension,
            ValueShape::new([2, 2]).unwrap(),
            ValueFrame::SpatialCartesian,
            Some(SpatialSupport::Volume {
                domain,
                dimensions: 2,
            }),
        )
    }

    fn equivalent_definition(distributed_two: bool) -> PureOperatorDefinition {
        let tensor = FormalTypeRule::spatial_tensor(2).unwrap();
        let mut builder =
            CalculusBuilder::new([tensor], ResultTypeRule::spatial_tensor(2).unwrap()).unwrap();
        let direct = builder
            .push(CalculusNode::FormalComponent {
                formal: 0,
                axes: [ResultAxis::new(0), ResultAxis::new(1)].into(),
            })
            .unwrap();
        let transposed = builder
            .push(CalculusNode::FormalComponent {
                formal: 0,
                axes: [ResultAxis::new(1), ResultAxis::new(0)].into(),
            })
            .unwrap();
        let sum = builder.push(CalculusNode::Add(direct, transposed)).unwrap();
        let body = if distributed_two {
            sum
        } else {
            let half = builder
                .push(CalculusNode::Rational(ExactRational::new(1, 2).unwrap()))
                .unwrap();
            let symmetric = builder.push(CalculusNode::Mul(half, sum)).unwrap();
            let two = builder
                .push(CalculusNode::Rational(ExactRational::integer(2)))
                .unwrap();
            builder.push(CalculusNode::Mul(two, symmetric)).unwrap()
        };
        builder.finish(body).unwrap()
    }

    #[test]
    fn exact_constitutive_equivalence_has_a_replayable_proof() {
        let arguments = [volume_tensor("body", DimExponents::DIMENSIONLESS)];
        let first = equivalent_definition(false)
            .instantiate(&arguments)
            .unwrap()
            .component(&[0, 1])
            .unwrap();
        let second = equivalent_definition(true)
            .instantiate(&arguments)
            .unwrap()
            .component(&[0, 1])
            .unwrap();
        let first_proof = first.normalize().unwrap();
        let second_proof = second.normalize().unwrap();
        first_proof.verify(&first).unwrap();
        second_proof.verify(&second).unwrap();
        assert!(first_proof.same_normal_form(&second_proof));
    }

    #[test]
    fn proof_checker_fails_closed_for_unknown_rule_and_source_substitution() {
        let definition = PureOperatorDefinition::symmetric_part().unwrap();
        let argument = volume_tensor("body", DimExponents::DIMENSIONLESS);
        let expansion = definition.instantiate(&[argument]).unwrap();
        let source = expansion.component(&[0, 1]).unwrap();
        let other = expansion.component(&[0, 0]).unwrap();
        let proof = source.normalize().unwrap();
        assert_eq!(
            proof.verify(&other),
            Err(CalculusError::ProofSourceMismatch)
        );

        let mut unknown = proof.clone();
        unknown.rule = NormalizationRuleId::from_raw(999);
        assert_eq!(
            unknown.verify(&source),
            Err(CalculusError::UnsupportedProofRule(999))
        );
    }

    #[test]
    fn proof_equivalence_requires_the_complete_typed_context() {
        let pressure =
            DimExponents::from_integers([1, -1, -2, 0, 0, 0, 0]).expect("bounded dimension");
        let definition = PureOperatorDefinition::symmetric_part().unwrap();
        let dimensionless = definition
            .instantiate(&[volume_tensor("body", DimExponents::DIMENSIONLESS)])
            .unwrap()
            .component(&[0, 1])
            .unwrap();
        let dimensioned = definition
            .instantiate(&[volume_tensor("body", pressure)])
            .unwrap()
            .component(&[0, 1])
            .unwrap();
        let dimensionless_proof = dimensionless.normalize().unwrap();
        let dimensioned_proof = dimensioned.normalize().unwrap();

        assert_eq!(
            dimensionless_proof.verify(&dimensioned),
            Err(CalculusError::ProofTypeMismatch)
        );
        assert!(!dimensionless_proof.same_normal_form(&dimensioned_proof));
    }
}
