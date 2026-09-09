//! Bare Kernel artifacts have no authored presentation metadata.
use super::*;
use crate::identity::{
    DeclarationPath, ElaborationKey, IdentityNamespace, InstancePath, ModelViewKey,
};
use eqiora_schema::kernel::KernelNode;

impl ModelNotation {
    /// Project identity-only labels for a complete validated Kernel Model.
    /// Source names, family selectors and removed exposures cannot be recovered
    /// from Kernel bytes. Recompile a source package to retain those sidecars.
    #[must_use]
    pub fn from_kernel<'a>(
        model: eqiora_core::OntologyId<eqiora_schema::Model>,
        nodes: impl IntoIterator<Item = &'a KernelNode>,
    ) -> Self {
        let namespace = IdentityNamespace::new(["kernel-notation".to_owned(), model.to_string()])
            .expect("bounded Model ID namespace");
        let path = InstancePath::new(["model"]).expect("fixed root path");
        let scope = ModelViewKey::new(namespace.clone(), path.clone())
            .and_then(|key| key.full_identity())
            .expect("bounded Model scope key");
        let mut entries = BTreeMap::new();
        for node in nodes {
            let roles: &[QuantityRole] = match node {
                KernelNode::Field(_) | KernelNode::Parameter(_) => &[QuantityRole::Value],
                KernelNode::Port(port) if port.physical_domain().is_some() => {
                    &[QuantityRole::Across, QuantityRole::Through]
                }
                KernelNode::Port(port) if port.boundary_physical_contract().is_some() => {
                    &[QuantityRole::Trace, QuantityRole::Flux]
                }
                KernelNode::Port(_) => &[QuantityRole::Value],
                _ => continue,
            };
            for role in roles {
                let raw = node.id();
                let identity = QuantityIdentity {
                    scope,
                    occurrence: ElaborationKey::entity(
                        namespace.clone(),
                        path.clone(),
                        DeclarationPath::new([raw.to_string()])
                            .expect("bounded Kernel declaration ID"),
                        raw.kind(),
                    )
                    .and_then(|key| key.full_identity())
                    .expect("bounded Kernel occurrence key"),
                    role: *role,
                    member: None,
                };
                let selector = format!("{raw}.{}", role.name());
                // Render exact original IDs, not a shortened hash or invented name.
                let exact = format!("{model}:{raw}:{}", role.name());
                entries.insert(
                    identity,
                    ResolvedNotation {
                        identity,
                        selector,
                        graph_id: Some(raw),
                        definition: None,
                        instance: None,
                        label: NotationLabel::identifier(&exact)
                            .expect("bounded Kernel IDs and closed role"),
                    },
                );
            }
        }
        Self { entries }
    }
}
