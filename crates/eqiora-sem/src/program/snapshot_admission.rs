//! Snapshot selection and whole-program admission orchestration.

use std::collections::BTreeMap;

use eqiora_core::diagnostic::codes;
use eqiora_core::{Diagnostic, OntologyId};
use eqiora_geometry::CanonicalGeometryRef;
use eqiora_graph::Snapshot;
use eqiora_schema::Model;

use super::geometry_admission::{admit_entity_sets, index_closed_bundle};
use super::spatial_domains::{
    cartesian_spatial_supports, validate_domains, validate_fields, validate_geometry_support_uses,
};
use super::{
    KernelProgram, kernel_error, kernel_path, model_path, validate_activations,
    validate_closed_topology, validate_connections, validate_relations,
};
use crate::conserving::validate_scalar_physical_networks;

impl KernelProgram {
    /// Select a `ModelView` from an immutable snapshot and validate all
    /// cross-node invariants required for execution.
    ///
    /// Validation is diagnostic-accumulating: independent faults are returned
    /// together in deterministic graph order.
    ///
    /// # Errors
    /// Returns diagnostics when the view is absent, its semantic topology is
    /// not closed, a symbol is unresolved, dimensions conflict, activation or
    /// clock wiring is ambiguous, or a connection contract is invalid.
    pub fn from_snapshot(
        snapshot: &Snapshot,
        model: OntologyId<Model>,
    ) -> Result<Self, Vec<Diagnostic>> {
        Self::from_snapshot_impl(snapshot, model, None)
    }

    /// Select and validate a Model together with its exact closed canonical
    /// geometry bundle.
    ///
    /// Artifact order is irrelevant. Every distinct geometry digest named by
    /// the Model must be supplied exactly once, and no unreferenced artifact is
    /// accepted. The returned program retains only internally derived spatial
    /// support facts, never artifact bytes or borrowed references.
    ///
    /// # Errors
    /// Returns diagnostics for the ordinary model invariants, an inexact
    /// artifact bundle, invalid entity-set selections, or unsupported geometry
    /// use.
    pub fn from_snapshot_with_geometry(
        snapshot: &Snapshot,
        model: OntologyId<Model>,
        geometry: &[CanonicalGeometryRef<'_>],
    ) -> Result<Self, Vec<Diagnostic>> {
        Self::from_snapshot_impl(snapshot, model, Some(geometry))
    }

    fn from_snapshot_impl(
        snapshot: &Snapshot,
        model: OntologyId<Model>,
        geometry: Option<&[CanonicalGeometryRef<'_>]>,
    ) -> Result<Self, Vec<Diagnostic>> {
        let raw_model = model.erase();
        let Some(view) = snapshot.ontology_view(&raw_model) else {
            return Err(vec![
                Diagnostic::error(
                    codes::ONTOLOGY_VIEW_NOT_FOUND,
                    format!("ModelView {raw_model} does not exist at the selected revision"),
                )
                .with_graph_path(model_path(model)),
            ]);
        };

        let mut diagnostics = Vec::new();
        let mut nodes = BTreeMap::new();
        let mut values = BTreeMap::new();
        for &member in view.members() {
            match snapshot.node(member) {
                Some(node) => match node.kernel_definition() {
                    Some(definition) => {
                        nodes.insert(member, definition.clone());
                        if let Some(value) = node.value() {
                            values.insert(member, value);
                        }
                    }
                    None => diagnostics.push(kernel_error(
                        member,
                        "ModelView member has no complete Semantic Kernel definition",
                    )),
                },
                None => diagnostics.push(
                    Diagnostic::error(
                        codes::NODE_NOT_FOUND,
                        format!("ModelView member {member} is absent from the snapshot"),
                    )
                    .with_graph_path(kernel_path(member)),
                ),
            }
        }

        let edges = snapshot
            .edges()
            .filter(|edge| {
                view.members().contains(&edge.from()) && view.members().contains(&edge.to())
            })
            .copied()
            .collect::<Vec<_>>();

        validate_closed_topology(snapshot, view.members(), &mut diagnostics);
        let invalid_domains = validate_domains(&nodes, &edges, &mut diagnostics);
        let mut spatial_supports = cartesian_spatial_supports(&nodes, &edges);
        let artifacts_admitted = geometry.is_some();
        if let Some(geometry) = geometry {
            let artifacts = match index_closed_bundle(&nodes, geometry) {
                Ok(artifacts) => artifacts,
                Err(bundle_faults) => {
                    diagnostics.extend(bundle_faults);
                    return Err(diagnostics);
                }
            };
            let admission = admit_entity_sets(&nodes, &edges, &invalid_domains, &artifacts);
            diagnostics.extend(admission.diagnostics);
            spatial_supports.extend(admission.supports);
        }
        validate_geometry_support_uses(&nodes, &edges, artifacts_admitted, &mut diagnostics);
        validate_fields(&nodes, &edges, &spatial_supports, &mut diagnostics);
        validate_relations(&nodes, &edges, &spatial_supports, &mut diagnostics);
        validate_activations(&nodes, &edges, &spatial_supports, &mut diagnostics);
        validate_connections(&nodes, &edges, &mut diagnostics);
        crate::boundary_physical::validate_networks(&nodes, &edges, &mut diagnostics);
        validate_scalar_physical_networks(&nodes, &edges, view.boundary(), &mut diagnostics);

        if diagnostics.is_empty() {
            Ok(Self {
                revision: snapshot.revision(),
                model,
                nodes,
                values,
                edges,
                boundary: view.boundary().clone(),
                spatial_supports,
            })
        } else {
            Err(diagnostics)
        }
    }
}
