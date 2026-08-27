//! Admission of one compiler-owned external-spatial Component occurrence.

use eqiora_artifact::{
    AcceptedModelArtifact, CanonicalModelArtifact, ModelDecoderLimits, ModelTransactionEnvelope,
};
use eqiora_compiler::CompiledModel;
use eqiora_core::diagnostic::codes;
use eqiora_core::{Diagnostic, DynQuantity};
use eqiora_geometry::{CanonicalGeometryRef, CanonicalGeometryV1, NamedEntitySet};
use eqiora_graph::{GraphStore, InMemoryGraphStore, Revision};
use eqiora_sem::KernelProgram;

use crate::{ModelDocument, aliases, single_diagnostic};

impl ModelDocument {
    /// Compile one definitions-only `.eqi` Component against exact-name
    /// selections borrowed from one common Geometry revision.
    ///
    /// The compiler selects the sole public Component unless `component` is
    /// explicit, derives Parameter dimensions from source, expands one
    /// ephemeral root occurrence, and returns the ordinary immutable Model.
    #[doc(hidden)]
    pub fn compile_with_geometry(
        filename: &str,
        source: &str,
        geometry: &CanonicalGeometryV1,
        component: Option<&str>,
        parameters: &[(&str, f64)],
    ) -> Result<Self, Vec<Diagnostic>> {
        let compiled = CompiledModel::compile_external_geometry_component(
            filename, source, geometry, component, parameters,
        )?;
        Self::accept_external_compiled(compiled, geometry)
    }

    /// Compile one definitions-only `.eqi` Component against typed selections
    /// borrowed from one exact common Geometry revision.
    ///
    /// Each support is `(slot, selection, parent)`. A volume has no parent; a
    /// boundary supplies `(parent slot, parent selection)`. Parameter tuples
    /// carry explicit coherent-SI dimensions. The compiler materializes one
    /// ephemeral root occurrence through the ordinary hierarchy expansion and
    /// typed transaction lowerer; no compiled-package lifecycle is exposed.
    ///
    /// # Errors
    /// Returns source, binding, typed-lowering, graph, semantic-admission, or
    /// artifact diagnostics. No partial Model is returned.
    #[allow(
        clippy::type_complexity,
        reason = "the closed tuple keeps this seam from introducing a public lifecycle type"
    )]
    #[doc(hidden)]
    pub fn compile_external_component(
        filename: &str,
        source: &str,
        geometry: &CanonicalGeometryV1,
        model: &str,
        component: &str,
        supports: &[(&str, &NamedEntitySet, Option<(&str, &NamedEntitySet)>)],
        parameters: &[(&str, DynQuantity)],
    ) -> Result<Self, Vec<Diagnostic>> {
        let compiled = CompiledModel::compile_external_component(
            filename,
            source,
            model,
            component,
            CanonicalGeometryRef::from(geometry),
            supports,
            parameters,
        )?;
        Self::accept_external_compiled(compiled, geometry)
    }

    fn accept_external_compiled(
        compiled: CompiledModel,
        geometry: &CanonicalGeometryV1,
    ) -> Result<Self, Vec<Diagnostic>> {
        let aliases = aliases(compiled.symbols());
        let model = compiled.model();
        let transaction = ModelTransactionEnvelope::from_transaction(compiled.transaction())
            .and_then(|envelope| envelope.to_transaction())
            .map_err(single_diagnostic)?;
        let mut store = InMemoryGraphStore::new();
        store.commit(transaction)?;
        let geometry_ref = CanonicalGeometryRef::from(geometry);
        let program =
            KernelProgram::from_snapshot_with_geometry(&store.snapshot(), model, &[geometry_ref])?;
        let artifact = AcceptedModelArtifact::from_program(&program).map_err(single_diagnostic)?;
        let bytes = artifact.canonical_json().map_err(single_diagnostic)?;
        let artifact = AcceptedModelArtifact::from_json(&bytes, ModelDecoderLimits::default())
            .map_err(single_diagnostic)?;
        let reference = artifact.artifact_reference().map_err(single_diagnostic)?;
        let (transaction, model) = artifact.to_transaction()?;
        let store = InMemoryGraphStore::restore_snapshot(
            transaction,
            Revision(artifact.source_revision()),
        )?;
        let program =
            KernelProgram::from_snapshot_with_geometry(&store.snapshot(), model, &[geometry_ref])?;
        if program.model() != reference.model()
            || program.revision().0 != reference.semantic_revision().get()
        {
            return Err(single_diagnostic(Diagnostic::error(
                codes::INVALID_ARTIFACT,
                "replayed Model identity or semantic revision differs from its exact artifact reference",
            )));
        }
        let document = Self {
            program,
            artifact,
            aliases,
            store,
            geometry_authority: vec![geometry.clone()],
        };
        document
            .replay_with_retained_geometry()
            .map_err(single_diagnostic)?;
        Ok(document)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use super::*;
    use eqiora_core::DimExponents;
    use eqiora_geometry::{
        CadAuthoredGraph, ConstrainedRectangleV1, EDGE_DIMENSION, FACE_DIMENSION, PlanarFace,
        PlanarRegion,
    };
    use eqiora_schema::kernel::{ExprNode, KernelNode, SymbolRef};

    const SOURCE: &str = r#"
public component FluidBoundaryLaw {
  public support fluid: volume(ambient_dimension = 2);
  public support inlet: boundary(parent = fluid);
  public support outlet: boundary(parent = fluid);
  public support walls: boundary(parent = fluid);
  public support cylinder: boundary(parent = fluid);
  public parameter value: 1;
  representation space = continuum;
  field state on fluid as space: 1 = 0;
  relation volume_law continuous on fluid { state - value = 0; }
  relation inlet_law continuous on inlet { trace(state) = 0; }
  relation outlet_law continuous on outlet { trace(state) = 0; }
  relation walls_law continuous on walls { trace(state) = 0; }
  relation cylinder_law continuous on cylinder { trace(state) = 0; }
}
"#;

    const STEADY_FLOW_PAST_CYLINDER_COMPONENT: &str = r#"
public component SteadyFlowPastCylinder {
  public support fluid: volume(ambient_dimension = 2);
  public support inlet: boundary(parent = fluid);
  public support outlet: boundary(parent = fluid);
  public support walls: boundary(parent = fluid);
  public support cylinder: boundary(parent = fluid);
  public parameter dynamic_viscosity: kg / (m * s);
  public parameter zero_pressure: kg / (m * s ^ 2);
  public parameter inlet_speed: m / s;
  public parameter channel_height: m;
  representation space = continuum;

  field velocity on fluid as space: m / s shape spatial_vector;
  field pressure on fluid as space: kg / (m * s ^ 2) = 0;
  field force_potential on fluid as space: kg / (m * s ^ 2) = 0;
  field inlet_profile on fluid as space: m / s = 0;

  relation force_definition continuous on fluid {
    force_potential - zero_pressure = 0;
  }
  relation inlet_profile_definition continuous on fluid {
    inlet_profile
      - 4 * inlet_speed * coordinate(1) * (channel_height - coordinate(1))
        / channel_height ^ 2 = 0;
  }
  relation momentum continuous on fluid {
    -div(
      2 * dynamic_viscosity * symmetric_part(grad(velocity))
      - isotropic_lift(pressure)
    ) - grad(force_potential) = 0;
  }
  relation incompressibility continuous on fluid {
    div(velocity) = 0;
  }

  relation inlet_velocity continuous on inlet {
    trace(velocity) + normal(isotropic_lift(inlet_profile)) = 0;
  }
  relation outlet_traction continuous on outlet {
    normal(
      2 * dynamic_viscosity * symmetric_part(grad(velocity))
      - isotropic_lift(pressure)
    ) = 0;
  }
  relation wall_velocity continuous on walls { trace(velocity) = 0; }
  relation cylinder_velocity continuous on cylinder { trace(velocity) = 0; }
}
"#;

    fn fixture_geometry() -> CanonicalGeometryV1 {
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
        .expect("bounded common Geometry")
    }

    fn fixture_geometry_v2() -> CanonicalGeometryV1 {
        let predecessor = CadAuthoredGraph::new(
            ConstrainedRectangleV1::new((0.0, 2.2), (0.0, 0.41), 0.0).unwrap(),
            1.0,
            1.0e-10,
        )
        .unwrap();
        let end_cap = predecessor.face_handle("end-cap").unwrap();
        let x_lower = predecessor.face_handle("profile-x-lower").unwrap();
        let x_upper = predecessor.face_handle("profile-x-upper").unwrap();
        let y_lower = predecessor.face_handle("profile-y-lower").unwrap();
        let y_upper = predecessor.face_handle("profile-y-upper").unwrap();
        let graph = predecessor
            .circular_through_cut([0.2, 0.2], 0.05, 1.0e-10)
            .unwrap();
        let cut_wall = graph.face_handle("cut-wall").unwrap();
        graph
            .planar_result()
            .unwrap()
            .with_named_topology(&BTreeMap::from([
                ("fluid".to_owned(), vec![end_cap]),
                ("inlet".to_owned(), vec![x_lower]),
                ("outlet".to_owned(), vec![x_upper]),
                ("walls".to_owned(), vec![y_lower, y_upper]),
                ("cylinder".to_owned(), vec![cut_wall]),
            ]))
            .unwrap()
    }

    fn cylinder_parameters() -> [(&'static str, DynQuantity); 4] {
        [
            (
                "dynamic_viscosity",
                DynQuantity::new(
                    1.0e-3,
                    DimExponents {
                        length: -1,
                        mass: 1,
                        time: -1,
                        ..DimExponents::DIMENSIONLESS
                    },
                ),
            ),
            (
                "zero_pressure",
                DynQuantity::new(
                    0.0,
                    DimExponents {
                        length: -1,
                        mass: 1,
                        time: -2,
                        ..DimExponents::DIMENSIONLESS
                    },
                ),
            ),
            (
                "inlet_speed",
                DynQuantity::new(
                    0.3,
                    DimExponents {
                        length: 1,
                        time: -1,
                        ..DimExponents::DIMENSIONLESS
                    },
                ),
            ),
            (
                "channel_height",
                DynQuantity::new(
                    0.41,
                    DimExponents {
                        length: 1,
                        ..DimExponents::DIMENSIONLESS
                    },
                ),
            ),
        ]
    }

    fn compile_cylinder(
        geometry: &CanonicalGeometryV1,
        parameters: &[(&str, DynQuantity)],
    ) -> ModelDocument {
        let fluid = geometry.entity_set("fluid").unwrap();
        let supports = [
            ("fluid", fluid, None),
            (
                "inlet",
                geometry.entity_set("inlet").unwrap(),
                Some(("fluid", fluid)),
            ),
            (
                "outlet",
                geometry.entity_set("outlet").unwrap(),
                Some(("fluid", fluid)),
            ),
            (
                "walls",
                geometry.entity_set("walls").unwrap(),
                Some(("fluid", fluid)),
            ),
            (
                "cylinder",
                geometry.entity_set("cylinder").unwrap(),
                Some(("fluid", fluid)),
            ),
        ];
        ModelDocument::compile_external_component(
            "steady-flow-past-cylinder.eqi",
            STEADY_FLOW_PAST_CYLINDER_COMPONENT,
            geometry,
            "SteadyFlowPastCylinderModel",
            "SteadyFlowPastCylinder",
            &supports,
            parameters,
        )
        .expect("Geometry v2 and actual Component close one common Model")
    }

    #[test]
    fn construction_named_geometry_v2_compiles_the_actual_cylinder_component() {
        let geometry = fixture_geometry_v2();
        let parameters = cylinder_parameters();
        let document = compile_cylinder(&geometry, &parameters);

        assert_eq!(
            document.geometry_authority.as_slice(),
            std::slice::from_ref(&geometry)
        );
        let bytes = document.canonical_json().expect("authority-backed replay");
        assert!(!bytes.is_empty());
        assert_eq!(
            document.replay_with_retained_geometry().unwrap(),
            *document.program()
        );
        let mut parameter_ids = BTreeSet::new();
        for (name, expected) in parameters {
            let parameter = document
                .aliases()
                .get(name)
                .copied()
                .unwrap_or_else(|| panic!("missing root Parameter `{name}`"));
            assert_eq!(
                document.aliases().get(&format!("definition.{name}")),
                Some(&parameter),
                "Component slot must retain the root Parameter identity",
            );
            let Some(KernelNode::Parameter(definition)) = document.program().node(parameter) else {
                panic!("`{name}` does not resolve to a Parameter")
            };
            assert_eq!(definition.value(), expected);
            parameter_ids.insert(parameter);
        }
        assert_eq!(parameter_ids.len(), 4);
        let parameter_references = document
            .program()
            .nodes()
            .filter_map(|node| match node {
                KernelNode::Relation(relation) => Some(relation),
                _ => None,
            })
            .flat_map(|relation| relation.residuals().nodes())
            .filter_map(|node| match node {
                ExprNode::Symbol(SymbolRef::Parameter(parameter)) => Some(parameter.erase()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(parameter_references.len(), 6);
        assert!(
            parameter_references
                .iter()
                .all(|parameter| parameter_ids.contains(parameter))
        );

        let mut reordered_parameters = cylinder_parameters();
        reordered_parameters.reverse();
        let reordered = compile_cylinder(&geometry, &reordered_parameters);
        assert!(document.structurally_equivalent(&reordered).unwrap());
        let mut changed_parameters = cylinder_parameters();
        changed_parameters[0].1 = DynQuantity::new(2.0e-3, changed_parameters[0].1.dim());
        let changed = compile_cylinder(&geometry, &changed_parameters);
        assert!(!document.structurally_equivalent(&changed).unwrap());

        assert!(
            ModelDocument::replay(&bytes)
                .unwrap_err()
                .iter()
                .any(|diagnostic| diagnostic.message().contains("requires artifact admission")),
            "resource-free replay must not fabricate Geometry authority",
        );
        for name in [
            "fluid",
            "inlet",
            "outlet",
            "walls",
            "cylinder",
            "definition.velocity",
            "definition.pressure",
        ] {
            assert!(document.aliases().contains_key(name), "missing `{name}`");
        }
    }

    #[test]
    fn typed_external_occurrence_returns_the_common_immutable_model_document() {
        let geometry = fixture_geometry();
        let fluid = geometry.entity_set("fluid").unwrap();
        let supports = [
            ("fluid", fluid, None),
            (
                "inlet",
                geometry.entity_set("inlet").unwrap(),
                Some(("fluid", fluid)),
            ),
            (
                "outlet",
                geometry.entity_set("outlet").unwrap(),
                Some(("fluid", fluid)),
            ),
            (
                "walls",
                geometry.entity_set("walls").unwrap(),
                Some(("fluid", fluid)),
            ),
            (
                "cylinder",
                geometry.entity_set("cylinder").unwrap(),
                Some(("fluid", fluid)),
            ),
        ];
        let document = ModelDocument::compile_external_component(
            "fluid-boundary.eqi",
            SOURCE,
            &geometry,
            "BoundFluid",
            "FluidBoundaryLaw",
            &supports,
            &[("value", DynQuantity::new(2.0, DimExponents::DIMENSIONLESS))],
        )
        .expect("external occurrence reaches common Model admission");

        assert!(
            !document
                .canonical_json()
                .expect("canonical Model")
                .is_empty()
        );
        for name in ["fluid", "inlet", "outlet", "walls", "cylinder"] {
            assert!(document.aliases().contains_key(name), "missing `{name}`");
        }
    }

    #[test]
    fn exact_name_geometry_compilation_selects_one_public_component() {
        let geometry = fixture_geometry();
        let automatic = ModelDocument::compile_with_geometry(
            "fluid-boundary.eqi",
            SOURCE,
            &geometry,
            None,
            &[("value", 2.0)],
        )
        .expect("sole public Component closes automatically");
        let explicit = ModelDocument::compile_with_geometry(
            "renamed-for-diagnostics.eqi",
            SOURCE,
            &geometry,
            Some("FluidBoundaryLaw"),
            &[("value", 2.0)],
        )
        .expect("explicit public Component closes identically");
        assert_eq!(automatic.digest().unwrap(), explicit.digest().unwrap());
        assert!(automatic.structurally_equivalent(&explicit).unwrap());
        ModelDocument::compile_with_geometry(
            "negative-is-not-a-compiler-policy.eqi",
            SOURCE,
            &geometry,
            None,
            &[("value", -2.0)],
        )
        .expect("compiler checks type and finiteness, not application positivity");

        let missing =
            ModelDocument::compile_with_geometry("missing.eqi", SOURCE, &geometry, None, &[])
                .unwrap_err();
        assert!(
            missing
                .iter()
                .any(|error| error.message().contains("value"))
        );
        let extra = ModelDocument::compile_with_geometry(
            "extra.eqi",
            SOURCE,
            &geometry,
            None,
            &[("value", 2.0), ("extra", 1.0)],
        )
        .unwrap_err();
        assert!(extra.iter().any(|error| error.message().contains("extra")));

        let ambiguous = format!(
            "{SOURCE}\n{}",
            SOURCE.replace("FluidBoundaryLaw", "OtherLaw")
        );
        let errors = ModelDocument::compile_with_geometry(
            "ambiguous.eqi",
            &ambiguous,
            &geometry,
            None,
            &[("value", 2.0)],
        )
        .unwrap_err();
        assert!(
            errors
                .iter()
                .any(|error| error.message().contains("component="))
        );
    }

    #[test]
    fn common_geometry_rejects_foreign_kind_and_wrong_parent_bindings() {
        let geometry = fixture_geometry();
        let foreign = fixture_geometry();
        let stale = [("fluid", foreign.entity_set("fluid").unwrap(), None)];
        assert!(
            ModelDocument::compile_external_component(
                "foreign.eqi",
                "not source",
                &geometry,
                "Root",
                "Law",
                &stale,
                &[],
            )
            .unwrap_err()[0]
                .message()
                .contains("foreign or stale")
        );

        let wrong_kind = [("fluid", geometry.entity_set("walls").unwrap(), None)];
        assert!(
            ModelDocument::compile_external_component(
                "wrong-kind.eqi",
                "not source",
                &geometry,
                "Root",
                "Law",
                &wrong_kind,
                &[],
            )
            .unwrap_err()[0]
                .message()
                .contains("selection dimension 1")
        );

        let topology = PlanarRegion::new(
            vec![
                [0.0, 0.0],
                [0.0, 1.0],
                [1.0, 0.0],
                [1.0, 1.0],
                [2.0, 0.0],
                [2.0, 1.0],
                [3.0, 0.0],
                [3.0, 1.0],
            ],
            vec![
                PlanarFace::new(vec![0, 2, 3, 1], Vec::new()),
                PlanarFace::new(vec![4, 6, 7, 5], Vec::new()),
            ],
            vec![
                NamedEntitySet::new("edge-a", EDGE_DIMENSION, vec![0]),
                NamedEntitySet::new("body-a", FACE_DIMENSION, vec![0]),
                NamedEntitySet::new("body-b", FACE_DIMENSION, vec![1]),
            ],
            1.0e-12,
        )
        .unwrap();
        let geometry = CanonicalGeometryV1::from_region(&topology).unwrap();
        let body = geometry.entity_set("body-b").unwrap();
        let wrong_parent = [
            ("body", body, None),
            (
                "wall",
                geometry.entity_set("edge-a").unwrap(),
                Some(("body", body)),
            ),
        ];
        assert!(
            ModelDocument::compile_external_component(
                "wrong-parent.eqi",
                "not source",
                &geometry,
                "Root",
                "Law",
                &wrong_parent,
                &[],
            )
            .unwrap_err()[0]
                .message()
                .contains("does not bind exact parent")
        );
    }
}
