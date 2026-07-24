//! Occurrence-free closure and boundary-partition proof for scalar physical
//! Component interfaces.
//!
//! Relation ownership remains a linear slot. Source Connections instead
//! contribute typed fragments of an equivalence relation, so overlapping
//! scalar physical fragments are normalized before final membership is
//! checked. A Component exports only the partition induced on its own public
//! boundary. Immediate child obligations remain fail-closed unless a parent
//! fragment handles them explicitly.

use std::collections::{BTreeMap, BTreeSet};

use eqiora_core::diagnostic::codes;
use eqiora_lang::TextRange;
use eqiora_schema::kernel::physical_closure::{
    PhysicalClosureViolation, PhysicalEndpointSlots, PhysicalSlot,
};

use crate::connection_sets::{ConnectionFragment, ConnectionSetError, normalize_connection_sets};
use crate::diagnostics::{BoundedDiagnostics, source_error};

use super::body_check::{
    ChildInstanceProof, DefinitionBodyProof, DefinitionBodyProofs, ResolvedPhysicalEndpoint,
};
use super::definition_graph::CheckedDefinitionGraph;
use super::preflight::DefinitionKey;

#[derive(Clone, Debug, Default)]
struct ComponentInterface {
    public_endpoints: BTreeMap<String, PhysicalEndpointSlots>,
    /// Exact equivalence classes induced on this Component's public boundary.
    /// Open and internally isolated public Ports appear as singleton classes.
    public_partition: Vec<Box<[String]>>,
}

#[derive(Clone, Copy)]
enum EndpointOrigin {
    Local { public: bool, range: TextRange },
    Child { instance_range: TextRange },
}

impl EndpointOrigin {
    const fn range(self) -> TextRange {
        match self {
            Self::Local { range, .. } => range,
            Self::Child { instance_range } => instance_range,
        }
    }
}

struct EndpointState {
    slots: PhysicalEndpointSlots,
    origin: EndpointOrigin,
}

struct DefinitionPartition {
    sets: Vec<Box<[ResolvedPhysicalEndpoint]>>,
}

/// Validate physical closure after the definition-DAG proof has established
/// finite acyclic traversal.
pub(super) fn validate(
    checked_graph: &CheckedDefinitionGraph,
    body_proofs: &DefinitionBodyProofs,
    diagnostics: &mut BoundedDiagnostics,
) {
    let mut interfaces = BTreeMap::<DefinitionKey, Option<ComponentInterface>>::new();
    for key in checked_graph.component_order() {
        let Some(proof) = body_proofs.components.get(key) else {
            interfaces.insert(key.clone(), None);
            continue;
        };
        if proof
            .children
            .values()
            .any(|child| !matches!(interfaces.get(&child.definition), Some(Some(_))))
        {
            interfaces.insert(key.clone(), None);
            continue;
        }
        let interface = validate_component(proof, &interfaces, diagnostics);
        interfaces.insert(key.clone(), interface);
    }

    for proof in body_proofs.models.values() {
        if proof
            .children
            .values()
            .all(|child| matches!(interfaces.get(&child.definition), Some(Some(_))))
        {
            validate_model(proof, &interfaces, diagnostics);
        }
    }
}

fn validate_component(
    proof: &DefinitionBodyProof,
    interfaces: &BTreeMap<DefinitionKey, Option<ComponentInterface>>,
    diagnostics: &mut BoundedDiagnostics,
) -> Option<ComponentInterface> {
    let mut endpoints = definition_endpoints(proof, interfaces);
    let mut valid = true;
    for relation in &proof.relation_endpoints {
        valid &= fill_owner_selections(relation, &proof.file, &mut endpoints, diagnostics);
    }
    let partition = normalize_definition_partition(proof, interfaces, diagnostics)?;
    mark_partition_membership(&partition, &mut endpoints);
    let public_partition = public_boundary_partition(&partition, &endpoints);

    let mut public_endpoints = BTreeMap::new();
    for (key, endpoint) in endpoints {
        match endpoint.origin {
            EndpointOrigin::Local { public: true, .. } => {
                let ResolvedPhysicalEndpoint::Local(name) = key else {
                    unreachable!("only a local declaration can be a public parent interface");
                };
                public_endpoints.insert(name, endpoint.slots);
            }
            EndpointOrigin::Local {
                public: false,
                range,
            } => {
                valid &= require_closed(
                    &proof.file,
                    range,
                    "private physical Port",
                    &key.display(),
                    endpoint.slots,
                    diagnostics,
                );
            }
            EndpointOrigin::Child { instance_range } => {
                valid &= require_closed_or_projectable_exposure(
                    &proof.file,
                    instance_range,
                    "child public physical Port",
                    &key.display(),
                    endpoint.slots,
                    diagnostics,
                );
            }
        }
    }
    valid.then_some(ComponentInterface {
        public_endpoints,
        public_partition,
    })
}

fn validate_model(
    proof: &DefinitionBodyProof,
    interfaces: &BTreeMap<DefinitionKey, Option<ComponentInterface>>,
    diagnostics: &mut BoundedDiagnostics,
) {
    let mut endpoints = definition_endpoints(proof, interfaces);
    for relation in &proof.relation_endpoints {
        fill_owner_selections(relation, &proof.file, &mut endpoints, diagnostics);
    }
    let Some(partition) = normalize_definition_partition(proof, interfaces, diagnostics) else {
        return;
    };
    mark_partition_membership(&partition, &mut endpoints);
    mark_deferred_memberships(proof, &mut endpoints, diagnostics);
    for (key, endpoint) in endpoints {
        match endpoint.origin {
            EndpointOrigin::Local { range, .. } => {
                require_closed(
                    &proof.file,
                    range,
                    "Model physical endpoint",
                    &key.display(),
                    endpoint.slots,
                    diagnostics,
                );
            }
            EndpointOrigin::Child { instance_range } => {
                require_closed_or_projectable_exposure(
                    &proof.file,
                    instance_range,
                    "Model child public physical Port",
                    &key.display(),
                    endpoint.slots,
                    diagnostics,
                );
            }
        }
    }
}

fn mark_deferred_memberships(
    proof: &DefinitionBodyProof,
    endpoints: &mut BTreeMap<ResolvedPhysicalEndpoint, EndpointState>,
    diagnostics: &mut BoundedDiagnostics,
) {
    for member in &proof.deferred_connection_memberships {
        let Some(endpoint) = endpoints.get_mut(member) else {
            diagnostics.push(source_error(
                codes::LANGUAGE_LOWERING_ERROR,
                &proof.file,
                proof.range,
                format!(
                    "typed exact-boundary Connection selected unavailable physical endpoint `{}`",
                    member.display()
                ),
            ));
            continue;
        };
        if endpoint.slots.membership() == PhysicalSlot::Open
            && let Err(violation) = endpoint.slots.fill_membership()
        {
            diagnostics.push(closure_error(
                &proof.file,
                endpoint.origin.range(),
                &member.display(),
                violation,
            ));
        }
    }
}

fn normalize_definition_partition(
    proof: &DefinitionBodyProof,
    interfaces: &BTreeMap<DefinitionKey, Option<ComponentInterface>>,
    diagnostics: &mut BoundedDiagnostics,
) -> Option<DefinitionPartition> {
    let limits = proof.connection_limits;
    let mut fragments = proof.physical_connection_fragments.clone();
    for (instance, child) in &proof.children {
        let interface = interfaces
            .get(&child.definition)
            .and_then(Option::as_ref)
            .expect("caller suppresses closure when a child body has no proof");
        for class in &interface.public_partition {
            if class.len() < 2 {
                continue;
            }
            let members = class.iter().map(|port| ResolvedPhysicalEndpoint::Child {
                instance: instance.clone(),
                port: port.clone(),
            });
            match ConnectionFragment::try_new(members, limits) {
                Ok(fragment) => fragments.push(fragment),
                Err(error) => {
                    diagnostics.push(partition_error(
                        &proof.file,
                        child.range,
                        "child boundary partition",
                        error,
                    ));
                    return None;
                }
            }
        }
    }
    let normalized = match normalize_connection_sets(&fragments, limits) {
        Ok(normalized) => normalized,
        Err(error) => {
            diagnostics.push(partition_error(
                &proof.file,
                proof.range,
                "definition physical partition",
                error,
            ));
            return None;
        }
    };
    Some(DefinitionPartition {
        sets: normalized
            .sets()
            .iter()
            .map(|set| set.members().to_vec().into_boxed_slice())
            .collect(),
    })
}

fn mark_partition_membership(
    partition: &DefinitionPartition,
    endpoints: &mut BTreeMap<ResolvedPhysicalEndpoint, EndpointState>,
) {
    for member in partition.sets.iter().flat_map(|set| set.iter()) {
        let endpoint = endpoints
            .get_mut(member)
            .expect("typed fragments select indexed physical endpoints");
        if endpoint.slots.membership() == PhysicalSlot::Open {
            endpoint
                .slots
                .fill_membership()
                .expect("an open membership slot can be filled once");
        }
    }
}

fn public_boundary_partition(
    partition: &DefinitionPartition,
    endpoints: &BTreeMap<ResolvedPhysicalEndpoint, EndpointState>,
) -> Vec<Box<[String]>> {
    let mut classes = Vec::<Box<[String]>>::new();
    let mut classified = BTreeSet::<String>::new();
    for set in &partition.sets {
        let class = set
            .iter()
            .filter_map(
                |member| match endpoints.get(member).map(|endpoint| endpoint.origin) {
                    Some(EndpointOrigin::Local { public: true, .. }) => match member {
                        ResolvedPhysicalEndpoint::Local(name) => Some(name.clone()),
                        ResolvedPhysicalEndpoint::Child { .. } => {
                            unreachable!("a public local origin has a local endpoint key")
                        }
                    },
                    _ => None,
                },
            )
            .collect::<Vec<_>>();
        if !class.is_empty() {
            classified.extend(class.iter().cloned());
            classes.push(class.into_boxed_slice());
        }
    }
    for (member, endpoint) in endpoints {
        let (ResolvedPhysicalEndpoint::Local(name), EndpointOrigin::Local { public: true, .. }) =
            (member, endpoint.origin)
        else {
            continue;
        };
        if !classified.contains(name) {
            classes.push(vec![name.clone()].into_boxed_slice());
        }
    }
    classes.sort_unstable();
    classes
}

fn definition_endpoints(
    proof: &DefinitionBodyProof,
    interfaces: &BTreeMap<DefinitionKey, Option<ComponentInterface>>,
) -> BTreeMap<ResolvedPhysicalEndpoint, EndpointState> {
    let mut endpoints = BTreeMap::new();
    for (name, local) in &proof.local_physical_ports {
        endpoints.insert(
            ResolvedPhysicalEndpoint::Local(name.clone()),
            EndpointState {
                slots: PhysicalEndpointSlots::open(),
                origin: EndpointOrigin::Local {
                    public: local.public,
                    range: local.range,
                },
            },
        );
    }
    for (instance, child) in &proof.children {
        insert_child_endpoints(instance, child, interfaces, &mut endpoints);
    }
    endpoints
}

fn insert_child_endpoints(
    instance: &str,
    child: &ChildInstanceProof,
    interfaces: &BTreeMap<DefinitionKey, Option<ComponentInterface>>,
    endpoints: &mut BTreeMap<ResolvedPhysicalEndpoint, EndpointState>,
) {
    let interface = interfaces
        .get(&child.definition)
        .and_then(Option::as_ref)
        .expect("caller suppresses closure when a child body has no proof");
    for (port, slots) in &interface.public_endpoints {
        endpoints.insert(
            ResolvedPhysicalEndpoint::Child {
                instance: instance.to_owned(),
                port: port.clone(),
            },
            EndpointState {
                slots: *slots,
                origin: EndpointOrigin::Child {
                    instance_range: child.range,
                },
            },
        );
    }
}

fn fill_owner_selections(
    selections: &super::body_check::PhysicalEndpointSelections,
    file: &str,
    endpoints: &mut BTreeMap<ResolvedPhysicalEndpoint, EndpointState>,
    diagnostics: &mut BoundedDiagnostics,
) -> bool {
    let mut valid = true;
    for key in selections {
        valid &= fill_owner(key, file, endpoints, diagnostics);
    }
    valid
}

fn fill_owner(
    key: &ResolvedPhysicalEndpoint,
    file: &str,
    endpoints: &mut BTreeMap<ResolvedPhysicalEndpoint, EndpointState>,
    diagnostics: &mut BoundedDiagnostics,
) -> bool {
    let endpoint = endpoints
        .get_mut(key)
        .expect("typed body proof selects an indexed physical endpoint");
    let range = endpoint.origin.range();
    let result = endpoint.slots.fill_owner();
    if let Err(violation) = result {
        diagnostics.push(closure_error(file, range, &key.display(), violation));
        false
    } else {
        true
    }
}

fn partition_error(
    file: &str,
    range: TextRange,
    subject: &str,
    error: ConnectionSetError,
) -> eqiora_core::Diagnostic {
    source_error(
        codes::LANGUAGE_LOWERING_ERROR,
        file,
        range,
        format!("cannot normalize {subject}: {error}"),
    )
}

fn require_closed(
    file: &str,
    range: TextRange,
    subject: &str,
    display: &str,
    slots: PhysicalEndpointSlots,
    diagnostics: &mut BoundedDiagnostics,
) -> bool {
    let mut closed = true;
    if slots.owner() == PhysicalSlot::Open {
        closed = false;
        diagnostics.push(source_error(
            codes::LANGUAGE_TYPE_ERROR,
            file,
            range,
            format!("{subject} `{display}` requires exactly one owning Relation"),
        ));
    }
    if slots.membership() == PhysicalSlot::Open {
        closed = false;
        diagnostics.push(source_error(
            codes::LANGUAGE_TYPE_ERROR,
            file,
            range,
            format!("{subject} `{display}` requires exactly one conserving Connection membership"),
        ));
    }
    debug_assert!(
        slots.require_closed().is_ok()
            || slots.owner() == PhysicalSlot::Open
            || slots.membership() == PhysicalSlot::Open
    );
    closed
}

/// Admit exactly the one open shape that occurrence lowering can erase.
///
/// A public child Port with no constitutive owner is not retained as a Kernel
/// unknown when it participates in a typed physical set; it is a source-level
/// exposure of that set. The root occurrence proof still has to find the
/// complete set, at least two retained owned members, and one explicit LCA
/// fragment before graph mutation. Every other open child shape fails here.
fn require_closed_or_projectable_exposure(
    file: &str,
    range: TextRange,
    subject: &str,
    display: &str,
    slots: PhysicalEndpointSlots,
    diagnostics: &mut BoundedDiagnostics,
) -> bool {
    if slots.owner() == PhysicalSlot::Open && slots.membership() == PhysicalSlot::Filled {
        true
    } else {
        require_closed(file, range, subject, display, slots, diagnostics)
    }
}

fn closure_error(
    file: &str,
    range: TextRange,
    endpoint: &str,
    violation: PhysicalClosureViolation,
) -> eqiora_core::Diagnostic {
    let message = match violation {
        PhysicalClosureViolation::MultipleOwners => {
            format!("physical endpoint `{endpoint}` cannot have more than one owning Relation")
        }
        PhysicalClosureViolation::MultipleMemberships => format!(
            "physical endpoint `{endpoint}` cannot belong to more than one conserving Connection"
        ),
        PhysicalClosureViolation::MissingOwner | PhysicalClosureViolation::MissingMembership => {
            unreachable!("slot filling reports only cardinality overflow")
        }
    };
    source_error(codes::LANGUAGE_TYPE_ERROR, file, range, message)
}

#[cfg(test)]
mod tests {
    use eqiora_lang::parse;

    use crate::source_identity::LocalSourceIdentity;

    use crate::hierarchy::preflight::Elaborator;
    use crate::hierarchy::{HierarchyLimits, check};

    fn validate_source(source: &str) -> Vec<eqiora_core::Diagnostic> {
        validate_source_with_limits(source, HierarchyLimits::default())
    }

    fn validate_source_with_limits(
        source: &str,
        limits: HierarchyLimits,
    ) -> Vec<eqiora_core::Diagnostic> {
        let document = parse("physical-closure.eqi", source)
            .into_compilation_document()
            .expect("fixture parses");
        let identity = LocalSourceIdentity::from_document(&document).expect("source identity");
        let elaborator = Elaborator::new(
            "physical-closure.eqi",
            source.len(),
            &document,
            identity,
            limits,
        )
        .expect("definition scopes resolve");
        match check::validate(&elaborator) {
            Ok(_) => Vec::new(),
            Err(diagnostics) => diagnostics,
        }
    }

    const PIN: &str = "connector Pin = scalar_physical(across = 1, through = 1);";

    #[test]
    fn public_primitive_exports_owner_and_membership_independently() {
        for body in [
            "",
            "relation owner continuous { across(p) = 0; }",
            "public port q: conserving on Pin; connect conserving p, q;",
            "public port q: conserving on Pin; relation owner continuous { across(p) = 0; } connect conserving p, q;",
        ] {
            let source = format!(
                "{PIN} component Primitive {{ public port p: conserving on Pin; {body} }} model Empty {{}}"
            );
            assert!(
                validate_source(&source).is_empty(),
                "all four public slot states are valid: {body}"
            );
        }

        let deduplicated = format!(
            "{PIN} component Primitive {{ public port p: conserving on Pin; relation owner continuous {{ across(p) + through(p) = 0; }} }} model Empty {{}}"
        );
        assert!(
            validate_source(&deduplicated).is_empty(),
            "across and through in one Relation fill one owner slot"
        );
    }

    #[test]
    fn unused_private_endpoint_and_double_owner_fail_with_source_spans() {
        let unused =
            format!("{PIN} component Broken {{ port p: conserving on Pin; }} model Empty {{}}");
        let diagnostics = validate_source(&unused);
        assert_eq!(diagnostics.len(), 2);
        assert!(
            diagnostics
                .iter()
                .all(|error| error.source_span().is_some())
        );

        let double = format!(
            "{PIN} component Broken {{ public port p: conserving on Pin; relation a continuous {{ across(p) = 0; }} relation b continuous {{ through(p) = 0; }} }} model Empty {{}}"
        );
        let diagnostics = validate_source(&double);
        assert!(diagnostics.iter().any(|error| {
            error.message().contains("more than one owning Relation")
                && error.source_span().is_some()
        }));
    }

    #[test]
    fn model_closes_every_child_slot_and_reports_each_unclosed_endpoint() {
        let all_open = format!(
            "{PIN} component Pair {{ public port a: conserving on Pin; public port b: conserving on Pin; }} model AllOpen {{ instance pair: Pair; }}"
        );
        let diagnostics = validate_source(&all_open);
        assert_eq!(diagnostics.len(), 4, "both slots remain open on both Ports");
        assert!(
            diagnostics
                .iter()
                .all(|error| error.source_span().is_some())
        );

        let one_open = format!(
            "{PIN} component Triple {{ public port a: conserving on Pin; public port b: conserving on Pin; public port c: conserving on Pin; }} model OneOpen {{ instance triple: Triple; relation owner continuous {{ across(triple.a) + across(triple.b) + across(triple.c) = 0; }} connect conserving triple.a, triple.b; }}"
        );
        let diagnostics = validate_source(&one_open);
        assert_eq!(diagnostics.len(), 1);
        assert!(
            diagnostics[0].message().contains("triple.c")
                && diagnostics[0]
                    .message()
                    .contains("conserving Connection membership")
        );
        assert!(diagnostics[0].source_span().is_some());
    }

    #[test]
    fn scalar_physical_fragments_overlap_but_relation_owners_remain_linear() {
        let transitive = r#"
model Network {
  domain electrical = scalar_physical(across = 1, through = 1);
  port a: conserving on electrical;
  port b: conserving on electrical;
  port c: conserving on electrical;
  relation owners continuous { across(a) + across(b) + across(c) = 0; }
  connect conserving a, b;
  connect conserving b, c;
}
"#;
        assert!(
            validate_source(transitive).is_empty(),
            "overlapping typed physical fragments form one membership set"
        );

        let double_owner = format!(
            "{PIN} component Broken {{ public port p: conserving on Pin; relation a continuous {{ across(p) = 0; }} relation b continuous {{ through(p) = 0; }} public port q: conserving on Pin; connect conserving p, q; }} model Empty {{}}"
        );
        let diagnostics = validate_source(&double_owner);
        assert!(diagnostics.iter().any(|error| {
            error.message().contains("more than one owning Relation")
                && error.source_span().is_some()
        }));
    }

    #[test]
    fn child_boundary_partitions_compose_and_remain_extensible() {
        let valid = format!(
            "{PIN} component Leaf {{ public port a: conserving on Pin; public port b: conserving on Pin; }} component Closed {{ instance leaf: Leaf; relation law continuous {{ across(leaf.a) + across(leaf.b) = 0; }} connect conserving leaf.a, leaf.b; }} model Empty {{}}"
        );
        assert!(validate_source(&valid).is_empty());

        let repeated_fragment = format!(
            "{PIN} component Leaf {{ public port a: conserving on Pin; public port b: conserving on Pin; relation law continuous {{ across(a) + across(b) = 0; }} connect conserving a, b; }} component Reconnect {{ instance leaf: Leaf; connect conserving leaf.a, leaf.b; }} model Empty {{}}"
        );
        assert!(
            validate_source(&repeated_fragment).is_empty(),
            "equivalent physical fragments are idempotent topology claims"
        );

        let transitive_child_partition = format!(
            "{PIN} component Leaf {{ public port a: conserving on Pin; public port b: conserving on Pin; relation owners continuous {{ across(a) + across(b) = 0; }} connect conserving a, b; }} component Wrapper {{ public port left: conserving on Pin; public port right: conserving on Pin; instance leaf: Leaf; relation owners continuous {{ across(left) + across(right) = 0; }} connect conserving left, leaf.a; connect conserving leaf.b, right; }} model Use {{ instance wrapper: Wrapper; }}"
        );
        assert!(
            validate_source(&transitive_child_partition).is_empty(),
            "a child boundary class joins distinct parent fragments"
        );

        let two_level_forwarding = format!(
            "{PIN} component Leaf {{ public port p: conserving on Pin; relation owner continuous {{ across(p) = 0; }} }} component Middle {{ public port p: conserving on Pin; relation owner continuous {{ across(p) = 0; }} instance leaf: Leaf; connect conserving p, leaf.p; }} component Outer {{ public port p: conserving on Pin; relation owner continuous {{ across(p) = 0; }} instance middle: Middle; connect conserving p, middle.p; }} model Use {{ instance outer: Outer; }}"
        );
        assert!(
            validate_source(&two_level_forwarding).is_empty(),
            "an explicitly forwarded child interface can be extended again"
        );
    }

    #[test]
    fn ownerless_child_port_is_only_deferred_as_an_explicit_exposure() {
        let forwarded = format!(
            "{PIN} component Leaf {{ public port p: conserving on Pin; relation law continuous {{ across(p) = 0; }} }} component Wrapper {{ public port p: conserving on Pin; instance leaf: Leaf; connect conserving p, leaf.p; }} model Use {{ instance left: Wrapper; instance right: Leaf; connect conserving left.p, right.p; }}"
        );
        assert!(
            validate_source(&forwarded).is_empty(),
            "the ownerless wrapper Port is a typed exposure candidate whose final occurrence set is proved later"
        );

        let unconnected = format!(
            "{PIN} component Open {{ public port p: conserving on Pin; }} model Use {{ instance open: Open; }}"
        );
        let diagnostics = validate_source(&unconnected);
        assert_eq!(diagnostics.len(), 2, "{diagnostics:?}");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message().contains("exactly one owning Relation"))
        );
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message()
                .contains("conserving Connection membership")
        }));
    }

    #[test]
    fn signal_marker_and_nominal_mismatch_rules_fail_before_partitioning() {
        for (name, source, expected) in [
            (
                "signal membership",
                "model M { port out: signal output 1; port a: signal input 1; port b: signal input 1; connect signal out -> a; connect signal out -> b; }",
                "already belongs to another Connection",
            ),
            (
                "marker membership",
                "model M { port a: conserving 1; port b: conserving 1; port c: conserving 1; connect conserving a, b; connect conserving a, c; }",
                "already belongs to another Connection",
            ),
            (
                "nominal physical type",
                "connector A = scalar_physical(across = 1, through = 1); connector B = scalar_physical(across = 1, through = 1); component C { public port a: conserving on A; public port b: conserving on B; connect conserving a, b; } model M {}",
                "exact same nominal Connector or Domain",
            ),
        ] {
            let diagnostics = validate_source(source);
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message().contains(expected)),
                "{name}: {diagnostics:?}"
            );
            assert!(
                diagnostics
                    .iter()
                    .all(|diagnostic| diagnostic.source_span().is_some()),
                "{name}: {diagnostics:?}"
            );
        }
    }

    #[test]
    fn definition_partition_uses_the_hierarchy_resource_policy() {
        let source = r#"
model Network {
  domain electrical = scalar_physical(across = 1, through = 1);
  port a: conserving on electrical;
  port b: conserving on electrical;
  port c: conserving on electrical;
  relation owners continuous { across(a) + across(b) + across(c) = 0; }
  connect conserving a, b;
  connect conserving b, c;
}
"#;
        let diagnostics = validate_source_with_limits(
            source,
            HierarchyLimits {
                max_connections: 1,
                ..HierarchyLimits::default()
            },
        );
        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert!(
            diagnostics[0]
                .message()
                .contains("requires 2 Connections, exceeding the 1 limit"),
            "{diagnostics:?}"
        );
        assert!(diagnostics[0].source_span().is_some());
    }

    #[test]
    fn invalid_body_or_child_closure_does_not_cascade() {
        let invalid_body = format!(
            "{PIN} component Broken {{ public port p: conserving on Pin; relation bad continuous {{ across(p) + missing = 0; }} }} model Use {{ instance broken: Broken; }}"
        );
        let diagnostics = validate_source(&invalid_body);
        assert_eq!(diagnostics.len(), 1);
        assert!(
            diagnostics[0]
                .message()
                .contains("unresolved expression symbol")
        );

        let invalid_parent = format!(
            "{PIN} component Leaf {{ public port p: conserving on Pin; }} component Parent {{ instance leaf: Leaf; }} model Use {{ instance parent: Parent; }}"
        );
        let diagnostics = validate_source(&invalid_parent);
        assert_eq!(diagnostics.len(), 2);
        assert!(diagnostics.iter().all(|error| {
            error
                .message()
                .contains("child public physical Port `leaf.p`")
                && !error.message().contains("parent")
        }));
    }
}
