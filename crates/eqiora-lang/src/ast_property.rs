use crate::ast::{
    ComponentDecl, Document, Expr, InstanceDecl, NamePath, TextRange, VisibilitySyntax,
};

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PropertyContractDecl {
    pub(crate) visibility: VisibilitySyntax,
    pub(crate) name: String,
    pub(crate) dimension: Expr,
    pub(crate) range: TextRange,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PropertyReleaseDecl {
    pub(crate) visibility: VisibilitySyntax,
    pub(crate) name: String,
    pub(crate) contract: NamePath,
    pub(crate) source_value: Expr,
    pub(crate) source_dimension: Expr,
    pub(crate) coherent_si_scale: Expr,
    pub(crate) citation: NamePath,
    pub(crate) license: NamePath,
    pub(crate) range: TextRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ComponentPropertyDecl {
    pub(crate) name: String,
    pub(crate) contract: NamePath,
    pub(crate) range: TextRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PropertyBindingDecl {
    pub(crate) property: String,
    pub(crate) release: NamePath,
    pub(crate) range: TextRange,
}

impl Document {
    #[must_use]
    pub fn property_contract_syntax(
        &self,
    ) -> impl ExactSizeIterator<Item = (VisibilitySyntax, &str, &Expr, TextRange)> {
        self.property_contracts.iter().map(|value| {
            (
                value.visibility,
                value.name.as_str(),
                &value.dimension,
                value.range,
            )
        })
    }

    #[must_use]
    pub fn property_release_syntax(
        &self,
    ) -> impl ExactSizeIterator<
        Item = (
            VisibilitySyntax,
            &str,
            &NamePath,
            &Expr,
            &Expr,
            &Expr,
            &NamePath,
            &NamePath,
            TextRange,
        ),
    > {
        self.property_releases.iter().map(|value| {
            (
                value.visibility,
                value.name.as_str(),
                &value.contract,
                &value.source_value,
                &value.source_dimension,
                &value.coherent_si_scale,
                &value.citation,
                &value.license,
                value.range,
            )
        })
    }

    #[must_use]
    pub fn isolated_property_declarations(&self) -> Vec<(String, VisibilitySyntax, bool, Self)> {
        let mut values = self
            .property_contracts
            .iter()
            .map(|declaration| {
                (
                    declaration.name.clone(),
                    declaration.visibility,
                    true,
                    Self {
                        dimensions: self.dimensions.clone(),
                        property_contracts: vec![declaration.clone()],
                        property_releases: Vec::new(),
                        connectors: Vec::new(),
                        components: Vec::new(),
                        pure_operators: Vec::new(),
                        models: Vec::new(),
                    },
                )
            })
            .collect::<Vec<_>>();
        values.extend(self.property_releases.iter().map(|declaration| {
            (
                declaration.name.clone(),
                declaration.visibility,
                false,
                Self {
                    dimensions: self.dimensions.clone(),
                    property_contracts: Vec::new(),
                    property_releases: vec![declaration.clone()],
                    connectors: Vec::new(),
                    components: Vec::new(),
                    pure_operators: Vec::new(),
                    models: Vec::new(),
                },
            )
        }));
        values
    }
}

impl ComponentDecl {
    #[must_use]
    pub fn property_requirement_syntax(
        &self,
    ) -> impl ExactSizeIterator<Item = (&str, &NamePath, TextRange)> {
        self.property_requirements
            .iter()
            .map(|value| (value.name.as_str(), &value.contract, value.range))
    }
}

impl InstanceDecl {
    #[must_use]
    pub fn property_binding_syntax(
        &self,
    ) -> impl ExactSizeIterator<Item = (&str, &NamePath, TextRange)> {
        self.property_bindings
            .iter()
            .map(|value| (value.property.as_str(), &value.release, value.range))
    }
}

impl fmt::Display for NamePath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.text)
    }
}

impl NamePath {
    #[must_use]
    pub fn is_qualified(&self) -> bool {
        self.segments.len() > 1
    }
}
use core::fmt;
