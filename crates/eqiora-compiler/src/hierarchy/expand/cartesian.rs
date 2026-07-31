use eqiora_core::DynQuantity;
use eqiora_schema::kernel::AxisBounds;

use crate::dimensions::length_dimension;

use super::*;

impl RootExpansion<'_, '_> {
    pub(super) fn allocate_cartesian_boundaries(
        &mut self,
        scope: &mut Scope,
        identities: &ScopeIdentities,
    ) -> Result<(), Diagnostic> {
        let model = self.model.clone();
        for item in model.items() {
            let Item::Domain(declaration) = item else {
                continue;
            };
            let DomainSyntax::Boundary { parent, .. } = declaration.syntax() else {
                continue;
            };
            let Some(SpatialSupport::Volume {
                domain: parent_identity,
                dimensions,
            }) = scope.spatial_support(parent).cloned()
            else {
                return Err(source_error(
                    codes::LANGUAGE_TYPE_ERROR,
                    self.model.file,
                    declaration.range(),
                    format!("boundary parent `{parent}` is not an exact spatial volume support"),
                ));
            };
            scope.insert_spatial_support(
                declaration.name().to_owned(),
                SpatialSupport::Boundary {
                    domain: identities.entities[declaration.name()].full,
                    parent: parent_identity,
                    dimensions,
                },
            );

            let DomainSyntax::Boundary { axis, side, .. } = declaration.syntax() else {
                unreachable!("boundary syntax was selected above");
            };
            let parent_declaration = model.items().iter().find_map(|item| match item {
                Item::Domain(candidate) if candidate.name() == parent => Some(candidate),
                _ => None,
            });
            let Some(parent_declaration) = parent_declaration else {
                continue;
            };
            let DomainSyntax::CartesianBox(bounds) = parent_declaration.syntax() else {
                continue;
            };
            let axes = bounds
                .iter()
                .map(|(lower, upper)| {
                    let (Some(lower), Some(upper)) = (lower.fixed_value(), upper.fixed_value())
                    else {
                        return Ok(None);
                    };
                    AxisBounds::new(
                        DynQuantity::new(lower, length_dimension()),
                        DynQuantity::new(upper, length_dimension()),
                    )
                    .map(Some)
                })
                .collect::<Result<Vec<_>, _>>()?;
            let side = match side {
                eqiora_lang::BoundarySideSyntax::Lower => BoundarySide::Lower,
                eqiora_lang::BoundarySideSyntax::Upper => BoundarySide::Upper,
            };
            let embedding = axes
                .into_iter()
                .collect::<Option<Vec<_>>>()
                .map(|axes| {
                    CartesianBoundaryEmbedding::derive(&axes, *axis, side).ok_or_else(|| {
                        source_error(
                            codes::LANGUAGE_TYPE_ERROR,
                            self.model.file,
                            declaration.range(),
                            "Cartesian boundary axis exceeds its exact parent dimension",
                        )
                    })
                })
                .transpose()?;
            self.boundary_embeddings
                .insert(identities.entities[declaration.name()].full, embedding);
            self.boundary_parents.insert(
                identities.entities[declaration.name()].full,
                parent_identity,
            );
            self.boundary_sides
                .insert(identities.entities[declaration.name()].full, (*axis, side));
        }
        Ok(())
    }
}
