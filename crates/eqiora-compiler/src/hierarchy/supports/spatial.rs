//! Exact declared Cartesian support contexts before numeric bound evaluation.
use super::*;

pub(in crate::hierarchy) fn model_spatial_supports(
    file: &str,
    model: &ModelDecl,
) -> Result<BTreeMap<String, SpatialSupport<String>>, Vec<Diagnostic>> {
    declared_spatial_supports(
        file,
        model.signature(),
        model.items().iter().filter_map(|item| match item {
            Item::Domain(value) => Some(value),
            _ => None,
        }),
    )
}

pub(in crate::hierarchy) fn component_spatial_supports(
    file: &str,
    component: &ComponentDecl,
) -> Result<BTreeMap<String, SpatialSupport<String>>, Vec<Diagnostic>> {
    declared_spatial_supports(file, component.signature(), std::iter::empty())
}

fn declared_spatial_supports<'a>(
    file: &str,
    signature: &[eqiora_lang::SignatureItem],
    domains: impl Iterator<Item = &'a eqiora_lang::DomainDecl>,
) -> Result<BTreeMap<String, SpatialSupport<String>>, Vec<Diagnostic>> {
    let interface = signature_support_interface(file, signature)?;
    let mut supports = interface
        .iter()
        .map(|(name, contract)| (name.to_owned(), contract.support().clone()))
        .collect::<BTreeMap<_, _>>();
    let mut boundaries = Vec::new();
    for declaration in domains {
        match declaration.syntax() {
            DomainSyntax::CartesianBox(bounds) if !bounds.is_empty() => {
                supports.insert(
                    declaration.name().to_owned(),
                    SpatialSupport::Volume {
                        domain: declaration.name().to_owned(),
                        dimensions: bounds.len(),
                    },
                );
            }
            DomainSyntax::Boundary { parent, .. } => boundaries.push((declaration, parent)),
            _ => {}
        }
    }

    let mut diagnostics = Vec::new();
    for (declaration, parent) in boundaries {
        match supports.get(parent) {
            Some(SpatialSupport::Volume { dimensions, .. }) => {
                supports.insert(
                    declaration.name().to_owned(),
                    SpatialSupport::Boundary {
                        domain: declaration.name().to_owned(),
                        parent: parent.clone(),
                        dimensions: *dimensions,
                    },
                );
            }
            Some(SpatialSupport::Boundary { .. }) => diagnostics.push(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                file,
                declaration.range(),
                "boundary support binding cannot use a boundary-of-boundary Domain",
            )),
            Some(SpatialSupport::Interface { .. }) => diagnostics.push(source_error(
                codes::LANGUAGE_LOWERING_ERROR,
                file,
                declaration.range(),
                "derived interface support cannot appear in source Domain resolution",
            )),
            None => {}
        }
    }
    if diagnostics.is_empty() {
        Ok(supports)
    } else {
        Err(diagnostics)
    }
}
