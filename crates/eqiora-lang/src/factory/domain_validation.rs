use crate::ast::DomainSyntax;
use crate::cartesian::CartesianCoordinateSyntax;

use super::{AstConstructionError, validate_expression, validate_finite, validate_identifier};

pub(super) fn validate_domain_syntax(syntax: &DomainSyntax) -> Result<(), AstConstructionError> {
    match syntax {
        DomainSyntax::CartesianBox(bounds) => {
            if bounds.is_empty() {
                return Err(AstConstructionError::new(
                    "a Cartesian box requires at least one coordinate pair",
                ));
            }
            for coordinate in bounds.iter().flat_map(|(lower, upper)| [lower, upper]) {
                match coordinate {
                    CartesianCoordinateSyntax::Fixed { value, .. } => {
                        validate_finite(*value, "Cartesian coordinate")?
                    }
                    CartesianCoordinateSyntax::Parameter { name, .. } => {
                        validate_identifier(name, "Cartesian coordinate Parameter")?
                    }
                }
            }
            Ok(())
        }
        DomainSyntax::Boundary { parent, .. } => validate_identifier(parent, "parent Domain"),
        DomainSyntax::ScalarPhysical {
            across_dimension,
            through_dimension,
        } => {
            validate_expression(across_dimension)?;
            validate_expression(through_dimension)
        }
    }
}
