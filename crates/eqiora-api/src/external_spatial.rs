//! Admission of one compiler-owned external-spatial Component occurrence.

use eqiora_compiler::{
    ExternalComponentBinding, ExternalGeometrySupportBinding, compile_external_component,
};
use eqiora_core::Diagnostic;
use eqiora_core::diagnostic::codes;
use eqiora_geometry::{CanonicalGeometryRef, CanonicalGeometryV1};
use eqiora_schema::kernel::GeometryDigest;

use crate::ModelDocument;

impl ModelDocument {
    /// Compile one definitions-only `.eqi` Component against exact external
    /// Geometry supports and return the ordinary immutable Model document.
    ///
    /// The supplied Geometry is the sole concrete shape authority. Source
    /// owns only abstract support contracts and physical-law structure.
    ///
    /// # Errors
    /// Returns source, binding, geometry-admission, typed-lowering, or
    /// canonical artifact diagnostics. No partial Model is returned.
    pub fn bind_external_component(
        filename: &str,
        source: &str,
        geometry: &CanonicalGeometryV1,
        binding: &ExternalComponentBinding,
    ) -> Result<Self, Vec<Diagnostic>> {
        let geometry_ref = CanonicalGeometryRef::from(geometry);
        validate_geometry_bindings(filename, geometry_ref, binding.supports())?;
        let compiled = compile_external_component(filename, source, binding)?;
        Self::accept_compiled_with_geometry(compiled, &[geometry_ref])
    }
}

fn validate_geometry_bindings(
    filename: &str,
    geometry: CanonicalGeometryRef<'_>,
    supports: &[ExternalGeometrySupportBinding],
) -> Result<(), Vec<Diagnostic>> {
    let expected_digest = GeometryDigest::new(geometry.digest_bytes());
    let mut diagnostics = Vec::new();
    for support in supports {
        if support.geometry() != expected_digest {
            diagnostics.push(binding_error(
                filename,
                format!(
                    "external support `{}` belongs to a foreign Geometry identity",
                    support.slot()
                ),
            ));
            continue;
        }
        let Some(dimension) = geometry.entity_set_dimension(support.entity_set()) else {
            diagnostics.push(binding_error(
                filename,
                format!(
                    "Geometry has no entity set `{}` for external support `{}`",
                    support.entity_set(),
                    support.slot()
                ),
            ));
            continue;
        };
        match support {
            ExternalGeometrySupportBinding::Region {
                ambient_dimension, ..
            } => {
                if *ambient_dimension != geometry.ambient_dimension()
                    || dimension != geometry.topological_dimension()
                {
                    diagnostics.push(binding_error(
                        filename,
                        format!(
                            "external region support `{}` has topological dimension {dimension} in a {}D Geometry, not declared ambient dimension {ambient_dimension}",
                            support.slot(),
                            geometry.ambient_dimension(),
                        ),
                    ));
                }
            }
            ExternalGeometrySupportBinding::Boundary { .. } => {
                if dimension.checked_add(1) != Some(geometry.topological_dimension()) {
                    diagnostics.push(binding_error(
                        filename,
                        format!(
                            "external boundary support `{}` has entity-set dimension {dimension}, expected {}",
                            support.slot(),
                            geometry.topological_dimension().saturating_sub(1),
                        ),
                    ));
                }
            }
        }
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

fn binding_error(filename: &str, message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::LANGUAGE_TYPE_ERROR, message).with_span(eqiora_core::Span {
        file: filename.to_owned(),
        start: 0,
        end: 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use eqiora_compiler::{ExternalGeometrySupportBinding, ExternalParameterBinding};

    const SOURCE: &str = r#"
public component BoundaryLaw {
  public support body: volume(ambient_dimension = 2);
  public support wall: boundary(parent = body);
  public parameter value: 1;
  representation space = continuum;
  field state on body as space: 1 = 0;
  relation volume_law continuous on body { state - value = 0; }
  relation wall_law continuous on wall { trace(state) = 0; }
}
"#;

    fn geometry() -> CanonicalGeometryV1 {
        CanonicalGeometryV1::from_circular_hole_named_roles(
            [[0.0, 2.2], [0.0, 0.41]],
            [0.2, 0.2],
            0.05,
            1.0e-12,
            "fluid",
            "inlet",
            "outlet",
            "walls",
            "walls",
            "cylinder",
        )
        .unwrap()
    }

    #[test]
    fn exact_geometry_closes_compilation_and_canonical_reconstruction() {
        let geometry = geometry();
        let digest = GeometryDigest::new(geometry.digest_bytes());
        let binding = ExternalComponentBinding::new(
            "BoundModel",
            "BoundaryLaw",
            vec![
                ExternalGeometrySupportBinding::region("body", digest, "fluid", 2),
                ExternalGeometrySupportBinding::boundary("wall", digest, "walls", "body"),
            ],
            vec![ExternalParameterBinding::new("value", 2.0)],
        );
        let document =
            ModelDocument::bind_external_component("boundary-law.eqi", SOURCE, &geometry, &binding)
                .unwrap();
        assert!(!document.canonical_json().unwrap().is_empty());
        for name in ["body", "wall", "definition.state"] {
            assert!(document.aliases().contains_key(name), "missing `{name}`");
        }
    }

    #[test]
    fn foreign_and_dimension_wrong_geometry_bindings_fail_before_compilation() {
        let geometry = geometry();
        let digest = GeometryDigest::new(geometry.digest_bytes());
        let cases = [
            (
                ExternalGeometrySupportBinding::region(
                    "body",
                    GeometryDigest::new([0x44; 32]),
                    "fluid",
                    2,
                ),
                "foreign Geometry identity",
            ),
            (
                ExternalGeometrySupportBinding::region("body", digest, "walls", 2),
                "topological dimension 1",
            ),
            (
                ExternalGeometrySupportBinding::boundary("wall", digest, "fluid", "body"),
                "entity-set dimension 2",
            ),
        ];
        for (support, expected) in cases {
            let binding = ExternalComponentBinding::new(
                "Rejected",
                "BoundaryLaw",
                vec![support],
                vec![ExternalParameterBinding::new("value", 2.0)],
            );
            let diagnostics = ModelDocument::bind_external_component(
                "boundary-law.eqi",
                SOURCE,
                &geometry,
                &binding,
            )
            .unwrap_err();
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message().contains(expected)),
                "missing `{expected}` in {diagnostics:#?}"
            );
        }
    }

    #[test]
    fn registered_evidence() {
        exact_geometry_closes_compilation_and_canonical_reconstruction();
        foreign_and_dimension_wrong_geometry_bindings_fail_before_compilation();
    }
}
