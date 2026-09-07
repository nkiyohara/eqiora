//! Construction of reusable Component declarations.

use crate::ast::formulation::FormulationDecl;
use crate::ast::{
    ComponentDecl, ComponentItem, ConnectionSyntax, Expr, InstanceDecl, TextRange, VisibilitySyntax,
};

use super::{
    AstConstructionError, SourceAstFactory, checked_identifier, checked_range,
    validate_boundary_connection, validate_boundary_family_binder, validate_expression,
    validate_identifier, validate_port_syntax,
};

impl SourceAstFactory {
    /// Construct an exact borrowed-clock binding.
    ///
    /// # Errors
    /// Rejects malformed slot and target identifiers or byte ranges.
    pub fn clock_binding(
        slot: impl Into<String>,
        target: impl Into<String>,
        range: TextRange,
    ) -> Result<crate::ClockBindingDecl, AstConstructionError> {
        Ok(crate::ClockBindingDecl {
            comments: Default::default(),
            slot: checked_identifier(slot, "Clock binding slot")?,
            target: checked_identifier(target, "Clock binding target")?,
            range: checked_range(range)?,
        })
    }

    /// Attach the complete exact-clock binding list to an instance.
    ///
    /// # Errors
    /// Rejects malformed identifiers or byte ranges.
    pub fn bind_clocks(
        mut instance: InstanceDecl,
        bindings: Vec<crate::ClockBindingDecl>,
    ) -> Result<InstanceDecl, AstConstructionError> {
        for binding in &bindings {
            validate_identifier(binding.slot(), "Clock binding slot")?;
            validate_identifier(binding.target(), "Clock binding target")?;
            checked_range(binding.range())?;
        }
        instance.clock_bindings = bindings;
        Ok(instance)
    }

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
        items: Vec<ComponentItem>,
        range: TextRange,
    ) -> Result<ComponentDecl, AstConstructionError> {
        for item in &items {
            validate_component_item(item)?;
        }
        Ok(ComponentDecl {
            comments: Default::default(),
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
            comments: Default::default(),
            visibility,
            name: checked_identifier(name, "component")?,
            items,
            formulations: vec![FormulationDecl {
                comments: Default::default(),
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

fn validate_component_item(item: &ComponentItem) -> Result<(), AstConstructionError> {
    let range = match item {
        ComponentItem::Let(declaration) => declaration.range(),
        ComponentItem::Parameter(declaration) => declaration.range(),
        ComponentItem::Port(declaration) => declaration.range(),
        ComponentItem::PortFamily(declaration) => {
            validate_port_syntax(declaration.port().syntax())?;
            validate_boundary_family_binder(declaration.binder())?;
            declaration.range()
        }
        ComponentItem::Support(declaration) => declaration.range(),
        ComponentItem::FieldRequirement(declaration) => declaration.range(),
        ComponentItem::Field(declaration) => declaration.range(),
        ComponentItem::Initial(declaration) => declaration.range(),
        ComponentItem::Clock(declaration) => declaration.range(),
        ComponentItem::ClockRequirement(declaration) => declaration.range(),
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
