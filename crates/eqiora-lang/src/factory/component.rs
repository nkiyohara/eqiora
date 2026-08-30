//! Construction of reusable Component declarations.

use crate::ast::formulation::FormulationDecl;
use crate::ast::{ComponentDecl, ComponentItem, Expr, TextRange, VisibilitySyntax};

use super::{
    AstConstructionError, SourceAstFactory, checked_identifier, checked_range,
    validate_component_item, validate_expression,
};

impl SourceAstFactory {
    /// Construct one reusable Component declaration.
    ///
    /// # Errors
    /// Returns an error for an invalid source identifier, member shape, or byte range.
    pub fn component(
        visibility: VisibilitySyntax,
        name: impl Into<String>,
        items: Vec<ComponentItem>,
        range: TextRange,
    ) -> Result<ComponentDecl, AstConstructionError> {
        for item in &items {
            validate_component_item(item)?;
        }
        Ok(ComponentDecl {
            visibility,
            name: checked_identifier(name, "component")?,
            items,
            formulations: Vec::new(),
            property_requirements: Vec::new(),
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
        items: Vec<ComponentItem>,
        relation: impl Into<String>,
        equality: (Expr, Expr, TextRange),
        range: TextRange,
    ) -> Result<ComponentDecl, AstConstructionError> {
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
            visibility,
            name: checked_identifier(name, "component")?,
            items,
            formulations: vec![FormulationDecl {
                relation,
                left,
                right,
                range: formulation_range,
            }],
            property_requirements: Vec::new(),
            range,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::{format, parse};

    use super::*;

    #[test]
    fn constructs_primal_form_without_model_item_coercion() {
        let parsed = parse(
            "form.eqi",
            "component C { relation balance continuous { 1 = 0; } form primal for balance { integrate(region, test(value)) = integrate(region, test(value)); } }",
        )
        .into_document()
        .unwrap();
        let source = &parsed.components()[0];
        let (_, left, right, range) = source.formulations().next().unwrap();
        let component = SourceAstFactory::component_with_primal_form(
            VisibilitySyntax::Private,
            "C",
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
