//! Canonical identity of one typed local Eqiora source unit.
//!
//! The identity is derived from parsed semantic syntax, never source bytes or
//! source spans. It seeds deterministic elaboration namespaces independently
//! of whitespace, file location, and declaration traversal order.

use core::fmt;
use std::collections::BTreeMap;

mod domain;
pub(crate) mod formulation;
mod instance;
mod visibility;

use eqiora_core::Diagnostic;
use eqiora_core::diagnostic::codes;
use eqiora_lang::{
    ActivationSyntax, BinaryOp, BoundaryConnectionDecl, BoundaryDecl, BoundaryFamilyBinderSyntax,
    BoundaryPairingSyntax, BoundaryPortReferenceSyntax, BoundaryPortSelectorSyntax,
    BoundarySideSyntax, CartesianCoordinateSyntax, ClockDecl, ComponentDecl, ComponentItem,
    ComponentParameterDecl, ComponentPortDecl, ComponentPortFamilyDecl, ConnectionDecl,
    ConnectionSyntax, ConnectorDecl, ConnectorSyntax, Document, DomainDecl, DomainSyntax, Expr,
    ExprKind, FieldDecl, FieldSlotDecl, FrameSyntax, Item, LetDecl, ModelDecl, NamePath,
    ParameterDecl, PortDecl, PortSyntax, PureOperatorDecl, RelationDecl, RelationFamilyDecl,
    RepresentationDecl, RepresentationSyntax, SignalDirectionSyntax, SupportSlotDecl,
    SupportSlotSyntax, TextRange, UnaryOp, ValueShapeSyntax, VisibilitySyntax,
};
use sha2::{Digest, Sha256};

use crate::connection_sets::{
    ConnectionFragment, ConnectionSetError, ConnectionSetLimits, normalize_connection_sets,
};
use crate::identity::IdentityNamespace;
use crate::pure_operator::compile_definition;
use domain::encode_domain;
use instance::encode_instance;
use visibility::encode_visibility;

const MAGIC: &[u8; 8] = b"EQIORASU";
const CANONICAL_VERSION: u16 = 1;
const COMPONENT_CONNECTION_ITEM_TAG: u16 = 6;
const MODEL_CONNECTION_ITEM_TAG: u16 = 8;
const COMPONENT_PORT_FAMILY_ITEM_TAG: u16 = 11;
const COMPONENT_RELATION_FAMILY_ITEM_TAG: u16 = 12;
const COMPONENT_BOUNDARY_CONNECTION_ITEM_TAG: u16 = 13;
const MODEL_BOUNDARY_CONNECTION_ITEM_TAG: u16 = 11;
const COMPONENT_SPATIAL_PERIODIC_CONNECTION_ITEM_TAG: u16 = 14;
const MODEL_SPATIAL_PERIODIC_CONNECTION_ITEM_TAG: u16 = 12;
const MODEL_LET_ITEM_TAG: u16 = 13;

/// Bounded resource policy for local source-unit identity construction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalSourceIdentityLimits {
    /// Maximum Connector, pure-operator, Component, and Model declarations combined.
    pub max_top_level_declarations: usize,
    /// Maximum declarations in one component or model body.
    pub max_members_per_container: usize,
    /// Maximum declarations summed across all component and model bodies.
    pub max_total_members: usize,
    /// Maximum expression nodes in the complete source unit.
    pub max_expression_nodes: usize,
    /// Maximum recursive expression depth.
    pub max_expression_depth: usize,
    /// Maximum residual roots in one Relation, with root order preserved.
    pub max_residuals_per_relation: usize,
    /// Maximum member paths in one Connection or Boundary declaration.
    pub max_connection_members: usize,
    /// Maximum named Parameter, spatial-support, and Field bindings in one instance.
    pub max_bindings_per_instance: usize,
    /// Maximum exact Boundary members in one complete-exterior set binding.
    pub max_boundary_set_members: usize,
    /// Maximum Boundary-set memberships summed across the source unit.
    pub max_total_boundary_set_memberships: usize,
    /// Maximum segments in one structured source path.
    pub max_path_segments: usize,
    /// Maximum UTF-8 bytes in one name or path segment.
    pub max_name_bytes: usize,
    /// Maximum UTF-8 name bytes summed across the source identity.
    pub max_total_name_bytes: usize,
    /// Maximum bytes in the complete canonical encoding.
    pub max_canonical_bytes: usize,
    /// Maximum bytes cumulatively materialized while canonical records are
    /// encoded for deterministic sorting.
    pub max_intermediate_bytes: usize,
}

impl Default for LocalSourceIdentityLimits {
    fn default() -> Self {
        Self {
            max_top_level_declarations: 65_536,
            max_members_per_container: 65_536,
            max_total_members: 1_000_000,
            max_expression_nodes: 1_000_000,
            max_expression_depth: 256,
            max_residuals_per_relation: 65_536,
            max_connection_members: 65_536,
            max_bindings_per_instance: 65_536,
            max_boundary_set_members: 65_536,
            max_total_boundary_set_memberships: 1_000_000,
            max_path_segments: 256,
            max_name_bytes: 4_096,
            max_total_name_bytes: 64 * 1_024 * 1_024,
            max_canonical_bytes: 128 * 1_024 * 1_024,
            max_intermediate_bytes: 512 * 1_024 * 1_024,
        }
    }
}

/// Domain-separated SHA-256 identity of one typed local source unit.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LocalSourceIdentity([u8; 32]);

impl LocalSourceIdentity {
    /// Compute an identity with the default bounded resource policy.
    pub fn from_document(document: &Document) -> Result<Self, Diagnostic> {
        Self::from_document_with_limits(document, LocalSourceIdentityLimits::default())
    }

    /// Compute an identity with explicit compiler resource limits.
    pub fn from_document_with_limits(
        document: &Document,
        limits: LocalSourceIdentityLimits,
    ) -> Result<Self, Diagnostic> {
        let canonical = canonical_source_bytes(document, limits)?;
        Ok(Self(Sha256::digest(canonical).into()))
    }

    /// Compute a package declaration identity after structurally replacing
    /// direct dependency aliases in type references with exact target
    /// namespace segments.
    pub(crate) fn from_document_with_resolved_aliases(
        document: &Document,
        aliases: &BTreeMap<String, Box<[String]>>,
    ) -> Result<Self, Diagnostic> {
        let canonical = canonical_source_bytes_with_aliases(
            document,
            LocalSourceIdentityLimits::default(),
            aliases.clone(),
        )?;
        Ok(Self(Sha256::digest(canonical).into()))
    }

    /// Exact SHA-256 bytes.
    #[must_use]
    pub const fn digest(&self) -> &[u8; 32] {
        &self.0
    }

    /// Convert this source-unit identity into a reserved deterministic
    /// namespace seed for local elaboration.
    pub fn namespace(&self) -> Result<IdentityNamespace, Diagnostic> {
        let mut digest = String::new();
        digest
            .try_reserve_exact(64)
            .map_err(|_| source_identity_error("cannot reserve source identity namespace"))?;
        const HEX: &[u8; 16] = b"0123456789abcdef";
        for byte in self.0 {
            digest.push(char::from(HEX[usize::from(byte >> 4)]));
            digest.push(char::from(HEX[usize::from(byte & 0x0f)]));
        }
        IdentityNamespace::new(["local-source-v1".to_owned(), digest])
    }
}

impl fmt::Debug for LocalSourceIdentity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "LocalSourceIdentity({self})")
    }
}

impl fmt::Display for LocalSourceIdentity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

fn canonical_source_bytes(
    document: &Document,
    limits: LocalSourceIdentityLimits,
) -> Result<Vec<u8>, Diagnostic> {
    canonical_source_bytes_with_aliases(document, limits, BTreeMap::new())
}

fn canonical_source_bytes_with_aliases(
    document: &Document,
    limits: LocalSourceIdentityLimits,
    resolved_aliases: BTreeMap<String, Box<[String]>>,
) -> Result<Vec<u8>, Diagnostic> {
    let top_level_count = document
        .property_contract_syntax()
        .len()
        .checked_add(document.property_release_syntax().len())
        .and_then(|count| count.checked_add(document.connectors().len()))
        .and_then(|count| count.checked_add(document.pure_operators().len()))
        .and_then(|count| count.checked_add(document.components().len()))
        .and_then(|count| count.checked_add(document.models().len()))
        .ok_or_else(|| source_identity_error("top-level declaration count overflows usize"))?;
    if top_level_count > limits.max_top_level_declarations {
        return Err(source_identity_error(format!(
            "source unit has {top_level_count} top-level declarations, exceeding the {} declaration limit",
            limits.max_top_level_declarations
        )));
    }

    let mut budget = Budget::with_resolved_aliases(limits, resolved_aliases);
    let connectors = encode_sorted_records(document.connectors(), &mut budget, encode_connector)?;
    let property_contract_syntax = document.property_contract_syntax().collect::<Vec<_>>();
    let property_release_syntax = document.property_release_syntax().collect::<Vec<_>>();
    let property_contracts = encode_sorted_records(
        &property_contract_syntax,
        &mut budget,
        encode_property_contract,
    )?;
    let property_releases = encode_sorted_records(
        &property_release_syntax,
        &mut budget,
        encode_property_release,
    )?;
    let pure_operators =
        encode_sorted_records(document.pure_operators(), &mut budget, encode_pure_operator)?;
    let components = encode_sorted_records(document.components(), &mut budget, encode_component)?;
    let models = encode_sorted_records(document.models(), &mut budget, encode_model)?;

    let mut encoder = Encoder::new(limits.max_canonical_bytes);
    encoder.raw(MAGIC)?;
    encoder.u16(CANONICAL_VERSION)?;
    encoder.field(1, |encoder| encoder.records(&connectors))?;
    encoder.field(2, |encoder| encoder.records(&components))?;
    encoder.field(3, |encoder| encoder.records(&models))?;
    if !pure_operators.is_empty() {
        encoder.field(4, |encoder| encoder.records(&pure_operators))?;
    }
    if !property_contracts.is_empty() {
        encoder.field(5, |encoder| encoder.records(&property_contracts))?;
    }
    if !property_releases.is_empty() {
        encoder.field(6, |encoder| encoder.records(&property_releases))?;
    }
    encoder.finish()
}

fn encode_property_contract(
    declaration: &(VisibilitySyntax, &str, &Expr, TextRange),
    budget: &mut Budget,
) -> Result<Vec<u8>, Diagnostic> {
    let (visibility, name, dimension, _) = *declaration;
    let mut encoder = Encoder::new(budget.limits.max_canonical_bytes);
    encoder.field(1, |encoder| encode_name(encoder, name, budget))?;
    encoder.field(2, |encoder| {
        encode_expression(encoder, dimension, budget, 1)
    })?;
    if visibility == VisibilitySyntax::Public {
        encoder.field(3, |encoder| encode_visibility(encoder, visibility))?;
    }
    encoder.finish()
}

fn encode_property_release(
    declaration: &(
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
    budget: &mut Budget,
) -> Result<Vec<u8>, Diagnostic> {
    let (visibility, name, contract, source_value, source_dimension, scale, citation, license, _) =
        *declaration;
    let mut encoder = Encoder::new(budget.limits.max_canonical_bytes);
    encoder.field(1, |encoder| encode_name(encoder, name, budget))?;
    encoder.field(2, |encoder| encode_type_path(encoder, contract, budget))?;
    encoder.field(3, |encoder| {
        encode_expression(encoder, source_value, budget, 1)
    })?;
    encoder.field(4, |encoder| {
        encode_expression(encoder, source_dimension, budget, 1)
    })?;
    encoder.field(5, |encoder| encode_expression(encoder, scale, budget, 1))?;
    encoder.field(6, |encoder| encode_type_path(encoder, citation, budget))?;
    encoder.field(7, |encoder| encode_type_path(encoder, license, budget))?;
    if visibility == VisibilitySyntax::Public {
        encoder.field(8, |encoder| encode_visibility(encoder, visibility))?;
    }
    encoder.finish()
}

fn encode_pure_operator(
    declaration: &PureOperatorDecl,
    budget: &mut Budget,
) -> Result<Vec<u8>, Diagnostic> {
    let definition = compile_definition("<source-identity>", declaration).map_err(|error| {
        source_identity_error(format!(
            "pure operator `{}` has no canonical definition: {}",
            declaration.name(),
            error.message()
        ))
    })?;
    let mut encoder = Encoder::new(budget.limits.max_canonical_bytes);
    encoder.field(1, |encoder| {
        encode_name(encoder, declaration.name(), budget)
    })?;
    encoder.field(2, |encoder| encoder.raw(&definition.digest().bytes()))?;
    if declaration.visibility() == VisibilitySyntax::Public {
        encoder.field(3, |encoder| {
            encode_visibility(encoder, declaration.visibility())
        })?;
    }
    encoder.finish()
}

fn encode_connector(
    declaration: &ConnectorDecl,
    budget: &mut Budget,
) -> Result<Vec<u8>, Diagnostic> {
    let mut encoder = Encoder::new(budget.limits.max_canonical_bytes);
    encoder.field(1, |encoder| {
        encode_name(encoder, declaration.name(), budget)
    })?;
    encoder.field(2, |encoder| match declaration.syntax() {
        ConnectorSyntax::ScalarPhysical {
            across_dimension,
            through_dimension,
        } => {
            encoder.u16(1)?;
            encoder.field(1, |encoder| {
                encode_expression(encoder, across_dimension, budget, 1)
            })?;
            encoder.field(2, |encoder| {
                encode_expression(encoder, through_dimension, budget, 1)
            })
        }
        ConnectorSyntax::FieldPhysical {
            trace,
            flux,
            shape,
            frame,
            pairing,
        } => {
            encoder.u16(2)?;
            encoder.field(1, |encoder| {
                encode_connector_quantity(encoder, trace, budget)
            })?;
            encoder.field(2, |encoder| {
                encode_connector_quantity(encoder, flux, budget)
            })?;
            encoder.field(3, |encoder| encode_value_shape(encoder, shape))?;
            encoder.field(4, |encoder| encode_frame(encoder, *frame))?;
            encoder.field(5, |encoder| encode_boundary_pairing(encoder, *pairing))
        }
        _ => Err(source_identity_error(
            "Connector syntax is newer than source identity v1",
        )),
    })?;
    if declaration.visibility() == VisibilitySyntax::Public {
        encoder.field(3, |encoder| {
            encode_visibility(encoder, declaration.visibility())
        })?;
    }
    encoder.finish()
}

fn encode_component(
    declaration: &ComponentDecl,
    budget: &mut Budget,
) -> Result<Vec<u8>, Diagnostic> {
    let member_count = declaration
        .items()
        .len()
        .checked_add(declaration.property_requirement_syntax().len())
        .ok_or_else(|| source_identity_error("component member count overflows usize"))?;
    budget.account_members(member_count, "component")?;
    let members = encode_container_records(
        declaration.items(),
        budget,
        component_connection,
        encode_component_item,
        COMPONENT_CONNECTION_ITEM_TAG,
    )?;
    let mut encoder = Encoder::new(budget.limits.max_canonical_bytes);
    encoder.field(1, |encoder| {
        encode_name(encoder, declaration.name(), budget)
    })?;
    encoder.field(2, |encoder| encoder.records(&members))?;
    if declaration.visibility() == VisibilitySyntax::Public {
        encoder.field(3, |encoder| {
            encode_visibility(encoder, declaration.visibility())
        })?;
    }
    let property_syntax = declaration
        .property_requirement_syntax()
        .collect::<Vec<_>>();
    if !property_syntax.is_empty() {
        let properties =
            encode_sorted_records(&property_syntax, budget, encode_component_property)?;
        encoder.field(4, |encoder| encoder.records(&properties))?;
    }
    encoder.finish()
}

fn encode_component_property(
    declaration: &(&str, &NamePath, TextRange),
    budget: &mut Budget,
) -> Result<Vec<u8>, Diagnostic> {
    let (name, contract, _) = *declaration;
    let mut encoder = Encoder::new(budget.limits.max_canonical_bytes);
    encoder.field(1, |encoder| encode_name(encoder, name, budget))?;
    encoder.field(2, |encoder| encode_type_path(encoder, contract, budget))?;
    encoder.finish()
}

fn encode_model(declaration: &ModelDecl, budget: &mut Budget) -> Result<Vec<u8>, Diagnostic> {
    budget.account_members(declaration.items().len(), "model")?;
    let members = encode_container_records(
        declaration.items(),
        budget,
        model_connection,
        encode_model_item,
        MODEL_CONNECTION_ITEM_TAG,
    )?;
    let mut encoder = Encoder::new(budget.limits.max_canonical_bytes);
    encoder.field(1, |encoder| {
        encode_name(encoder, declaration.name(), budget)
    })?;
    encoder.field(2, |encoder| encoder.records(&members))?;
    encoder.finish()
}

fn component_connection(item: &ComponentItem) -> Option<&ConnectionDecl> {
    match item {
        ComponentItem::Connection(declaration) => Some(declaration),
        _ => None,
    }
}

fn model_connection(item: &Item) -> Option<&ConnectionDecl> {
    match item {
        Item::Connection(declaration) => Some(declaration),
        _ => None,
    }
}

/// Encode one definition body after replacing its conserving source
/// fragments with their maximal structural connection sets.
///
/// A disjoint fragment encodes exactly as it did before normalization. Signal
/// declarations stay on the ordinary item path because their output is
/// positional and they do not form an undirected equivalence relation.
fn encode_container_records<T>(
    values: &[T],
    budget: &mut Budget,
    connection_of: for<'a> fn(&'a T) -> Option<&'a ConnectionDecl>,
    encode_item: fn(&T, &mut Budget) -> Result<Vec<u8>, Diagnostic>,
    connection_item_tag: u16,
) -> Result<Vec<Vec<u8>>, Diagnostic> {
    let limits = budget.connection_set_limits();
    let mut fragment_count = 0_usize;
    let mut membership_count = 0_usize;
    for value in values {
        let Some(connection) = connection_of(value) else {
            continue;
        };
        if connection.syntax() != ConnectionSyntax::Conserving {
            continue;
        }
        let members = connection.port_paths().len();
        budget.check_connection_members(members, "Connection")?;
        check_connection_set_limit(
            "members in one connection fragment",
            members,
            limits.max_members_per_fragment,
        )?;
        fragment_count = fragment_count.checked_add(1).ok_or_else(|| {
            connection_set_identity_error(
                "preflight",
                ConnectionSetError::CountOverflow {
                    resource: "connection fragments",
                },
            )
        })?;
        membership_count = membership_count.checked_add(members).ok_or_else(|| {
            connection_set_identity_error(
                "preflight",
                ConnectionSetError::CountOverflow {
                    resource: "connection fragment memberships",
                },
            )
        })?;
    }
    check_connection_set_limit("connection fragments", fragment_count, limits.max_fragments)?;
    check_connection_set_limit(
        "connection fragment memberships",
        membership_count,
        limits.max_memberships,
    )?;

    let mut records = Vec::new();
    records
        .try_reserve_exact(values.len())
        .map_err(|_| source_identity_error("cannot reserve canonical source records"))?;
    let mut fragments = Vec::new();
    fragments
        .try_reserve_exact(fragment_count)
        .map_err(|_| source_identity_error("cannot reserve conserving connection fragments"))?;

    for value in values {
        if let Some(connection) = connection_of(value)
            && connection.syntax() == ConnectionSyntax::Conserving
        {
            fragments.push(encode_conserving_fragment(connection, budget, limits)?);
            continue;
        }
        let record = encode_item(value, budget)?;
        budget.account_materialized_bytes(record.len())?;
        records.push(record);
    }

    let normalized = normalize_connection_sets(&fragments, limits)
        .map_err(|error| connection_set_identity_error("normalize", error))?;
    for set in normalized.sets() {
        let record = encode_conserving_connection_item(
            connection_item_tag,
            set.members(),
            budget.limits.max_canonical_bytes,
        )?;
        budget.account_materialized_bytes(record.len())?;
        records.push(record);
    }
    records.sort_unstable();
    Ok(records)
}

fn check_connection_set_limit(
    resource: &'static str,
    observed: usize,
    limit: usize,
) -> Result<(), Diagnostic> {
    if observed > limit {
        Err(connection_set_identity_error(
            "preflight",
            ConnectionSetError::LimitExceeded {
                resource,
                observed,
                limit,
            },
        ))
    } else {
        Ok(())
    }
}

fn encode_conserving_fragment(
    declaration: &ConnectionDecl,
    budget: &mut Budget,
    limits: ConnectionSetLimits,
) -> Result<ConnectionFragment<Vec<u8>>, Diagnostic> {
    budget.check_connection_members(declaration.port_paths().len(), "Connection")?;
    let paths = encode_sorted_paths(declaration.port_paths(), budget)?;
    ConnectionFragment::try_new(paths, limits)
        .map_err(|error| connection_set_identity_error("encode", error))
}

fn encode_conserving_connection_item(
    item_tag: u16,
    canonical_paths: &[Vec<u8>],
    max_canonical_bytes: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut encoder = Encoder::new(max_canonical_bytes);
    encoder.u16(item_tag)?;
    encoder.field(1, |encoder| encoder.u8(2))?;
    encoder.field(2, |encoder| encoder.records(canonical_paths))?;
    encoder.finish()
}

fn connection_set_identity_error(operation: &'static str, error: ConnectionSetError) -> Diagnostic {
    source_identity_error(format!(
        "cannot {operation} conserving source connection sets: {error}"
    ))
}

fn encode_component_item(item: &ComponentItem, budget: &mut Budget) -> Result<Vec<u8>, Diagnostic> {
    let mut encoder = Encoder::new(budget.limits.max_canonical_bytes);
    match item {
        ComponentItem::Parameter(declaration) => {
            encoder.u16(1)?;
            encode_component_parameter(&mut encoder, declaration, budget)?;
        }
        ComponentItem::Port(declaration) => {
            encoder.u16(2)?;
            encode_component_port(&mut encoder, declaration, budget)?;
        }
        ComponentItem::PortFamily(declaration) => {
            encoder.u16(COMPONENT_PORT_FAMILY_ITEM_TAG)?;
            encode_component_port_family(&mut encoder, declaration, budget)?;
        }
        ComponentItem::Field(declaration) => {
            encoder.u16(3)?;
            encode_field(&mut encoder, declaration, budget)?;
        }
        ComponentItem::Clock(declaration) => {
            encoder.u16(4)?;
            encode_clock(&mut encoder, declaration, budget)?;
        }
        ComponentItem::Relation(declaration) => {
            encoder.u16(5)?;
            encode_relation(&mut encoder, declaration, budget)?;
        }
        ComponentItem::RelationFamily(declaration) => {
            encoder.u16(COMPONENT_RELATION_FAMILY_ITEM_TAG)?;
            encode_relation_family(&mut encoder, declaration, budget)?;
        }
        ComponentItem::Connection(declaration) => {
            encoder.u16(COMPONENT_CONNECTION_ITEM_TAG)?;
            encode_connection(&mut encoder, declaration, budget)?;
        }
        ComponentItem::BoundaryConnection(declaration) => {
            encoder.u16(match declaration.syntax() {
                ConnectionSyntax::Conserving => COMPONENT_BOUNDARY_CONNECTION_ITEM_TAG,
                ConnectionSyntax::SpatialPeriodic => COMPONENT_SPATIAL_PERIODIC_CONNECTION_ITEM_TAG,
                ConnectionSyntax::Signal => {
                    return Err(source_identity_error(
                        "boundary Connection cannot use signal semantics",
                    ));
                }
            })?;
            encode_boundary_connection(&mut encoder, declaration, budget)?;
        }
        ComponentItem::Instance(declaration) => {
            encoder.u16(7)?;
            encode_instance(&mut encoder, declaration, budget)?;
        }
        ComponentItem::Representation(declaration) => {
            encoder.u16(8)?;
            encode_representation(&mut encoder, declaration, budget)?;
        }
        ComponentItem::Support(declaration) => {
            encoder.u16(9)?;
            encode_support_slot(&mut encoder, declaration, budget)?;
        }
        ComponentItem::FieldSlot(declaration) => {
            encoder.u16(10)?;
            encode_field_slot(&mut encoder, declaration, budget)?;
        }
        _ => {
            return Err(source_identity_error(
                "component item is newer than source identity v1",
            ));
        }
    }
    encoder.finish()
}

fn encode_model_item(item: &Item, budget: &mut Budget) -> Result<Vec<u8>, Diagnostic> {
    let mut encoder = Encoder::new(budget.limits.max_canonical_bytes);
    match item {
        Item::Domain(declaration) => {
            encoder.u16(1)?;
            encode_domain(&mut encoder, declaration, budget)?;
        }
        Item::Representation(declaration) => {
            encoder.u16(2)?;
            encode_representation(&mut encoder, declaration, budget)?;
        }
        Item::Field(declaration) => {
            encoder.u16(3)?;
            encode_field(&mut encoder, declaration, budget)?;
        }
        Item::Parameter(declaration) => {
            encoder.u16(4)?;
            encode_parameter(&mut encoder, declaration, budget)?;
        }
        Item::Let(declaration) => {
            encoder.u16(MODEL_LET_ITEM_TAG)?;
            encode_let(&mut encoder, declaration, budget)?;
        }
        Item::Port(declaration) => {
            encoder.u16(5)?;
            encode_port(&mut encoder, declaration, budget)?;
        }
        Item::Clock(declaration) => {
            encoder.u16(6)?;
            encode_clock(&mut encoder, declaration, budget)?;
        }
        Item::Relation(declaration) => {
            encoder.u16(7)?;
            encode_relation(&mut encoder, declaration, budget)?;
        }
        Item::Connection(declaration) => {
            encoder.u16(MODEL_CONNECTION_ITEM_TAG)?;
            encode_connection(&mut encoder, declaration, budget)?;
        }
        Item::BoundaryConnection(declaration) => {
            encoder.u16(match declaration.syntax() {
                ConnectionSyntax::Conserving => MODEL_BOUNDARY_CONNECTION_ITEM_TAG,
                ConnectionSyntax::SpatialPeriodic => MODEL_SPATIAL_PERIODIC_CONNECTION_ITEM_TAG,
                ConnectionSyntax::Signal => {
                    return Err(source_identity_error(
                        "boundary Connection cannot use signal semantics",
                    ));
                }
            })?;
            encode_boundary_connection(&mut encoder, declaration, budget)?;
        }
        Item::Boundary(declaration) => {
            encoder.u16(9)?;
            encode_boundary(&mut encoder, declaration, budget)?;
        }
        Item::Instance(declaration) => {
            encoder.u16(10)?;
            encode_instance(&mut encoder, declaration, budget)?;
        }
        _ => {
            return Err(source_identity_error(
                "model item is newer than source identity v1",
            ));
        }
    }
    encoder.finish()
}

fn encode_component_parameter(
    encoder: &mut Encoder,
    declaration: &ComponentParameterDecl,
    budget: &mut Budget,
) -> Result<(), Diagnostic> {
    encoder.field(1, |encoder| {
        encode_visibility(encoder, declaration.visibility())
    })?;
    encoder.field(2, |encoder| {
        encode_name(encoder, declaration.name(), budget)
    })?;
    encoder.field(3, |encoder| {
        encode_expression(encoder, declaration.dimension(), budget, 1)
    })?;
    encoder.field(4, |encoder| match declaration.default() {
        Some(default) => {
            encoder.u8(1)?;
            encoder.field(1, |encoder| encode_expression(encoder, default, budget, 1))
        }
        None => encoder.u8(0),
    })
}

fn encode_component_port(
    encoder: &mut Encoder,
    declaration: &ComponentPortDecl,
    budget: &mut Budget,
) -> Result<(), Diagnostic> {
    encoder.field(1, |encoder| {
        encode_visibility(encoder, declaration.visibility())
    })?;
    encoder.field(2, |encoder| {
        encode_name(encoder, declaration.name(), budget)
    })?;
    encoder.field(3, |encoder| {
        encode_port_syntax(encoder, declaration.syntax(), budget)
    })
}

fn encode_component_port_family(
    encoder: &mut Encoder,
    declaration: &ComponentPortFamilyDecl,
    budget: &mut Budget,
) -> Result<(), Diagnostic> {
    encoder.field(1, |encoder| {
        encode_component_port(encoder, declaration.port(), budget)
    })?;
    encoder.field(2, |encoder| {
        encode_boundary_family_binder(encoder, declaration.binder(), budget)
    })
}

fn encode_support_slot(
    encoder: &mut Encoder,
    declaration: &SupportSlotDecl,
    budget: &mut Budget,
) -> Result<(), Diagnostic> {
    encoder.field(1, |encoder| {
        encode_visibility(encoder, declaration.visibility())
    })?;
    encoder.field(2, |encoder| {
        encode_name(encoder, declaration.name(), budget)
    })?;
    encoder.field(3, |encoder| match declaration.syntax() {
        SupportSlotSyntax::Volume { ambient_dimension } => {
            encoder.u16(1)?;
            encoder.field(1, |encoder| {
                encoder.u64(u64::try_from(*ambient_dimension).map_err(|_| {
                    source_identity_error("support ambient dimension does not fit canonical u64")
                })?)
            })
        }
        SupportSlotSyntax::Boundary { parent } => {
            encoder.u16(2)?;
            encoder.field(1, |encoder| encode_name(encoder, parent, budget))
        }
        SupportSlotSyntax::CompleteExterior { parent } => {
            encoder.u16(3)?;
            encoder.field(1, |encoder| encode_name(encoder, parent, budget))
        }
        _ => Err(source_identity_error(
            "support slot syntax is newer than source identity v1",
        )),
    })
}

fn encode_field_slot(
    encoder: &mut Encoder,
    declaration: &FieldSlotDecl,
    budget: &mut Budget,
) -> Result<(), Diagnostic> {
    encoder.field(1, |encoder| {
        encode_name(encoder, declaration.name(), budget)
    })?;
    encoder.field(2, |encoder| {
        encode_name(encoder, declaration.support(), budget)
    })?;
    encoder.field(3, |encoder| {
        encode_expression(encoder, declaration.dimension(), budget, 1)
    })?;
    encoder.field(4, |encoder| match declaration.shape() {
        Some(shape) => {
            encoder.u8(1)?;
            encode_value_shape(encoder, shape)
        }
        None => encoder.u8(0),
    })
}

fn encode_representation(
    encoder: &mut Encoder,
    declaration: &RepresentationDecl,
    budget: &mut Budget,
) -> Result<(), Diagnostic> {
    encoder.field(1, |encoder| {
        encode_name(encoder, declaration.name(), budget)
    })?;
    encoder.field(2, |encoder| match declaration.syntax() {
        RepresentationSyntax::Continuum => encoder.u16(1),
        _ => Err(source_identity_error(
            "Representation syntax is newer than source identity v1",
        )),
    })
}

fn encode_field(
    encoder: &mut Encoder,
    declaration: &FieldDecl,
    budget: &mut Budget,
) -> Result<(), Diagnostic> {
    encoder.field(1, |encoder| {
        encode_name(encoder, declaration.name(), budget)
    })?;
    encoder.field(2, |encoder| {
        encode_optional_name(encoder, declaration.domain(), budget)
    })?;
    encoder.field(3, |encoder| {
        encode_optional_name(encoder, declaration.representation(), budget)
    })?;
    encoder.field(4, |encoder| {
        encode_expression(encoder, declaration.dimension(), budget, 1)
    })?;
    if let Some(initial) = declaration.initial() {
        encoder.field(5, |encoder| encoder.f64(initial))?;
    }
    if let Some(shape) = declaration.shape().filter(|shape| {
        !matches!(shape, ValueShapeSyntax::Scalar)
            && !matches!(shape, ValueShapeSyntax::Exact(extents) if extents.is_empty())
    }) {
        // Disjoint optional tag: legacy scalar declarations retain their
        // exact v1 bytes and therefore their existing source identity.
        encoder.field(6, |encoder| encode_value_shape(encoder, shape))?;
    }
    Ok(())
}

fn encode_parameter(
    encoder: &mut Encoder,
    declaration: &ParameterDecl,
    budget: &mut Budget,
) -> Result<(), Diagnostic> {
    encoder.field(1, |encoder| {
        encode_name(encoder, declaration.name(), budget)
    })?;
    encoder.field(2, |encoder| {
        encode_expression(encoder, declaration.dimension(), budget, 1)
    })?;
    encoder.field(3, |encoder| encoder.f64(declaration.initial()))
}

fn encode_let(
    encoder: &mut Encoder,
    declaration: &LetDecl,
    budget: &mut Budget,
) -> Result<(), Diagnostic> {
    encoder.field(1, |encoder| {
        encode_name(encoder, declaration.name(), budget)
    })?;
    encoder.field(2, |encoder| {
        encode_expression(encoder, declaration.dimension(), budget, 1)
    })?;
    encoder.field(3, |encoder| {
        encode_expression(encoder, declaration.value(), budget, 1)
    })
}

fn encode_port(
    encoder: &mut Encoder,
    declaration: &PortDecl,
    budget: &mut Budget,
) -> Result<(), Diagnostic> {
    encoder.field(1, |encoder| {
        encode_name(encoder, declaration.name(), budget)
    })?;
    encoder.field(2, |encoder| {
        encode_port_syntax(encoder, declaration.syntax(), budget)
    })
}

fn encode_port_syntax(
    encoder: &mut Encoder,
    syntax: &PortSyntax,
    budget: &mut Budget,
) -> Result<(), Diagnostic> {
    match syntax {
        PortSyntax::Signal {
            direction,
            dimension,
        } => {
            encoder.u16(1)?;
            encoder.field(1, |encoder| {
                encoder.u8(match direction {
                    SignalDirectionSyntax::Input => 1,
                    SignalDirectionSyntax::Output => 2,
                })
            })?;
            encoder.field(2, |encoder| {
                encode_expression(encoder, dimension, budget, 1)
            })
        }
        PortSyntax::ConservingMarker { dimension } => {
            encoder.u16(2)?;
            encoder.field(1, |encoder| {
                encode_expression(encoder, dimension, budget, 1)
            })
        }
        PortSyntax::ScalarPhysical { domain } => {
            encoder.u16(3)?;
            encoder.field(1, |encoder| encode_name(encoder, domain, budget))
        }
        PortSyntax::ScalarPhysicalConnector { connector } => {
            encoder.u16(4)?;
            encoder.field(1, |encoder| encode_type_path(encoder, connector, budget))
        }
        PortSyntax::FieldPhysical { connector, support } => {
            encoder.u16(5)?;
            encoder.field(1, |encoder| encode_type_path(encoder, connector, budget))?;
            encoder.field(2, |encoder| encode_name(encoder, support, budget))
        }
        _ => Err(source_identity_error(
            "Port syntax is newer than source identity v1",
        )),
    }
}

fn encode_connector_quantity(
    encoder: &mut Encoder,
    quantity: &eqiora_lang::ConnectorQuantitySyntax,
    budget: &mut Budget,
) -> Result<(), Diagnostic> {
    encoder.field(1, |encoder| encode_name(encoder, quantity.name(), budget))?;
    encoder.field(2, |encoder| {
        encode_expression(encoder, quantity.dimension(), budget, 1)
    })
}

fn encode_value_shape(encoder: &mut Encoder, shape: &ValueShapeSyntax) -> Result<(), Diagnostic> {
    match shape {
        ValueShapeSyntax::Scalar => encoder.u16(1),
        ValueShapeSyntax::Exact(extents) if extents.is_empty() => encoder.u16(1),
        ValueShapeSyntax::Exact(extents) => {
            encoder.u16(2)?;
            encoder.u32(as_u32(extents.len(), "value-shape rank")?)?;
            for extent in extents {
                encoder.u32(*extent)?;
            }
            Ok(())
        }
        ValueShapeSyntax::SpatialVector => encoder.u16(3),
        _ => Err(source_identity_error(
            "value shape is newer than source identity v1",
        )),
    }
}

fn encode_frame(encoder: &mut Encoder, frame: FrameSyntax) -> Result<(), Diagnostic> {
    match frame {
        FrameSyntax::Invariant => encoder.u8(1),
        FrameSyntax::Spatial => encoder.u8(2),
        _ => Err(source_identity_error(
            "frame syntax is newer than source identity v1",
        )),
    }
}

fn encode_boundary_pairing(
    encoder: &mut Encoder,
    pairing: BoundaryPairingSyntax,
) -> Result<(), Diagnostic> {
    match pairing {
        BoundaryPairingSyntax::EuclideanBoundaryDuality => encoder.u8(1),
        _ => Err(source_identity_error(
            "boundary pairing is newer than source identity v1",
        )),
    }
}

fn encode_clock(
    encoder: &mut Encoder,
    declaration: &ClockDecl,
    budget: &mut Budget,
) -> Result<(), Diagnostic> {
    encoder.field(1, |encoder| {
        encode_name(encoder, declaration.name(), budget)
    })?;
    encoder.field(2, |encoder| {
        encoder.u64(declaration.period().numerator())?;
        encoder.u64(declaration.period().denominator())
    })?;
    encoder.field(3, |encoder| {
        encoder.u64(declaration.phase().numerator())?;
        encoder.u64(declaration.phase().denominator())
    })
}

fn encode_relation(
    encoder: &mut Encoder,
    declaration: &RelationDecl,
    budget: &mut Budget,
) -> Result<(), Diagnostic> {
    if declaration.residuals().len() > budget.limits.max_residuals_per_relation {
        return Err(source_identity_error(format!(
            "Relation `{}` has {} residuals, exceeding the {} residual limit",
            declaration.name(),
            declaration.residuals().len(),
            budget.limits.max_residuals_per_relation
        )));
    }
    encoder.field(1, |encoder| {
        encode_name(encoder, declaration.name(), budget)
    })?;
    encoder.field(2, |encoder| match declaration.activation() {
        ActivationSyntax::Continuous => encoder.u16(1),
        ActivationSyntax::Periodic(clock) => {
            encoder.u16(2)?;
            encoder.field(1, |encoder| encode_name(encoder, clock, budget))
        }
        _ => Err(source_identity_error(
            "Activation syntax is newer than source identity v1",
        )),
    })?;
    encoder.field(3, |encoder| {
        encode_optional_name(encoder, declaration.domain(), budget)
    })?;
    encoder.field(4, |encoder| {
        encoder.u32(as_u32(
            declaration.residuals().len(),
            "Relation residual count",
        )?)?;
        for residual in declaration.residuals() {
            encoder.field(1, |encoder| encode_expression(encoder, residual, budget, 1))?;
        }
        Ok(())
    })
}

fn encode_relation_family(
    encoder: &mut Encoder,
    declaration: &RelationFamilyDecl,
    budget: &mut Budget,
) -> Result<(), Diagnostic> {
    encoder.field(1, |encoder| {
        encode_relation(encoder, declaration.relation(), budget)
    })?;
    encoder.field(2, |encoder| {
        encode_boundary_family_binder(encoder, declaration.binder(), budget)
    })
}

fn encode_connection(
    encoder: &mut Encoder,
    declaration: &ConnectionDecl,
    budget: &mut Budget,
) -> Result<(), Diagnostic> {
    budget.check_connection_members(declaration.port_paths().len(), "Connection")?;
    encoder.field(1, |encoder| {
        encoder.u8(match declaration.syntax() {
            ConnectionSyntax::Signal => 1,
            ConnectionSyntax::Conserving => 2,
            ConnectionSyntax::SpatialPeriodic => 3,
        })
    })?;
    encoder.field(2, |encoder| match declaration.syntax() {
        ConnectionSyntax::Conserving => {
            let paths = encode_sorted_paths(declaration.port_paths(), budget)?;
            encoder.records(&paths)
        }
        ConnectionSyntax::Signal => {
            let Some((output, inputs)) = declaration.port_paths().split_first() else {
                return Err(source_identity_error(
                    "signal Connection has no output member",
                ));
            };
            encoder.field(1, |encoder| encode_path(encoder, output, budget))?;
            let inputs = encode_sorted_paths(inputs, budget)?;
            encoder.field(2, |encoder| encoder.records(&inputs))
        }
        ConnectionSyntax::SpatialPeriodic => {
            let paths = encode_sorted_paths(declaration.port_paths(), budget)?;
            encoder.records(&paths)
        }
    })
}

fn encode_boundary_connection(
    encoder: &mut Encoder,
    declaration: &BoundaryConnectionDecl,
    budget: &mut Budget,
) -> Result<(), Diagnostic> {
    budget.check_connection_members(declaration.ports().len(), "boundary-family Connection")?;
    encoder.field(1, |encoder| match declaration.binder() {
        Some(binder) => {
            encoder.u8(1)?;
            encode_boundary_family_binder(encoder, binder, budget)
        }
        None => encoder.u8(0),
    })?;
    let ports = encode_sorted_records(declaration.ports(), budget, |port, budget| {
        let mut port_encoder = Encoder::new(budget.limits.max_canonical_bytes);
        encode_boundary_port_reference(&mut port_encoder, port, budget)?;
        port_encoder.finish()
    })?;
    encoder.field(2, |encoder| encoder.records(&ports))
}

fn encode_boundary_family_binder(
    encoder: &mut Encoder,
    binder: &BoundaryFamilyBinderSyntax,
    budget: &mut Budget,
) -> Result<(), Diagnostic> {
    encoder.field(1, |encoder| encode_name(encoder, binder.member(), budget))?;
    encoder.field(2, |encoder| encode_name(encoder, binder.set(), budget))
}

fn encode_boundary_port_reference(
    encoder: &mut Encoder,
    reference: &BoundaryPortReferenceSyntax,
    budget: &mut Budget,
) -> Result<(), Diagnostic> {
    encoder.field(1, |encoder| encode_path(encoder, reference.port(), budget))?;
    if let Some(selector) = reference.selector() {
        encoder.field(2, |encoder| {
            encode_boundary_port_selector(encoder, selector, budget)
        })?;
    }
    Ok(())
}

fn encode_boundary_port_selector(
    encoder: &mut Encoder,
    selector: &BoundaryPortSelectorSyntax,
    budget: &mut Budget,
) -> Result<(), Diagnostic> {
    encoder.field(1, |encoder| encode_name(encoder, selector.member(), budget))?;
    encoder.field(2, |encoder| encode_name(encoder, selector.target(), budget))
}

fn encode_boundary(
    encoder: &mut Encoder,
    declaration: &BoundaryDecl,
    budget: &mut Budget,
) -> Result<(), Diagnostic> {
    budget.check_connection_members(declaration.port_paths().len(), "Boundary")?;
    let paths = encode_sorted_paths(declaration.port_paths(), budget)?;
    encoder.field(1, |encoder| encoder.records(&paths))
}

fn encode_optional_name(
    encoder: &mut Encoder,
    name: Option<&str>,
    budget: &mut Budget,
) -> Result<(), Diagnostic> {
    match name {
        Some(name) => {
            encoder.u8(1)?;
            encode_name(encoder, name, budget)
        }
        None => encoder.u8(0),
    }
}

fn encode_name(encoder: &mut Encoder, name: &str, budget: &mut Budget) -> Result<(), Diagnostic> {
    budget.account_name(name)?;
    encoder.string(name)
}

fn encode_path(
    encoder: &mut Encoder,
    path: &NamePath,
    budget: &mut Budget,
) -> Result<(), Diagnostic> {
    let segment_count = path.segments().len();
    if segment_count > budget.limits.max_path_segments {
        return Err(source_identity_error(format!(
            "source path has {segment_count} segments, exceeding the {} segment limit",
            budget.limits.max_path_segments
        )));
    }
    encoder.u32(as_u32(segment_count, "source path segment count")?)?;
    for segment in path.segments() {
        encode_name(encoder, segment, budget)?;
    }
    Ok(())
}

fn encode_type_path(
    encoder: &mut Encoder,
    path: &NamePath,
    budget: &mut Budget,
) -> Result<(), Diagnostic> {
    let segments = path.segments().collect::<Vec<_>>();
    let resolved = match segments.as_slice() {
        [alias, name] => budget
            .resolved_aliases
            .get(*alias)
            .cloned()
            .map(|target| (target, *name)),
        _ => None,
    };
    let Some((target, name)) = resolved else {
        return encode_path(encoder, path, budget);
    };
    let segment_count = target
        .len()
        .checked_add(2)
        .ok_or_else(|| source_identity_error("resolved type path segment count overflows usize"))?;
    if segment_count > budget.limits.max_path_segments {
        return Err(source_identity_error(format!(
            "resolved type path has {segment_count} segments, exceeding the {} segment limit",
            budget.limits.max_path_segments
        )));
    }
    encoder.u32(as_u32(segment_count, "resolved type path segment count")?)?;
    encode_name(encoder, "resolved-package-v1", budget)?;
    for segment in &target {
        encode_name(encoder, segment, budget)?;
    }
    encode_name(encoder, name, budget)
}

fn encode_expression(
    encoder: &mut Encoder,
    expression: &Expr,
    budget: &mut Budget,
    depth: usize,
) -> Result<(), Diagnostic> {
    budget.account_expression(depth)?;
    match expression.kind() {
        ExprKind::Number(value) => {
            encoder.u16(1)?;
            encoder.f64(*value)
        }
        ExprKind::Name(name) => {
            encoder.u16(2)?;
            encoder.field(1, |encoder| encode_name(encoder, name, budget))
        }
        ExprKind::Path(path) => {
            encoder.u16(3)?;
            encoder.field(1, |encoder| encode_path(encoder, path, budget))
        }
        ExprKind::BoundaryPortSelection { port, selector } => {
            encoder.u16(7)?;
            encoder.field(1, |encoder| encode_path(encoder, port, budget))?;
            encoder.field(2, |encoder| {
                encode_boundary_port_selector(encoder, selector, budget)
            })
        }
        ExprKind::Unary { op, value } => {
            if matches!(op, UnaryOp::Neg)
                && matches!(value.kind(), ExprKind::Number(value) if *value == 0.0)
            {
                budget.account_expression(next_depth(depth)?)?;
                encoder.u16(1)?;
                return encoder.f64(0.0);
            }
            encoder.u16(4)?;
            encoder.field(1, |encoder| {
                encoder.u8(match op {
                    UnaryOp::Neg => 1,
                })
            })?;
            let child_depth = next_depth(depth)?;
            encoder.field(2, |encoder| {
                encode_expression(encoder, value, budget, child_depth)
            })
        }
        ExprKind::Binary { op, left, right } => {
            encoder.u16(5)?;
            encoder.field(1, |encoder| {
                encoder.u8(match op {
                    BinaryOp::Add => 1,
                    BinaryOp::Sub => 2,
                    BinaryOp::Mul => 3,
                    BinaryOp::Div => 4,
                    BinaryOp::Pow => 5,
                })
            })?;
            let child_depth = next_depth(depth)?;
            encoder.field(2, |encoder| {
                encode_expression(encoder, left, budget, child_depth)
            })?;
            encoder.field(3, |encoder| {
                encode_expression(encoder, right, budget, child_depth)
            })
        }
        ExprKind::Call { callee, arguments } if !callee.is_qualified() && arguments.len() == 1 => {
            // Byte-for-byte compatibility with source identity v1.
            encoder.u16(6)?;
            encoder.field(1, |encoder| encode_name(encoder, callee.as_str(), budget))?;
            let child_depth = next_depth(depth)?;
            encoder.field(2, |encoder| {
                encode_expression(encoder, &arguments[0], budget, child_depth)
            })
        }
        ExprKind::Call { callee, arguments } => {
            encoder.u16(8)?;
            encoder.field(1, |encoder| encode_type_path(encoder, callee, budget))?;
            let child_depth = next_depth(depth)?;
            let mut encoded = Vec::with_capacity(arguments.len());
            for argument in arguments {
                let mut argument_encoder = Encoder::new(budget.limits.max_canonical_bytes);
                encode_expression(&mut argument_encoder, argument, budget, child_depth)?;
                encoded.push(argument_encoder.finish()?);
            }
            let materialized = encoded.iter().try_fold(0_usize, |total, value| {
                total.checked_add(value.len()).ok_or_else(|| {
                    source_identity_error("call argument encoding bytes overflow usize")
                })
            })?;
            budget.account_materialized_bytes(materialized)?;
            encoder.field(2, |encoder| encoder.records(&encoded))
        }
        _ => Err(source_identity_error(
            "expression syntax is newer than source identity v1",
        )),
    }
}

fn encode_sorted_paths(
    paths: &[NamePath],
    budget: &mut Budget,
) -> Result<Vec<Vec<u8>>, Diagnostic> {
    encode_sorted_records(paths, budget, |path, budget| {
        let mut encoder = Encoder::new(budget.limits.max_canonical_bytes);
        encode_path(&mut encoder, path, budget)?;
        encoder.finish()
    })
}

fn encode_sorted_records<T>(
    values: &[T],
    budget: &mut Budget,
    mut encode: impl FnMut(&T, &mut Budget) -> Result<Vec<u8>, Diagnostic>,
) -> Result<Vec<Vec<u8>>, Diagnostic> {
    let mut records = Vec::new();
    records
        .try_reserve_exact(values.len())
        .map_err(|_| source_identity_error("cannot reserve canonical source records"))?;
    for value in values {
        let record = encode(value, budget)?;
        budget.account_materialized_bytes(record.len())?;
        records.push(record);
    }
    records.sort_unstable();
    Ok(records)
}

struct Budget {
    limits: LocalSourceIdentityLimits,
    total_members: usize,
    expression_nodes: usize,
    total_name_bytes: usize,
    materialized_bytes: usize,
    boundary_set_memberships: usize,
    resolved_aliases: BTreeMap<String, Box<[String]>>,
}

impl Budget {
    const fn new(limits: LocalSourceIdentityLimits) -> Self {
        Self {
            limits,
            total_members: 0,
            expression_nodes: 0,
            total_name_bytes: 0,
            materialized_bytes: 0,
            boundary_set_memberships: 0,
            resolved_aliases: BTreeMap::new(),
        }
    }

    fn with_resolved_aliases(
        limits: LocalSourceIdentityLimits,
        resolved_aliases: BTreeMap<String, Box<[String]>>,
    ) -> Self {
        Self {
            resolved_aliases,
            ..Self::new(limits)
        }
    }

    fn account_members(&mut self, count: usize, container: &'static str) -> Result<(), Diagnostic> {
        if count > self.limits.max_members_per_container {
            return Err(source_identity_error(format!(
                "{container} has {count} members, exceeding the {} member limit",
                self.limits.max_members_per_container
            )));
        }
        self.total_members = self
            .total_members
            .checked_add(count)
            .ok_or_else(|| source_identity_error("source member count overflows usize"))?;
        if self.total_members > self.limits.max_total_members {
            return Err(source_identity_error(format!(
                "source unit exceeds the {} total member limit",
                self.limits.max_total_members
            )));
        }
        Ok(())
    }

    fn check_connection_members(
        &self,
        count: usize,
        label: &'static str,
    ) -> Result<(), Diagnostic> {
        if count > self.limits.max_connection_members {
            return Err(source_identity_error(format!(
                "{label} has {count} members, exceeding the {} member limit",
                self.limits.max_connection_members
            )));
        }
        Ok(())
    }

    fn account_boundary_set_members(&mut self, count: usize) -> Result<(), Diagnostic> {
        if count > self.limits.max_boundary_set_members {
            return Err(source_identity_error(format!(
                "complete-exterior binding has {count} members, exceeding the {} member limit",
                self.limits.max_boundary_set_members
            )));
        }
        self.boundary_set_memberships = self
            .boundary_set_memberships
            .checked_add(count)
            .ok_or_else(|| source_identity_error("BoundarySet membership count overflows usize"))?;
        if self.boundary_set_memberships > self.limits.max_total_boundary_set_memberships {
            return Err(source_identity_error(format!(
                "source unit exceeds the {} total BoundarySet membership limit",
                self.limits.max_total_boundary_set_memberships
            )));
        }
        Ok(())
    }

    fn connection_set_limits(&self) -> ConnectionSetLimits {
        let defaults = ConnectionSetLimits::default();
        let max_memberships = self
            .limits
            .max_members_per_container
            .saturating_mul(self.limits.max_connection_members)
            .min(defaults.max_memberships);
        ConnectionSetLimits {
            max_fragments: self
                .limits
                .max_members_per_container
                .min(defaults.max_fragments),
            max_memberships,
            max_endpoints: max_memberships.min(defaults.max_endpoints),
            // Every maximal set consumes at least one source Connection item,
            // so the existing container budget is also its natural bound.
            max_sets: self.limits.max_members_per_container.min(defaults.max_sets),
            max_members_per_fragment: self
                .limits
                .max_connection_members
                .min(defaults.max_members_per_fragment),
            // A transitive union must not evade the existing per-Connection
            // member policy merely because it was written as small fragments.
            max_members_per_set: self
                .limits
                .max_connection_members
                .min(defaults.max_members_per_set),
        }
    }

    fn account_expression(&mut self, depth: usize) -> Result<(), Diagnostic> {
        if depth > self.limits.max_expression_depth {
            return Err(source_identity_error(format!(
                "expression exceeds the {} level depth limit",
                self.limits.max_expression_depth
            )));
        }
        self.expression_nodes = self
            .expression_nodes
            .checked_add(1)
            .ok_or_else(|| source_identity_error("expression node count overflows usize"))?;
        if self.expression_nodes > self.limits.max_expression_nodes {
            return Err(source_identity_error(format!(
                "source unit exceeds the {} expression node limit",
                self.limits.max_expression_nodes
            )));
        }
        Ok(())
    }

    fn account_name(&mut self, name: &str) -> Result<(), Diagnostic> {
        if name.is_empty() {
            return Err(source_identity_error(
                "source identity name must not be empty",
            ));
        }
        if name.len() > self.limits.max_name_bytes {
            return Err(source_identity_error(format!(
                "source name requires {} bytes, exceeding the {} byte limit",
                name.len(),
                self.limits.max_name_bytes
            )));
        }
        self.total_name_bytes = self
            .total_name_bytes
            .checked_add(name.len())
            .ok_or_else(|| source_identity_error("source name bytes overflow usize"))?;
        if self.total_name_bytes > self.limits.max_total_name_bytes {
            return Err(source_identity_error(format!(
                "source unit exceeds the {} total name byte limit",
                self.limits.max_total_name_bytes
            )));
        }
        Ok(())
    }

    fn account_materialized_bytes(&mut self, count: usize) -> Result<(), Diagnostic> {
        self.materialized_bytes = self
            .materialized_bytes
            .checked_add(count)
            .ok_or_else(|| source_identity_error("intermediate source bytes overflow usize"))?;
        if self.materialized_bytes > self.limits.max_intermediate_bytes {
            return Err(source_identity_error(format!(
                "canonical source sorting exceeds the {} intermediate byte limit",
                self.limits.max_intermediate_bytes
            )));
        }
        Ok(())
    }
}

struct Encoder {
    bytes: Vec<u8>,
    max_bytes: usize,
}

impl Encoder {
    const fn new(max_bytes: usize) -> Self {
        Self {
            bytes: Vec::new(),
            max_bytes,
        }
    }

    fn finish(self) -> Result<Vec<u8>, Diagnostic> {
        if self.bytes.len() > self.max_bytes {
            return Err(source_identity_error(
                "canonical source encoding exceeded its byte limit",
            ));
        }
        Ok(self.bytes)
    }

    fn field(
        &mut self,
        tag: u8,
        encode: impl FnOnce(&mut Self) -> Result<(), Diagnostic>,
    ) -> Result<(), Diagnostic> {
        self.u8(tag)?;
        let length_offset = self.bytes.len();
        self.raw(&[0; 4])?;
        let payload_offset = self.bytes.len();
        encode(self)?;
        let payload_len = self
            .bytes
            .len()
            .checked_sub(payload_offset)
            .ok_or_else(|| source_identity_error("canonical field length underflow"))?;
        self.bytes[length_offset..payload_offset]
            .copy_from_slice(&as_u32(payload_len, "canonical field length")?.to_be_bytes());
        Ok(())
    }

    fn records(&mut self, records: &[Vec<u8>]) -> Result<(), Diagnostic> {
        self.u32(as_u32(records.len(), "canonical record count")?)?;
        for record in records {
            self.u32(as_u32(record.len(), "canonical record length")?)?;
            self.raw(record)?;
        }
        Ok(())
    }

    fn string(&mut self, value: &str) -> Result<(), Diagnostic> {
        self.u32(as_u32(value.len(), "canonical string length")?)?;
        self.raw(value.as_bytes())
    }

    fn f64(&mut self, value: f64) -> Result<(), Diagnostic> {
        if !value.is_finite() {
            return Err(source_identity_error(
                "source identity accepts only finite f64 values",
            ));
        }
        let canonical = if value == 0.0 { 0.0 } else { value };
        self.u64(canonical.to_bits())
    }

    fn u8(&mut self, value: u8) -> Result<(), Diagnostic> {
        self.raw(&[value])
    }

    fn u16(&mut self, value: u16) -> Result<(), Diagnostic> {
        self.raw(&value.to_be_bytes())
    }

    fn u32(&mut self, value: u32) -> Result<(), Diagnostic> {
        self.raw(&value.to_be_bytes())
    }

    fn u64(&mut self, value: u64) -> Result<(), Diagnostic> {
        self.raw(&value.to_be_bytes())
    }

    fn raw(&mut self, value: &[u8]) -> Result<(), Diagnostic> {
        let next_len = self
            .bytes
            .len()
            .checked_add(value.len())
            .ok_or_else(|| source_identity_error("canonical source bytes overflow usize"))?;
        if next_len > self.max_bytes {
            return Err(source_identity_error(format!(
                "canonical source encoding exceeds the {} byte limit",
                self.max_bytes
            )));
        }
        self.bytes
            .try_reserve_exact(value.len())
            .map_err(|_| source_identity_error("cannot reserve canonical source bytes"))?;
        self.bytes.extend_from_slice(value);
        Ok(())
    }
}

fn next_depth(depth: usize) -> Result<usize, Diagnostic> {
    depth
        .checked_add(1)
        .ok_or_else(|| source_identity_error("expression depth overflows usize"))
}

fn as_u32(value: usize, label: &'static str) -> Result<u32, Diagnostic> {
    u32::try_from(value).map_err(|_| source_identity_error(format!("{label} exceeds u32")))
}

fn source_identity_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::LANGUAGE_LOWERING_ERROR, message)
}

#[cfg(test)]
mod tests {
    use eqiora_lang::{format, parse};

    use super::*;

    fn document(source: &str) -> Document {
        parse("fixture.eqi", source).into_document().unwrap()
    }

    fn identity(source: &str) -> LocalSourceIdentity {
        LocalSourceIdentity::from_document(&document(source)).unwrap()
    }

    #[test]
    fn canonical_source_identity_has_a_stable_golden() {
        let document = document("model minimal { parameter gain: 1 = 2; }");
        let canonical =
            canonical_source_bytes(&document, LocalSourceIdentityLimits::default()).unwrap();
        let digest = LocalSourceIdentity::from_document(&document).unwrap();

        assert_eq!(
            canonical,
            vec![
                69, 81, 73, 79, 82, 65, 83, 85, 0, 1, 1, 0, 0, 0, 4, 0, 0, 0, 0, 2, 0, 0, 0, 4, 0,
                0, 0, 0, 3, 0, 0, 0, 80, 0, 0, 0, 1, 0, 0, 0, 72, 1, 0, 0, 0, 11, 0, 0, 0, 7, 109,
                105, 110, 105, 109, 97, 108, 2, 0, 0, 0, 51, 0, 0, 0, 1, 0, 0, 0, 43, 0, 4, 1, 0,
                0, 0, 8, 0, 0, 0, 4, 103, 97, 105, 110, 2, 0, 0, 0, 10, 0, 1, 63, 240, 0, 0, 0, 0,
                0, 0, 3, 0, 0, 0, 8, 64, 0, 0, 0, 0, 0, 0, 0
            ]
        );
        assert_eq!(
            digest.to_string(),
            "dba42a75e6e12596d935fc7161127605c1c769e89a9686e8e93ac4ab150e63a5"
        );
        let namespace = digest.namespace().unwrap();
        assert_eq!(namespace.segments()[0], "local-source-v1");
        assert_eq!(namespace.segments()[1], digest.to_string());
    }

    #[test]
    fn formatting_file_and_span_changes_do_not_change_identity() {
        let compact = "model m{parameter p:1=2;relation r continuous{p-1=0;}}";
        let parsed = document(compact);
        let formatted = format(&parsed);
        let relocated = parse(
            "elsewhere/relocated.eqi",
            &format!("\n\n// shifts every source span\n{formatted}"),
        )
        .into_document()
        .unwrap();

        assert_eq!(
            LocalSourceIdentity::from_document(&parsed).unwrap(),
            LocalSourceIdentity::from_document(&relocated).unwrap()
        );
    }

    #[test]
    fn pure_operator_identity_is_definition_semantic_and_order_independent() {
        let first = r#"
public pure operator outer(left: spatial[1], right: spatial[1]) -> spatial[2]
  = component(left, 0) * component(right, 1);
private pure operator scale(value: scalar) -> scalar
  = rational(2, 1) * component(value);
model M { parameter p: 1 = 1; }
"#;
        let renamed_and_reordered = r#"
private pure operator scale(x: scalar) -> scalar
  = rational(2, 1) * component(x);
public pure operator outer(a: spatial[1], b: spatial[1]) -> spatial[2]
  = component(a, 0) * component(b, 1);
model M { parameter p: 1 = 1; }
"#;
        let changed_body = renamed_and_reordered.replace(
            "component(a, 0) * component(b, 1)",
            "component(b, 0) * component(a, 1)",
        );

        assert_eq!(identity(first), identity(renamed_and_reordered));
        assert_ne!(identity(first), identity(&changed_body));
    }

    #[test]
    fn declaration_binding_and_anonymous_member_permutations_are_invariant() {
        let first = r#"
connector Pin = scalar_physical(across = 1, through = A);
connector Heat = scalar_physical(across = K, through = kg * m ^ 2 / (s ^ 3 * K));
component Pair {
  public parameter resistance: 1 = 2;
  public parameter scale: 1 = 3;
  public port positive: conserving on Pin;
  public port negative: conserving on Pin;
  instance inner: Library.Resistor(resistance = resistance, scale = scale);
  relation law continuous { across(positive) - across(negative) = 0; }
  connect conserving positive, inner.positive, negative;
}
component Empty {}
model circuit {
  instance right: Pair(scale = 4, resistance = 5);
  instance left: Pair(resistance = 2, scale = 3);
  connect conserving left.positive, right.negative, right.positive;
  boundary right.positive, left.negative;
}
model auxiliary {}
"#;
        let permuted = r#"
connector Heat = scalar_physical(across = K, through = kg * m ^ 2 / (s ^ 3 * K));
connector Pin = scalar_physical(across = 1, through = A);
component Empty {}
component Pair {
  connect conserving negative, positive, inner.positive;
  relation law continuous { across(positive) - across(negative) = 0; }
  instance inner: Library.Resistor(scale = scale, resistance = resistance);
  public port negative: conserving on Pin;
  public port positive: conserving on Pin;
  public parameter scale: 1 = 3;
  public parameter resistance: 1 = 2;
}
model auxiliary {}
model circuit {
  boundary left.negative, right.positive;
  connect conserving right.positive, left.positive, right.negative;
  instance left: Pair(scale = 3, resistance = 2);
  instance right: Pair(resistance = 5, scale = 4);
}
"#;

        assert_eq!(identity(first), identity(permuted));
    }

    #[test]
    fn support_declarations_and_bindings_are_canonical_but_exact_targets_are_semantic() {
        let body_first = r#"
component C {
  public support body: volume(ambient_dimension = 2);
  public support wall: boundary(parent = body);
}
model M {
  domain volume = box(0, 1, 0, 1);
  domain left = boundary(volume, axis = 0, side = lower);
  domain right = boundary(volume, axis = 0, side = upper);
  instance c: C(support body = volume, support wall = left);
}
"#;
        let permuted = r#"
component C {
  public support wall: boundary(parent = body);
  public support body: volume(ambient_dimension = 2);
}
model M {
  instance c: C(support wall = left, support body = volume);
  domain right = boundary(volume, axis = 0, side = upper);
  domain left = boundary(volume, axis = 0, side = lower);
  domain volume = box(0, 1, 0, 1);
}
"#;
        let rebound = permuted.replace("support wall = left", "support wall = right");

        assert_eq!(identity(body_first), identity(permuted));
        assert_ne!(identity(body_first), identity(&rebound));
    }

    #[test]
    fn field_slots_and_bindings_are_canonical_but_exact_targets_are_semantic() {
        let slot_first = r#"
component Law {
  public support body: volume(ambient_dimension = 2);
  public field slot displacement on body as continuum: m shape spatial_vector;
  public field slot potential on body as continuum: K;
}
model M {
  domain body = box(0, 1, 0, 1);
  representation space = continuum;
  field displacement on body as space: m shape spatial_vector;
  field potential on body as space: K = 0;
  field other on body as space: K = 0;
  instance law: Law(
    support body = body,
    field displacement = displacement,
    field potential = potential
  );
}
"#;
        let permuted = r#"
component Law {
  public field slot potential on body as continuum: K;
  public field slot displacement on body as continuum: m shape spatial_vector;
  public support body: volume(ambient_dimension = 2);
}
model M {
  instance law: Law(
    field potential = potential,
    field displacement = displacement,
    support body = body
  );
  field other on body as space: K = 0;
  field potential on body as space: K = 0;
  field displacement on body as space: m shape spatial_vector;
  representation space = continuum;
  domain body = box(0, 1, 0, 1);
}
"#;
        let rebound = permuted.replace("field potential = potential", "field potential = other");

        assert_eq!(identity(slot_first), identity(permuted));
        assert_ne!(identity(slot_first), identity(&rebound));
    }

    #[test]
    fn complete_exterior_family_records_are_order_independent_and_exact() {
        let source = r#"
public connector MechanicalBoundary = field_physical(
  trace = displacement: m,
  flux = traction: kg / (m * s ^ 2),
  shape = spatial_vector,
  frame = spatial,
  pairing = euclidean_boundary_duality
);
public component SurfaceLaw {
  public support body: volume(ambient_dimension = 2);
  public support exterior: complete_exterior(parent = body);
  public port mechanical[boundary in exterior]:
    conserving MechanicalBoundary over boundary;
  relation carrier[boundary in exterior] continuous on boundary {
    trace(mechanical[boundary = boundary])
      - trace(mechanical[boundary = boundary]) = 0;
  }
}
model M {
  domain body = box(0, 1, 0, 1);
  domain x_lower = boundary(body, axis = 0, side = lower);
  domain x_upper = boundary(body, axis = 0, side = upper);
  domain y_lower = boundary(body, axis = 1, side = lower);
  domain y_upper = boundary(body, axis = 1, side = upper);
  instance surface: SurfaceLaw(
    support body = body,
    support exterior = boundaries(x_lower, x_upper, y_lower, y_upper)
  );
  connect conserving
    surface.mechanical[boundary = x_lower],
    surface.mechanical[boundary = x_upper];
}
"#;
        let permuted = source
            .replace(
                "boundaries(x_lower, x_upper, y_lower, y_upper)",
                "boundaries(y_upper, x_lower, y_lower, x_upper)",
            )
            .replace(
                "surface.mechanical[boundary = x_lower],\n    surface.mechanical[boundary = x_upper]",
                "surface.mechanical[boundary = x_upper],\n    surface.mechanical[boundary = x_lower]",
            );
        let different_selector = source.replace(
            "surface.mechanical[boundary = x_upper];",
            "surface.mechanical[boundary = y_upper];",
        );

        assert_eq!(identity(source), identity(&permuted));
        assert_ne!(identity(source), identity(&different_selector));
    }

    #[test]
    fn complete_exterior_memberships_have_independent_source_identity_limits() {
        let document = document(
            r#"
component SurfaceLaw {
  public support body: volume(ambient_dimension = 2);
  public support exterior: complete_exterior(parent = body);
}
model M {
  domain body = box(0, 1, 0, 1);
  domain x_lower = boundary(body, axis = 0, side = lower);
  domain x_upper = boundary(body, axis = 0, side = upper);
  domain y_lower = boundary(body, axis = 1, side = lower);
  domain y_upper = boundary(body, axis = 1, side = upper);
  instance surface: SurfaceLaw(
    support body = body,
    support exterior = boundaries(x_lower, x_upper, y_lower, y_upper)
  );
}
"#,
        );
        let per_set = LocalSourceIdentityLimits {
            max_boundary_set_members: 3,
            ..LocalSourceIdentityLimits::default()
        };
        let total = LocalSourceIdentityLimits {
            max_total_boundary_set_memberships: 3,
            ..LocalSourceIdentityLimits::default()
        };

        assert!(LocalSourceIdentity::from_document_with_limits(&document, per_set).is_err());
        assert!(LocalSourceIdentity::from_document_with_limits(&document, total).is_err());
    }

    #[test]
    fn parameter_support_and_field_bindings_share_one_limit() {
        let document = document(
            r#"
component Law {
  public parameter gain: 1;
  public support body: volume(ambient_dimension = 1);
  public field slot state on body as continuum: 1;
}
model M {
  domain body = box(0, 1);
  representation space = continuum;
  field state on body as space: 1 = 0;
  instance law: Law(gain = 1, support body = body, field state = state);
}
"#,
        );
        let limits = LocalSourceIdentityLimits {
            max_bindings_per_instance: 2,
            ..LocalSourceIdentityLimits::default()
        };
        let error = LocalSourceIdentity::from_document_with_limits(&document, limits)
            .expect_err("all three binding families share one checked budget");
        assert!(error.message().contains("3 bindings"));
    }

    #[test]
    fn explicit_and_implicit_scalar_field_shapes_share_identity() {
        let implicit = eqiora_lang::parse(
            "implicit.eqi",
            "model M { field x: 1 = 0; relation r continuous { x = 0; } }",
        )
        .into_document()
        .unwrap();
        let explicit = eqiora_lang::parse(
            "explicit.eqi",
            "model M { field x: 1 shape [] = 0; relation r continuous { x = 0; } }",
        )
        .into_document()
        .unwrap();
        assert_eq!(
            LocalSourceIdentity::from_document(&implicit).unwrap(),
            LocalSourceIdentity::from_document(&explicit).unwrap()
        );
    }

    #[test]
    fn field_connector_alias_spelling_resolves_to_exact_package_identity() {
        let source = |alias: &str| {
            eqiora_lang::parse(
                "alias.eqi",
                &format!(
                    "public component Side {{ public support body: volume(ambient_dimension = 2); public support wall: boundary(parent = body); public port p: conserving {alias}.Boundary over wall; }}"
                ),
            )
            .into_document()
            .unwrap()
        };
        let aliases = |alias: &str| {
            BTreeMap::from([(
                alias.to_owned(),
                vec!["org.example".to_owned(), "mechanics".to_owned()].into_boxed_slice(),
            )])
        };
        assert_eq!(
            LocalSourceIdentity::from_document_with_resolved_aliases(
                &source("short"),
                &aliases("short"),
            )
            .unwrap(),
            LocalSourceIdentity::from_document_with_resolved_aliases(
                &source("renamed"),
                &aliases("renamed"),
            )
            .unwrap(),
        );
    }

    #[test]
    fn conserving_fragments_have_definition_local_equivalence_identity() {
        let component_nary = "component C { connect conserving a, b, c; } model Empty {}";
        let component_chain =
            "component C { connect conserving a, b; connect conserving b, c; } model Empty {}";
        assert_eq!(identity(component_nary), identity(component_chain));

        let model_nary = "model M { connect conserving a, b, c; }";
        let model_chain = "model M { connect conserving a, b; connect conserving b, c; }";
        assert_eq!(identity(model_nary), identity(model_chain));
    }

    #[test]
    fn duplicate_conserving_fragments_are_identity_idempotent() {
        let component_once = "component C { connect conserving a, b; } model Empty {}";
        let component_twice =
            "component C { connect conserving a, b; connect conserving b, a; } model Empty {}";
        assert_eq!(identity(component_once), identity(component_twice));

        let model_once = "model M { connect conserving a, b; }";
        let model_twice = "model M { connect conserving a, b; connect conserving b, a; }";
        assert_eq!(identity(model_once), identity(model_twice));
    }

    #[test]
    fn spatial_periodic_pair_identity_is_endpoint_order_invariant_but_not_conserving() {
        let lower_upper = "model M { connect periodic lower.p, upper.p; }";
        let upper_lower = "model M { connect periodic upper.p, lower.p; }";
        let conserving = "model M { connect conserving lower.p, upper.p; }";

        assert_eq!(identity(lower_upper), identity(upper_lower));
        assert_ne!(identity(lower_upper), identity(conserving));
    }

    #[test]
    fn disjoint_conserving_and_signal_records_keep_the_legacy_bytes() {
        let document = document(
            "component C { connect conserving a, b; connect signal out -> in_b, in_a; connect conserving c, d; } \
             model M { connect conserving w, x; connect signal source -> sink_b, sink_a; connect conserving y, z; }",
        );
        let limits = LocalSourceIdentityLimits::default();

        let mut component_legacy_budget = Budget::new(limits);
        let component_legacy = encode_sorted_records(
            document.components()[0].items(),
            &mut component_legacy_budget,
            encode_component_item,
        )
        .unwrap();
        let mut component_normalized_budget = Budget::new(limits);
        let component_normalized = encode_container_records(
            document.components()[0].items(),
            &mut component_normalized_budget,
            component_connection,
            encode_component_item,
            COMPONENT_CONNECTION_ITEM_TAG,
        )
        .unwrap();
        assert_eq!(component_normalized, component_legacy);

        let mut model_legacy_budget = Budget::new(limits);
        let model_legacy = encode_sorted_records(
            document.models()[0].items(),
            &mut model_legacy_budget,
            encode_model_item,
        )
        .unwrap();
        let mut model_normalized_budget = Budget::new(limits);
        let model_normalized = encode_container_records(
            document.models()[0].items(),
            &mut model_normalized_budget,
            model_connection,
            encode_model_item,
            MODEL_CONNECTION_ITEM_TAG,
        )
        .unwrap();
        assert_eq!(model_normalized, model_legacy);
    }

    #[test]
    fn normalized_conserving_sets_cannot_evade_connection_member_limits() {
        let document = document(
            "component C { connect conserving a, b; connect conserving b, c; } model Empty {}",
        );
        let limits = LocalSourceIdentityLimits {
            max_connection_members: 2,
            ..LocalSourceIdentityLimits::default()
        };
        let diagnostic = LocalSourceIdentity::from_document_with_limits(&document, limits)
            .expect_err("the transitive set has three members");

        assert!(
            diagnostic
                .message()
                .contains("members in one normalized connection set")
        );
    }

    #[test]
    fn signal_output_is_positional_but_inputs_are_canonical_members() {
        let base = "model m { port o: signal output 1; port a: signal input 1; port b: signal input 1; connect signal o -> a, b; }";
        let inputs_permuted = "model m { port o: signal output 1; port a: signal input 1; port b: signal input 1; connect signal o -> b, a; }";
        let different_output = "model m { port o: signal output 1; port a: signal input 1; port b: signal input 1; connect signal a -> o, b; }";

        assert_eq!(identity(base), identity(inputs_permuted));
        assert_ne!(identity(base), identity(different_output));
    }

    #[test]
    fn semantic_structure_and_exact_values_change_identity() {
        let base = "model m { parameter p: 1 = 2; relation r continuous { p + 1 = 0; } }";
        let changed_value = "model m { parameter p: 1 = 3; relation r continuous { p + 1 = 0; } }";
        let changed_operator =
            "model m { parameter p: 1 = 2; relation r continuous { p - 1 = 0; } }";
        let changed_activation = "model m { clock c = periodic(period = 1/1, phase = 0/1); parameter p: 1 = 2; relation r periodic(c) { p + 1 = 0; } }";

        assert_ne!(identity(base), identity(changed_value));
        assert_ne!(identity(base), identity(changed_operator));
        assert_ne!(identity(base), identity(changed_activation));
    }

    #[test]
    fn let_alias_source_structure_has_exact_identity() {
        let base = "model m { parameter p: m = 2; let k: 1 / m = math.pi / p; }";
        let reformatted = "model m {\n parameter p: m = 2;\n let k: 1/m = math.pi/p;\n}";
        let renamed = "model m { parameter p: m = 2; let wave: 1 / m = math.pi / p; }";
        let changed = "model m { parameter p: m = 2; let k: 1 / m = 2 / p; }";

        assert_eq!(identity(base), identity(reformatted));
        assert_ne!(identity(base), identity(renamed));
        assert_ne!(identity(base), identity(changed));
    }

    #[test]
    fn canonical_identity_is_structural_not_algebraic_equivalence() {
        let folded = "model m { parameter p: 1 = 2; relation r continuous { p + 2 = 0; } }";
        let unfolded = "model m { parameter p: 1 = 2; relation r continuous { p + (1 + 1) = 0; } }";
        let multiplied_dimension =
            "model m { parameter area: m * m = 1; relation r continuous { area = 0; } }";
        let powered_dimension =
            "model m { parameter area: m ^ 2 = 1; relation r continuous { area = 0; } }";

        assert_ne!(identity(folded), identity(unfolded));
        assert_ne!(identity(multiplied_dimension), identity(powered_dimension));
    }

    #[test]
    fn interface_visibility_defaults_ports_bindings_and_domains_are_semantic() {
        let public_default = "component C { public parameter p: 1 = 2; public port s: signal input 1; } model m { instance x: C(p = 2); }";
        let private_default = "component C { parameter p: 1 = 2; public port s: signal input 1; } model m { instance x: C(p = 2); }";
        let required = "component C { public parameter p: 1; public port s: signal input 1; } model m { instance x: C(p = 2); }";
        let output_port = "component C { public parameter p: 1 = 2; public port s: signal output 1; } model m { instance x: C(p = 2); }";
        let changed_binding = "component C { public parameter p: 1 = 2; public port s: signal input 1; } model m { instance x: C(p = 3); }";
        assert_ne!(identity(public_default), identity(private_default));
        assert_ne!(identity(public_default), identity(required));
        assert_ne!(identity(public_default), identity(output_port));
        assert_ne!(identity(public_default), identity(changed_binding));

        let on_domain = "model m { domain d = box(0, 1); relation r continuous on d { 1 = 0; } }";
        let without_domain = "model m { domain d = box(0, 1); relation r continuous { 1 = 0; } }";
        assert_ne!(identity(on_domain), identity(without_domain));
    }

    #[test]
    fn package_visibility_is_semantic_and_private_is_the_canonical_default() {
        let private =
            "connector Pin = scalar_physical(across = 1, through = A); component Resistor {}";
        let explicit_private = "private component Resistor {} private connector Pin = scalar_physical(across = 1, through = A);";
        let public_connector = "component Resistor {} public connector Pin = scalar_physical(across = 1, through = A);";
        let public_component = "public component Resistor {} connector Pin = scalar_physical(across = 1, through = A);";

        assert_eq!(identity(private), identity(explicit_private));
        assert_ne!(identity(private), identity(public_connector));
        assert_ne!(identity(private), identity(public_component));
        assert_ne!(identity(public_connector), identity(public_component));

        let formatted = format(&document(public_connector));
        assert_eq!(
            identity(public_connector),
            LocalSourceIdentity::from_document(
                &parse(
                    "elsewhere/library.eqi",
                    &format!("// relocated\n{formatted}")
                )
                .into_document()
                .unwrap(),
            )
            .unwrap()
        );
    }

    #[test]
    fn residual_root_order_is_preserved() {
        let first = "model m { parameter a: 1 = 1; parameter b: 1 = 2; relation r continuous { a = 0; b = 0; } }";
        let reversed = "model m { parameter a: 1 = 1; parameter b: 1 = 2; relation r continuous { b = 0; a = 0; } }";

        assert_ne!(identity(first), identity(reversed));
    }

    #[test]
    fn negative_zero_is_normalized_to_positive_zero() {
        let positive = "model m { parameter p: 1 = 0; }";
        let negative = "model m { parameter p: 1 = -0; }";

        assert_eq!(identity(positive), identity(negative));
    }

    #[test]
    fn negative_zero_has_one_source_transaction_and_model_meaning() {
        let positive = "connector Pin = scalar_physical(across = 1, through = 1); model m { domain d = box(0, 1); representation space = continuum; field x on d as space: 1 = 0; parameter p: 1 = 0; relation r continuous on d { x + p + 0 = 0; } }";
        let negative = "connector Pin = scalar_physical(across = 1, through = 1); model m { domain d = box(-0, 1); representation space = continuum; field x on d as space: 1 = -0; parameter p: 1 = -0; relation r continuous on d { x + p + -0 = 0; } }";

        assert_eq!(identity(positive), identity(negative));
        let mut positive = crate::compile("zero.eqi", positive).unwrap();
        let mut negative = crate::compile("zero.eqi", negative).unwrap();
        let positive = positive.remove(0);
        let negative = negative.remove(0);
        assert_eq!(positive.model(), negative.model());
        assert_eq!(positive.transaction().ops(), negative.transaction().ops());
    }

    #[test]
    fn cartesian_coordinate_sources_preserve_exact_root_declaration_identity() {
        let parameter = "model m { parameter extent: m = 2; parameter other: m = 2; domain body = box(-1, extent, extent, 6); relation r continuous on body { coordinate(0) - coordinate(0) = 0; } }";
        let declarations_permuted = "model m { domain body = box(-1, extent, extent, 6); relation r continuous on body { coordinate(0) - coordinate(0) = 0; } parameter other: m = 2; parameter extent: m = 2; }";
        let fixed = "model m { parameter extent: m = 2; parameter other: m = 2; domain body = box(-1, 2, 2, 6); relation r continuous on body { coordinate(0) - coordinate(0) = 0; } }";
        let other_root = "model m { parameter extent: m = 2; parameter other: m = 2; domain body = box(-1, other, other, 6); relation r continuous on body { coordinate(0) - coordinate(0) = 0; } }";

        assert_eq!(identity(parameter), identity(declarations_permuted));
        assert_ne!(identity(parameter), identity(fixed));
        assert_ne!(identity(parameter), identity(other_root));
    }

    #[test]
    fn resource_limits_fail_closed() {
        let base_document =
            document("model m { parameter p: 1 = 2; relation r continuous { p + 1 = 0; } }");
        let top_level = LocalSourceIdentityLimits {
            max_top_level_declarations: 0,
            ..LocalSourceIdentityLimits::default()
        };
        assert!(LocalSourceIdentity::from_document_with_limits(&base_document, top_level).is_err());

        let expressions = LocalSourceIdentityLimits {
            max_expression_nodes: 1,
            ..LocalSourceIdentityLimits::default()
        };
        assert!(
            LocalSourceIdentity::from_document_with_limits(&base_document, expressions).is_err()
        );

        let bytes = LocalSourceIdentityLimits {
            max_canonical_bytes: 8,
            ..LocalSourceIdentityLimits::default()
        };
        assert!(LocalSourceIdentity::from_document_with_limits(&base_document, bytes).is_err());

        let intermediate = LocalSourceIdentityLimits {
            max_intermediate_bytes: 1,
            ..LocalSourceIdentityLimits::default()
        };
        assert!(
            LocalSourceIdentity::from_document_with_limits(&base_document, intermediate).is_err()
        );

        let mixed_bindings = document(
            "component C { public parameter p: 1; public support d: volume(ambient_dimension = 1); } \
             model M { domain d = box(0, 1); instance c: C(p = 1, support d = d); }",
        );
        let bindings = LocalSourceIdentityLimits {
            max_bindings_per_instance: 1,
            ..LocalSourceIdentityLimits::default()
        };
        assert!(LocalSourceIdentity::from_document_with_limits(&mixed_bindings, bindings).is_err());
    }
}
