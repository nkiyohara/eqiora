use crate::ast::{Document, Expr, NamePath, TextRange, VisibilitySyntax};

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PropertyContractDecl {
    pub(crate) comments: crate::ast::comments::SourceComments,
    pub(crate) visibility: VisibilitySyntax,
    pub(crate) name: String,
    pub(crate) value_type: crate::ValueTypeSyntax,
    pub(crate) range: TextRange,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PropertyReleaseDecl {
    pub(crate) comments: crate::ast::comments::SourceComments,
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
pub(crate) struct MaterialCompositionDecl {
    pub(crate) comments: crate::ast::comments::SourceComments,
    pub(crate) visibility: VisibilitySyntax,
    pub(crate) name: String,
    pub(crate) properties: Vec<PropertyBindingDecl>,
    pub(crate) range: TextRange,
}

/// One exact nominal property contract in a shared external signature.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentPropertyDecl {
    pub(crate) comments: crate::ast::comments::SourceComments,
    pub(crate) name: String,
    pub(crate) contract: NamePath,
    pub(crate) range: TextRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PropertyBindingDecl {
    pub(crate) comments: crate::ast::comments::SourceComments,
    pub(crate) property: String,
    pub(crate) release: NamePath,
    pub(crate) range: TextRange,
}

impl Document {
    #[must_use]
    pub fn property_contract_syntax(
        &self,
    ) -> impl ExactSizeIterator<Item = (VisibilitySyntax, &str, &crate::ValueTypeSyntax, TextRange)>
    {
        self.property_contracts.iter().map(|value| {
            (
                value.visibility,
                value.name.as_str(),
                &value.value_type,
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
    pub fn material_composition_syntax(
        &self,
    ) -> impl ExactSizeIterator<
        Item = (
            VisibilitySyntax,
            &str,
            Vec<(&str, &NamePath, TextRange)>,
            TextRange,
        ),
    > {
        self.material_compositions.iter().map(|value| {
            (
                value.visibility,
                value.name.as_str(),
                value
                    .properties
                    .iter()
                    .map(|binding| (binding.property.as_str(), &binding.release, binding.range))
                    .collect(),
                value.range,
            )
        })
    }

    #[must_use]
    pub fn isolated_property_declarations(&self) -> Vec<(String, VisibilitySyntax, Self)> {
        let mut values = self
            .property_contracts
            .iter()
            .map(|declaration| {
                (
                    declaration.name.clone(),
                    declaration.visibility,
                    Self {
                        comments: Default::default(),
                        imports: Vec::new(),
                        enumerations: self.enumerations.clone(),
                        finite_spaces: self.finite_spaces.clone(),
                        dimensions: self.dimensions.clone(),
                        property_contracts: vec![declaration.clone()],
                        property_releases: Vec::new(),
                        material_compositions: Vec::new(),
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
                Self {
                    comments: Default::default(),
                    imports: Vec::new(),
                    enumerations: self.enumerations.clone(),
                    finite_spaces: self.finite_spaces.clone(),
                    dimensions: self.dimensions.clone(),
                    property_contracts: Vec::new(),
                    property_releases: vec![declaration.clone()],
                    material_compositions: Vec::new(),
                    connectors: Vec::new(),
                    components: Vec::new(),
                    pure_operators: Vec::new(),
                    models: Vec::new(),
                },
            )
        }));
        values.extend(self.material_compositions.iter().map(|declaration| {
            (
                declaration.name.clone(),
                declaration.visibility,
                Self {
                    comments: Default::default(),
                    imports: Vec::new(),
                    enumerations: self.enumerations.clone(),
                    finite_spaces: self.finite_spaces.clone(),
                    dimensions: self.dimensions.clone(),
                    property_contracts: Vec::new(),
                    property_releases: Vec::new(),
                    material_compositions: vec![declaration.clone()],
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

impl ComponentPropertyDecl {
    /// Public signature name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }
    /// Exact nominal property contract path.
    #[must_use]
    pub const fn contract(&self) -> &NamePath {
        &self.contract
    }
    /// Full signature-entry range.
    #[must_use]
    pub const fn range(&self) -> TextRange {
        self.range
    }
}
