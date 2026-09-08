//! Construction of reusable Component declarations.

use crate::ast::formulation::FormulationDecl;
use crate::ast::{
    ComponentDecl, ComponentItem, ConnectionSyntax, Expr, TextRange, VisibilitySyntax,
};

use super::{
    AstConstructionError, SourceAstFactory, checked_identifier, checked_range,
    validate_boundary_connection, validate_boundary_family_binder, validate_expression,
    validate_port_syntax,
};

impl SourceAstFactory {
    /// Construct one borrowed exact-clock requirement without declaring a period.
    ///
    /// # Errors
    /// Rejects malformed identifiers and ranges.
    pub fn clock_requirement(
        name: impl Into<String>,
        range: TextRange,
    ) -> Result<crate::ClockRequirementDecl, AstConstructionError> {
        Ok(crate::ClockRequirementDecl {
            comments: Default::default(),
            name: checked_identifier(name, "required clock")?,
            range: checked_range(range)?,
        })
    }

    /// Construct one reusable Component declaration.
    ///
    /// # Errors
    /// Returns an error for an invalid source identifier, member shape, or byte range.
    pub fn component(
        visibility: VisibilitySyntax,
        name: impl Into<String>,
        signature: Vec<crate::SignatureItem>,
        items: Vec<ComponentItem>,
        range: TextRange,
    ) -> Result<ComponentDecl, AstConstructionError> {
        super::signature::validate_signature(&signature)?;
        for item in &items {
            validate_component_item(item)?;
        }
        Ok(ComponentDecl {
            comments: Default::default(),
            visibility,
            name: checked_identifier(name, "component")?,
            signature,
            items,
            formulations: Vec::new(),
            range: checked_range(range)?,
        })
    }

    /// Construct a Component with one scalar-primal equality after its members.
    ///
    /// # Errors
    /// Returns an error for an invalid member, identifier, expression, or byte range.
    pub fn component_with_primal_form(
        visibility: VisibilitySyntax,
        name: impl Into<String>,
        signature: Vec<crate::SignatureItem>,
        items: Vec<ComponentItem>,
        relation: impl Into<String>,
        equality: (Expr, Expr, TextRange),
        range: TextRange,
    ) -> Result<ComponentDecl, AstConstructionError> {
        super::signature::validate_signature(&signature)?;
        for item in &items {
            validate_component_item(item)?;
        }
        let (left, right, formulation_range) = equality;
        validate_expression(&left)?;
        validate_expression(&right)?;
        let relation = checked_identifier(relation, "Formulation Relation")?;
        let formulation_range = checked_range(formulation_range)?;
        let range = checked_range(range)?;
        Ok(ComponentDecl {
            comments: Default::default(),
            visibility,
            name: checked_identifier(name, "component")?,
            signature,
            items,
            formulations: vec![FormulationDecl {
                comments: Default::default(),
                relation,
                left,
                right,
                range: formulation_range,
            }],
            range,
        })
    }
}

fn validate_component_item(item: &ComponentItem) -> Result<(), AstConstructionError> {
    let range = match item {
        ComponentItem::Let(declaration) => declaration.range(),
        ComponentItem::Parameter(declaration) => {
            if declaration.visibility() == VisibilitySyntax::Public {
                return Err(AstConstructionError::new(
                    "public parameters belong in the signature",
                ));
            }
            declaration.range()
        }
        ComponentItem::Port(declaration) => {
            if declaration.visibility() == VisibilitySyntax::Public {
                return Err(AstConstructionError::new(
                    "public ports belong in the signature",
                ));
            }
            declaration.range()
        }
        ComponentItem::PortFamily(declaration) => {
            validate_port_syntax(declaration.port().syntax())?;
            validate_boundary_family_binder(declaration.binder())?;
            declaration.range()
        }
        ComponentItem::Field(declaration) => declaration.range(),
        ComponentItem::Initial(declaration) => declaration.range(),
        ComponentItem::Clock(declaration) => declaration.range(),
        ComponentItem::Relation(declaration) => declaration.range(),
        ComponentItem::RelationFamily(declaration) => {
            validate_boundary_family_binder(declaration.binder())?;
            declaration.range()
        }
        ComponentItem::Connection(declaration) => declaration.range(),
        ComponentItem::BoundaryConnection(declaration) => {
            validate_boundary_connection(declaration)?;
            if declaration.syntax() == ConnectionSyntax::SpatialPeriodic {
                return Err(AstConstructionError::new(
                    "a spatial-periodic Connection belongs only to a closed Model",
                ));
            }
            declaration.range()
        }
        ComponentItem::IndexSet(declaration) => {
            validate_expression(declaration.extent())?;
            declaration.range()
        }
        ComponentItem::Instance(declaration) => declaration.range(),
    };
    checked_range(range).map(|_| ())
}

#[cfg(test)]
mod tests {
    use crate::{format, parse};

    use super::*;

    #[test]
    fn constructs_primal_form_without_model_item_coercion() {
        let parsed = parse(
            "form.eqi",
            "component C() { relation balance { 1 = 0; } form primal for balance { integrate(region, test(value)) = integrate(region, test(value)); } }",
        )
        .into_document()
        .unwrap();
        let source = &parsed.components()[0];
        let (_, left, right, range) = source.formulations().next().unwrap();
        let component = SourceAstFactory::component_with_primal_form(
            VisibilitySyntax::Private,
            "C",
            source.signature().to_vec(),
            source.items().to_vec(),
            "balance",
            (left.clone(), right.clone(), range),
            source.range(),
        )
        .unwrap();
        let document = SourceAstFactory::document(Vec::new(), vec![component], Vec::new()).unwrap();

        assert_eq!(document.components()[0].formulations().len(), 1);
        assert!(format(&document).contains("form primal for balance"));
    }
}
