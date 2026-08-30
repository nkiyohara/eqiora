use std::collections::{BTreeMap, BTreeSet};

use eqiora_core::diagnostic::codes;
use eqiora_core::{Diagnostic, EntityKind, GraphPath, ValueShape};
use eqiora_lang::{
    BoundaryConnectionDecl, BoundaryPairingSyntax, ComponentItem, ComponentPortDecl,
    ComponentPortFamilyDecl, ConnectionDecl, ConnectionSyntax, ConnectorSyntax, DomainSyntax,
    FrameSyntax, InstanceDecl, Item, PortSyntax, VisibilitySyntax,
};
use eqiora_schema::kernel::typing::SpatialSupport;
use eqiora_schema::kernel::{
    BoundaryPairing, BoundaryPhysicalConnector, BoundaryPhysicalPortContract, BoundarySide,
    CartesianBoundaryEmbedding, ValueFrame, validate_boundary_physical_connection,
};

use crate::connection_sets::ConnectionFragment;
use crate::diagnostics::source_error;
use crate::dimensions::lower_dimension;
use crate::identity::{
    DeclarationPath, ElaborationIdentityLimits, ElaborationKey, FullElaborationIdentity,
    GeneratedRole, IdentityNamespace, InstancePath, ModelViewKey,
};
use crate::lower::{LoweringDomainContract, LoweringExpression, LoweringPortContract};

use super::body_check::field_expression_type;
use super::complete_exterior::CartesianDomain;
use super::field_slots::{FieldContract, component_field_interface, resolve_instance_fields};
use super::occurrence_connections::{
    OccurrenceConnectionFragment, OccurrencePhysicalEndpoint, normalize_occurrence_connections,
};
use super::{exposure_cuts::ExposureCutIndex, hierarchy_error};

use super::flat::{
    ConnectionIdentity, DisplayIdentity, EntityIdentity, EntitySourceOrigin, ExpandedBlueprint,
    FlatItemBlueprint, PhysicalExposureContractIdentity, PhysicalExposureProjectionBlueprint,
    RelationIdentity, SourceLocation,
};

mod cartesian;
mod external;
use super::parameters::{
    ConstantValue, ParameterLineage, ParameterResolver, ResolvedParameter, normalize_zero,
};
use super::preflight::{
    ComponentDefinition, ConnectorDefinition, DefinitionKey, DefinitionNamespace, Elaborator,
    ExpansionSize, ModelDefinition,
};
use super::scope::{
    ActiveBoundaryMember, FlatSymbol, InstanceInterface, Scope, SymbolKind,
    resolve_boundary_port_reference, resolve_local_kind, resolve_ports, resolve_visible_ports,
    rewrite_expression_with_boundary_member, rewrite_field_scope, rewrite_model_port,
    rewrite_relation,
};
use super::supports::{
    CompleteExteriorMembershipBudget, ResolvedBoundaryTarget, ResolvedSupportBindings,
    component_support_interface, resolve_instance_support_bindings,
};

fn instance_binding_locations(file: &str, instance: &InstanceDecl) -> Vec<SourceLocation> {
    let mut ranges = instance
        .bindings()
        .iter()
        .map(|binding| binding.range())
        .chain(
            instance
                .support_bindings()
                .iter()
                .map(|binding| binding.range()),
        )
        .chain(
            instance
                .boundary_set_bindings()
                .iter()
                .map(|binding| binding.range()),
        )
        .chain(
            instance
                .field_bindings()
                .iter()
                .map(|binding| binding.range()),
        )
        .collect::<Vec<_>>();
    ranges.sort_by_key(|range| (range.start(), range.end()));
    ranges
        .into_iter()
        .map(|range| SourceLocation::new(file, range))
        .collect()
}

fn field_forwarding_locations(file: &str, instance: &InstanceDecl) -> Vec<SourceLocation> {
    if instance.field_bindings().is_empty() {
        return Vec::new();
    }
    let mut ranges = instance
        .support_bindings()
        .iter()
        .map(|binding| binding.range())
        .chain(
            instance
                .field_bindings()
                .iter()
                .map(|binding| binding.range()),
        )
        .collect::<Vec<_>>();
    ranges.sort_by_key(|range| (range.start(), range.end()));
    ranges
        .into_iter()
        .map(|range| SourceLocation::new(file, range))
        .collect()
}

fn parameter_forwarding_locations(file: &str, instance: &InstanceDecl) -> Vec<SourceLocation> {
    let mut ranges = instance
        .bindings()
        .iter()
        .map(|binding| binding.range())
        .collect::<Vec<_>>();
    ranges.sort_by_key(|range| (range.start(), range.end()));
    ranges
        .into_iter()
        .map(|range| SourceLocation::new(file, range))
        .collect()
}

fn boundary_set_forwarding_locations(
    file: &str,
    instance: &InstanceDecl,
    support_bindings: &ResolvedSupportBindings<FullElaborationIdentity>,
) -> Vec<SourceLocation> {
    if support_bindings.boundary_sets().next().is_none() {
        return Vec::new();
    }
    let mut ranges = instance
        .support_bindings()
        .iter()
        .map(|binding| binding.range())
        .chain(
            support_bindings
                .boundary_sets()
                .map(|(_, set)| set.source_range()),
        )
        .collect::<Vec<_>>();
    ranges.sort_by_key(|range| (range.start(), range.end()));
    ranges
        .into_iter()
        .map(|range| SourceLocation::new(file, range))
        .collect()
}

fn normalize_binding_locations(bindings: &mut Vec<SourceLocation>) {
    bindings.sort_by(|left, right| {
        left.file
            .cmp(&right.file)
            .then_with(|| left.range.start().cmp(&right.range.start()))
            .then_with(|| left.range.end().cmp(&right.range.end()))
    });
    bindings.dedup_by(|left, right| left.file == right.file && left.range == right.range);
}

fn one_diagnostic(error: Diagnostic) -> Vec<Diagnostic> {
    vec![error]
}

fn contextualize_diagnostic(error: Diagnostic, instance_path: &InstancePath) -> Diagnostic {
    error.with_graph_path(GraphPath::new(instance_path.segments().iter().cloned()))
}

fn contextualize_diagnostics(
    errors: Vec<Diagnostic>,
    instance_path: &InstancePath,
) -> Vec<Diagnostic> {
    errors
        .into_iter()
        .map(|error| contextualize_diagnostic(error, instance_path))
        .collect()
}

#[derive(Debug, Default)]
struct ScopeIdentities {
    entities: BTreeMap<String, EntityIdentity>,
    relations: BTreeMap<String, RelationIdentity>,
    boundary_family_entities: BTreeMap<(String, FullElaborationIdentity), EntityIdentity>,
    boundary_family_relations: BTreeMap<(String, FullElaborationIdentity), RelationIdentity>,
}

struct ComponentOccurrence<'a, 'd> {
    definition: &'a ComponentDefinition<'d>,
    instance: &'a InstanceDecl,
    instance_file: &'a str,
    instance_path: &'a InstancePath,
}

struct ConnectionOrigin {
    instance: SourceLocation,
    bindings: Vec<SourceLocation>,
    definition_file: String,
}

#[derive(Debug, Clone)]
struct PhysicalPortOccurrence {
    identity: EntityIdentity,
    display_name: String,
    instance_path: InstancePath,
    exposure_candidate: bool,
    contract: Option<PhysicalExposureContractIdentity>,
}

struct PhysicalPortMaterialization {
    contract: PhysicalExposureContractIdentity,
}

struct PortFamilyMemberRegistration<'a> {
    file: &'a str,
    range: eqiora_lang::TextRange,
    display_name: String,
    family_name: &'a str,
    selector_member: &'a str,
    boundary: FullElaborationIdentity,
    identity: &'a EntityIdentity,
}

#[derive(Debug, Clone)]
struct PhysicalConnectionOrigin {
    declaration_path: DeclarationPath,
    instance_path: InstancePath,
    source: EntitySourceOrigin,
}

#[derive(Debug, Clone)]
struct StagedPhysicalConnection {
    topology: ConnectionFragment<FullElaborationIdentity>,
    origin: PhysicalConnectionOrigin,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct ConnectorSpecializationKey {
    definition: DefinitionKey,
    shape: ValueShape,
}

pub(super) struct RootExpansion<'a, 'd> {
    elaborator: &'a Elaborator<'d>,
    model: ModelDefinition<'d>,
    namespace: IdentityNamespace,
    root_path: InstancePath,
    model_key: ModelViewKey,
    model_full: FullElaborationIdentity,
    items: Vec<FlatItemBlueprint>,
    connector_domains: BTreeMap<ConnectorSpecializationKey, FlatSymbol>,
    display_symbols: BTreeMap<String, DisplayIdentity>,
    physical_ports: BTreeMap<FullElaborationIdentity, PhysicalPortOccurrence>,
    physical_ports_by_name: BTreeMap<String, FullElaborationIdentity>,
    physical_owner_relations: BTreeMap<FullElaborationIdentity, BTreeSet<FullElaborationIdentity>>,
    physical_connections: Vec<StagedPhysicalConnection>,
    spatial_periodic_ports: BTreeSet<FullElaborationIdentity>,
    physical_exposures: Vec<PhysicalExposureProjectionBlueprint>,
    boundary_embeddings: BTreeMap<FullElaborationIdentity, Option<CartesianBoundaryEmbedding>>,
    boundary_parents: BTreeMap<FullElaborationIdentity, FullElaborationIdentity>,
    boundary_sides: BTreeMap<FullElaborationIdentity, (usize, BoundarySide)>,
    complete_exterior_memberships: CompleteExteriorMembershipBudget,
}

impl<'a, 'd> RootExpansion<'a, 'd> {
    pub(super) fn new(
        elaborator: &'a Elaborator<'d>,
        model: ModelDefinition<'d>,
        size: ExpansionSize,
    ) -> Result<Self, Diagnostic> {
        let namespace = elaborator.identity_namespace.clone();
        let root_path = InstancePath::with_limits([model.name()], elaborator.limits.identity)?;
        let model_key = ModelViewKey::with_limits(
            namespace.clone(),
            root_path.clone(),
            elaborator.limits.identity,
        )?;
        let model_full = model_key.full_identity()?;
        let item_capacity = size
            .declarations
            .checked_add(size.connections)
            .and_then(|value| value.checked_add(1))
            .ok_or_else(|| hierarchy_error("flat item capacity overflows usize"))?;
        let mut items = Vec::new();
        items
            .try_reserve_exact(item_capacity)
            .map_err(|_| hierarchy_error("cannot reserve flat component expansion"))?;
        Ok(Self {
            elaborator,
            model,
            namespace,
            root_path,
            model_key,
            model_full,
            items,
            connector_domains: BTreeMap::new(),
            display_symbols: BTreeMap::new(),
            physical_ports: BTreeMap::new(),
            physical_ports_by_name: BTreeMap::new(),
            physical_owner_relations: BTreeMap::new(),
            physical_connections: Vec::new(),
            spatial_periodic_ports: BTreeSet::new(),
            physical_exposures: Vec::new(),
            boundary_embeddings: BTreeMap::new(),
            boundary_parents: BTreeMap::new(),
            boundary_sides: BTreeMap::new(),
            complete_exterior_memberships: CompleteExteriorMembershipBudget::new(
                elaborator.limits.complete_exteriors,
            ),
        })
    }

    fn entity_identity(
        &self,
        instance_path: &InstancePath,
        declaration_path: impl IntoIterator<Item = String>,
        kind: EntityKind,
        definition: SourceLocation,
        instance: SourceLocation,
        bindings: Vec<SourceLocation>,
    ) -> Result<EntityIdentity, Diagnostic> {
        let declaration_path =
            DeclarationPath::with_limits(declaration_path, self.elaborator.limits.identity)?;
        let key = ElaborationKey::entity_with_limits(
            self.namespace.clone(),
            instance_path.clone(),
            declaration_path,
            kind,
            self.elaborator.limits.identity,
        )?;
        let full = key.full_identity()?;
        Ok(EntityIdentity {
            key,
            full,
            definition,
            instance,
            bindings,
        })
    }

    fn relation_identity(
        &self,
        instance_path: &InstancePath,
        declaration_path: Vec<String>,
        definition: SourceLocation,
        instance: SourceLocation,
        bindings: Vec<SourceLocation>,
    ) -> Result<RelationIdentity, Diagnostic> {
        let entity = self.entity_identity(
            instance_path,
            declaration_path.clone(),
            EntityKind::Relation,
            definition,
            instance,
            bindings,
        )?;
        let declaration_path =
            DeclarationPath::with_limits(declaration_path, self.elaborator.limits.identity)?;
        let activation_key = ElaborationKey::generated_with_limits(
            self.namespace.clone(),
            instance_path.clone(),
            declaration_path,
            GeneratedRole::RelationActivation,
            self.elaborator.limits.identity,
        )?;
        let activation_full = activation_key.full_identity()?;
        Ok(RelationIdentity {
            entity,
            activation_key,
            activation_full,
        })
    }

    fn boundary_family_entity_identity(
        &self,
        instance_path: &InstancePath,
        declaration_path: Vec<String>,
        kind: EntityKind,
        boundary: FullElaborationIdentity,
        source: EntitySourceOrigin,
    ) -> Result<EntityIdentity, Diagnostic> {
        let declaration_path =
            DeclarationPath::with_limits(declaration_path, self.elaborator.limits.identity)?;
        let key = ElaborationKey::boundary_family_entity_with_limits(
            self.namespace.clone(),
            instance_path.clone(),
            declaration_path,
            kind,
            boundary,
            self.elaborator.limits.identity,
        )?;
        let full = key.full_identity()?;
        Ok(EntityIdentity {
            key,
            full,
            definition: source.definition,
            instance: source.instance,
            bindings: source.bindings,
        })
    }

    fn boundary_family_relation_identity(
        &self,
        instance_path: &InstancePath,
        declaration_path: Vec<String>,
        boundary: FullElaborationIdentity,
        source: EntitySourceOrigin,
    ) -> Result<RelationIdentity, Diagnostic> {
        let entity = self.boundary_family_entity_identity(
            instance_path,
            declaration_path.clone(),
            EntityKind::Relation,
            boundary,
            source,
        )?;
        let declaration_path =
            DeclarationPath::with_limits(declaration_path, self.elaborator.limits.identity)?;
        let activation_key = ElaborationKey::boundary_family_generated_with_limits(
            self.namespace.clone(),
            instance_path.clone(),
            declaration_path,
            GeneratedRole::RelationActivation,
            boundary,
            self.elaborator.limits.identity,
        )?;
        let activation_full = activation_key.full_identity()?;
        Ok(RelationIdentity {
            entity,
            activation_key,
            activation_full,
        })
    }

    fn register_symbol(
        &mut self,
        display_name: String,
        local_name: &str,
        identity: &EntityIdentity,
        kind: SymbolKind,
        scope: &mut Scope,
    ) -> Result<(), Diagnostic> {
        let internal_name = internal_name(identity.full);
        let symbol = FlatSymbol {
            internal_name,
            display_name: display_name.clone(),
            full_identity: identity.full,
            kind,
        };
        if scope
            .insert_symbol(local_name.to_owned(), symbol.clone())
            .is_some()
        {
            return Err(hierarchy_error(format!(
                "duplicate flattened scope symbol `{local_name}`"
            )));
        }
        if self
            .display_symbols
            .insert(
                display_name,
                DisplayIdentity {
                    full: identity.full,
                    kind: identity.key.entity_kind(),
                },
            )
            .is_some()
        {
            return Err(hierarchy_error(format!(
                "duplicate flattened display symbol `{}`",
                symbol.display_name
            )));
        }
        Ok(())
    }

    fn register_port_family_member(
        &mut self,
        registration: PortFamilyMemberRegistration<'_>,
        scope: &mut Scope,
    ) -> Result<(), Diagnostic> {
        let symbol = FlatSymbol {
            internal_name: internal_name(registration.identity.full),
            display_name: registration.display_name.clone(),
            full_identity: registration.identity.full,
            kind: SymbolKind::Port,
        };
        scope.insert_port_family_member(
            registration.file,
            registration.range,
            registration.family_name.to_owned(),
            registration.selector_member,
            registration.boundary,
            symbol.clone(),
        )?;
        if self
            .display_symbols
            .insert(
                registration.display_name,
                DisplayIdentity {
                    full: registration.identity.full,
                    kind: EntityKind::Port,
                },
            )
            .is_some()
        {
            return Err(hierarchy_error(format!(
                "duplicate flattened display symbol `{}`",
                symbol.display_name
            )));
        }
        Ok(())
    }

    fn register_family_relation_display(
        &mut self,
        display_name: String,
        identity: &RelationIdentity,
    ) -> Result<(), Diagnostic> {
        if self
            .display_symbols
            .insert(
                display_name.clone(),
                DisplayIdentity {
                    full: identity.entity.full,
                    kind: EntityKind::Relation,
                },
            )
            .is_some()
        {
            return Err(hierarchy_error(format!(
                "duplicate flattened display symbol `{display_name}`"
            )));
        }
        Ok(())
    }

    fn register_physical_port_occurrence(
        &mut self,
        identity: EntityIdentity,
        display_name: String,
        instance_path: InstancePath,
        exposure_candidate: bool,
        contract: Option<PhysicalExposureContractIdentity>,
    ) -> Result<(), Diagnostic> {
        let full = identity.full;
        let internal_name = internal_name(full);
        if self
            .physical_ports_by_name
            .insert(internal_name, full)
            .is_some()
        {
            return Err(hierarchy_error(format!(
                "duplicate flattened physical Port identity {full}"
            )));
        }
        if self
            .physical_ports
            .insert(
                full,
                PhysicalPortOccurrence {
                    identity,
                    display_name,
                    instance_path,
                    exposure_candidate,
                    contract,
                },
            )
            .is_some()
        {
            return Err(hierarchy_error(format!(
                "duplicate flattened physical Port occurrence {full}"
            )));
        }
        Ok(())
    }

    fn record_physical_relation_owners(
        &mut self,
        file: &str,
        range: eqiora_lang::TextRange,
        relation: FullElaborationIdentity,
        residuals: &[LoweringExpression],
    ) -> Result<(), Diagnostic> {
        let mut names = BTreeSet::new();
        if residuals
            .iter()
            .any(|expression| !expression.collect_physical_port_names(&mut names))
        {
            return Err(source_error(
                codes::LANGUAGE_LOWERING_ERROR,
                file,
                range,
                "Relation expression is newer than physical ownership analysis",
            ));
        }
        let mut selected = BTreeSet::new();
        for name in names {
            if let Some(port) = self.physical_ports_by_name.get(&name) {
                selected.insert(*port);
            }
        }
        for port in selected {
            let owners = self.physical_owner_relations.entry(port).or_default();
            owners.insert(relation);
            if owners.len() > 1 {
                let display = &self.physical_ports[&port].display_name;
                return Err(source_error(
                    codes::LANGUAGE_TYPE_ERROR,
                    file,
                    range,
                    format!("physical Port `{display}` cannot have more than one owning Relation"),
                ));
            }
        }
        Ok(())
    }

    fn connector_domain(
        &mut self,
        connector: ConnectorDefinition<'d>,
        ambient_dimension: Option<usize>,
    ) -> Result<FlatSymbol, Diagnostic> {
        let definition = DefinitionKey {
            namespace: connector.namespace.clone(),
            name: connector.name().to_owned(),
        };
        let (shape, contract) = match connector.syntax() {
            ConnectorSyntax::ScalarPhysical {
                across_dimension,
                through_dimension,
            } => {
                lower_dimension(connector.file, across_dimension)?;
                lower_dimension(connector.file, through_dimension)?;
                (
                    ValueShape::scalar(),
                    LoweringDomainContract::Source(DomainSyntax::ScalarPhysical {
                        across_dimension: across_dimension.clone(),
                        through_dimension: through_dimension.clone(),
                    }),
                )
            }
            ConnectorSyntax::FieldPhysical {
                trace,
                flux,
                shape,
                frame,
                pairing,
            } => {
                let ambient_dimension = ambient_dimension.ok_or_else(|| {
                    source_error(
                        codes::LANGUAGE_TYPE_ERROR,
                        connector.file,
                        connector.range(),
                        "field-physical Connector specialization requires an exact boundary support",
                    )
                })?;
                let shape = super::body_check::resolve_value_shape(
                    connector.file,
                    connector.range(),
                    shape,
                    ambient_dimension,
                )
                .map_err(|mut errors| {
                    errors.pop().unwrap_or_else(|| {
                        hierarchy_error("value-shape specialization failed without a diagnostic")
                    })
                })?;
                let frame = match frame {
                    FrameSyntax::Invariant => ValueFrame::Invariant,
                    FrameSyntax::Spatial => ValueFrame::SpatialCartesian,
                    _ => {
                        return Err(source_error(
                            codes::LANGUAGE_LOWERING_ERROR,
                            connector.file,
                            connector.range(),
                            "Connector frame is newer than this compiler",
                        ));
                    }
                };
                if frame == ValueFrame::SpatialCartesian
                    && (shape.is_scalar()
                        || shape.extents().iter().any(|extent| {
                            usize::try_from(extent.get()).ok() != Some(ambient_dimension)
                        }))
                {
                    return Err(source_error(
                        codes::LANGUAGE_TYPE_ERROR,
                        connector.file,
                        connector.range(),
                        "spatial Connector extents must all equal the exact support ambient dimension",
                    ));
                }
                let pairing = match pairing {
                    BoundaryPairingSyntax::EuclideanBoundaryDuality => {
                        BoundaryPairing::EuclideanBoundaryDuality
                    }
                    _ => {
                        return Err(source_error(
                            codes::LANGUAGE_LOWERING_ERROR,
                            connector.file,
                            connector.range(),
                            "Connector pairing is newer than this compiler",
                        ));
                    }
                };
                let contract = BoundaryPhysicalConnector::new(
                    lower_dimension(connector.file, trace.dimension())?,
                    lower_dimension(connector.file, flux.dimension())?,
                    shape.clone(),
                    frame,
                    pairing,
                )
                .map_err(|violation| {
                    source_error(
                        codes::LANGUAGE_TYPE_ERROR,
                        connector.file,
                        connector.range(),
                        format!("invalid field-physical Connector: {violation:?}"),
                    )
                })?;
                (shape, LoweringDomainContract::BoundaryPhysical(contract))
            }
            _ => {
                return Err(source_error(
                    codes::LANGUAGE_LOWERING_ERROR,
                    connector.file,
                    connector.range(),
                    "Connector syntax is newer than this compiler",
                ));
            }
        };
        let key = ConnectorSpecializationKey {
            definition,
            shape: shape.clone(),
        };
        if let Some(domain) = self.connector_domains.get(&key) {
            return Ok(domain.clone());
        }
        let mut declaration_path = connector.namespace.declaration_prefix();
        declaration_path.extend(["connector".to_owned(), connector.name().to_owned()]);
        if matches!(&contract, LoweringDomainContract::BoundaryPhysical(_)) {
            declaration_path.push("shape".to_owned());
            let specialization = if shape.is_scalar() {
                "scalar".to_owned()
            } else {
                shape
                    .extents()
                    .iter()
                    .map(|extent| extent.get().to_string())
                    .collect::<Vec<_>>()
                    .join("x")
            };
            declaration_path.push(specialization);
        }
        let identity = self.entity_identity(
            &self.root_path,
            declaration_path,
            EntityKind::Domain,
            SourceLocation::new(connector.file, connector.range()),
            SourceLocation::new(self.model.file, self.model.range()),
            Vec::new(),
        )?;
        let internal_name = internal_name(identity.full);
        self.items.push(FlatItemBlueprint::Domain {
            name: internal_name.clone(),
            contract,
            range: connector.range(),
            identity: identity.clone(),
        });
        let mut display_name = match &connector.namespace {
            DefinitionNamespace::Local => format!("connector::{}", connector.name()),
            DefinitionNamespace::Resolved(owner) => {
                format!("connector::{owner}::{}", connector.name())
            }
        };
        if !shape.is_scalar() {
            display_name.push_str(&format!("::{shape:?}"));
        }
        let symbol = FlatSymbol {
            internal_name,
            display_name: display_name.clone(),
            full_identity: identity.full,
            kind: SymbolKind::Domain,
        };
        self.display_symbols.insert(
            display_name,
            DisplayIdentity {
                full: identity.full,
                kind: EntityKind::Domain,
            },
        );
        self.connector_domains.insert(key, symbol.clone());
        Ok(symbol)
    }

    pub(super) fn expand(mut self) -> Result<ExpandedBlueprint, Vec<Diagnostic>> {
        let model = self.model.clone();
        let mut root_scope = Scope::default();
        root_scope.set_pure_operators(self.elaborator.visible_pure_operators(&model.namespace));
        let identities = match self.allocate_model_scope(&mut root_scope) {
            Ok(value) => value,
            Err(error) => return Err(vec![error]),
        };

        for item in model.items() {
            if let Item::Instance(instance) = item {
                let component = self
                    .elaborator
                    .resolve_component(
                        &model.namespace,
                        instance.definition(),
                        model.file,
                        instance.range(),
                    )
                    .map_err(one_diagnostic)?;
                let instance_path = match child_instance_path(
                    &self.root_path,
                    instance.name(),
                    self.elaborator.limits.identity,
                ) {
                    Ok(value) => value,
                    Err(error) => return Err(vec![error]),
                };
                let interface = self.expand_component(
                    component,
                    instance,
                    model.file,
                    instance_path,
                    instance.name().to_owned(),
                    &root_scope,
                )?;
                root_scope.insert_child(instance.name().to_owned(), interface);
            }
        }

        if let Err(error) = self.materialize_model_items(&root_scope, &identities) {
            return Err(vec![error]);
        }
        if let Err(error) = self.finalize_physical_connections() {
            return Err(vec![error]);
        }
        self.items.sort_by_key(FlatItemBlueprint::sort_key);
        Ok(ExpandedBlueprint::new(
            self.model.name().to_owned(),
            SourceLocation::new(self.model.file, self.model.range()),
            self.model_key,
            self.model_full,
            self.items,
            self.display_symbols,
            self.physical_exposures,
        ))
    }

    fn allocate_model_scope(&mut self, scope: &mut Scope) -> Result<ScopeIdentities, Diagnostic> {
        let model = self.model.clone();
        let mut identities = ScopeIdentities::default();
        for item in model.items() {
            let (name, kind, symbol_kind, parameter_value, range) = match item {
                Item::Domain(value) => (
                    value.name(),
                    EntityKind::Domain,
                    SymbolKind::Domain,
                    None,
                    value.range(),
                ),
                Item::Representation(value) => (
                    value.name(),
                    EntityKind::Representation,
                    SymbolKind::Representation,
                    None,
                    value.range(),
                ),
                Item::Field(value) => (
                    value.name(),
                    EntityKind::Field,
                    SymbolKind::Field,
                    None,
                    value.range(),
                ),
                Item::Parameter(declaration) => {
                    let dimension = lower_dimension(self.model.file, declaration.dimension())?;
                    let value = ConstantValue {
                        value: normalize_zero(declaration.initial()),
                        dimension,
                    };
                    (
                        declaration.name(),
                        EntityKind::Parameter,
                        SymbolKind::Parameter,
                        Some(value),
                        declaration.range(),
                    )
                }
                Item::Port(value) => (
                    value.name(),
                    EntityKind::Port,
                    SymbolKind::Port,
                    None,
                    value.range(),
                ),
                Item::Clock(value) => (
                    value.name(),
                    EntityKind::ClockDomain,
                    SymbolKind::Clock,
                    None,
                    value.range(),
                ),
                Item::Relation(value) => {
                    let path = definition_path(
                        &self.model.namespace,
                        "model",
                        self.model.name(),
                        value.name(),
                    );
                    let identity = self.relation_identity(
                        &self.root_path,
                        path,
                        SourceLocation::new(self.model.file, value.range()),
                        SourceLocation::new(self.model.file, self.model.range()),
                        Vec::new(),
                    )?;
                    self.register_symbol(
                        value.name().to_owned(),
                        value.name(),
                        &identity.entity,
                        SymbolKind::Relation,
                        scope,
                    )?;
                    identities
                        .relations
                        .insert(value.name().to_owned(), identity);
                    continue;
                }
                Item::Connection(_)
                | Item::BoundaryConnection(_)
                | Item::Boundary(_)
                | Item::Let(_)
                | Item::Instance(_) => continue,
                _ => {
                    return Err(source_error(
                        codes::LANGUAGE_LOWERING_ERROR,
                        self.model.file,
                        self.model.range(),
                        "model item is newer than hierarchy elaboration",
                    ));
                }
            };
            let identity = self.entity_identity(
                &self.root_path,
                definition_path(&self.model.namespace, "model", self.model.name(), name),
                kind,
                SourceLocation::new(self.model.file, range),
                SourceLocation::new(self.model.file, self.model.range()),
                Vec::new(),
            )?;
            self.register_symbol(name.to_owned(), name, &identity, symbol_kind, scope)?;
            if let Some(value) = parameter_value
                && scope
                    .insert_parameter(
                        name.to_owned(),
                        ResolvedParameter::model_parameter(
                            value,
                            identity.full,
                            internal_name(identity.full),
                            range,
                        ),
                    )
                    .is_some()
            {
                return Err(hierarchy_error(format!(
                    "duplicate flattened Parameter term `{name}`"
                )));
            }
            identities.entities.insert(name.to_owned(), identity);
        }
        let mut compile_time_values = scope.symbolic_parameters();
        super::parameters::resolve_model_lets(self.model.file, &model, &mut compile_time_values)
            .map_err(|diagnostics| {
                diagnostics
                    .into_iter()
                    .next()
                    .unwrap_or_else(|| hierarchy_error("let alias resolution failed"))
            })?;
        for item in model.items() {
            let Item::Let(declaration) = item else {
                continue;
            };
            let value = compile_time_values
                .remove(declaration.name())
                .ok_or_else(|| hierarchy_error("resolved let alias is missing"))?;
            scope
                .insert_let(declaration.name().to_owned(), value)
                .map_err(hierarchy_error)?;
        }
        for item in model.items() {
            let Item::Domain(declaration) = item else {
                continue;
            };
            match declaration.syntax() {
                DomainSyntax::CartesianBox(bounds) => {
                    scope.insert_spatial_support(
                        declaration.name().to_owned(),
                        SpatialSupport::Volume {
                            domain: identities.entities[declaration.name()].full,
                            dimensions: bounds.len(),
                        },
                    );
                }
                DomainSyntax::Boundary { .. } | DomainSyntax::ScalarPhysical { .. } => {}
                _ => {
                    return Err(source_error(
                        codes::LANGUAGE_LOWERING_ERROR,
                        self.model.file,
                        declaration.range(),
                        "Domain syntax is newer than spatial-support allocation",
                    ));
                }
            }
        }
        self.allocate_cartesian_boundaries(scope, &identities)?;
        for item in model.items() {
            let Item::Field(declaration) = item else {
                continue;
            };
            let support = declaration
                .domain()
                .and_then(|domain| scope.spatial_support(domain).cloned());
            let field_type = field_expression_type(self.model.file, declaration, support)?;
            if scope
                .insert_field_type(declaration.name().to_owned(), field_type)
                .is_some()
            {
                return Err(hierarchy_error(format!(
                    "duplicate flattened Field type `{}`",
                    declaration.name()
                )));
            }
        }
        for item in model.items() {
            let Item::Port(declaration) = item else {
                continue;
            };
            if !matches!(declaration.syntax(), PortSyntax::ScalarPhysical { .. }) {
                continue;
            }
            let PortSyntax::ScalarPhysical { domain } = declaration.syntax() else {
                unreachable!("scalar-physical Port syntax was selected above");
            };
            let connector = scope.symbol(domain).ok_or_else(|| {
                source_error(
                    codes::LANGUAGE_LOWERING_ERROR,
                    self.model.file,
                    declaration.range(),
                    "resolved scalar-physical Port Domain has no flattened symbol",
                )
            })?;
            let identity = identities.entities[declaration.name()].clone();
            self.register_physical_port_occurrence(
                identity,
                declaration.name().to_owned(),
                self.root_path.clone(),
                false,
                Some(PhysicalExposureContractIdentity::ScalarPhysical {
                    connector: connector.full_identity,
                }),
            )?;
        }
        Ok(identities)
    }

    fn expand_component(
        &mut self,
        component: ComponentDefinition<'d>,
        instance: &InstanceDecl,
        instance_file: &str,
        instance_path: InstancePath,
        display_prefix: String,
        parent_scope: &Scope,
    ) -> Result<InstanceInterface, Vec<Diagnostic>> {
        let parameters = ParameterResolver::new(
            component.file,
            instance_file,
            &component,
            instance,
            |name| parent_scope.parameter(name).cloned(),
        )
        .and_then(ParameterResolver::resolve_all)
        .map_err(|errors| contextualize_diagnostics(errors, &instance_path))?;
        let support_interface = component_support_interface(component.file, component.declaration)
            .map_err(|errors| contextualize_diagnostics(errors, &instance_path))?;
        let boundary_sides = &self.boundary_sides;
        let membership_budget = &mut self.complete_exterior_memberships;
        let support_bindings = resolve_instance_support_bindings(
            instance_file,
            component.declaration,
            &support_interface,
            instance,
            |name| parent_scope.spatial_support(name).cloned(),
            |name| {
                let SpatialSupport::Boundary { domain, .. } = parent_scope.spatial_support(name)?
                else {
                    return None;
                };
                let symbol = parent_scope.symbol(name)?;
                Some(ResolvedBoundaryTarget::new(
                    symbol.internal_name.clone(),
                    *domain,
                ))
            },
            |identity| match parent_scope.spatial_support_by_identity(*identity)? {
                SpatialSupport::Volume { dimensions, .. } => Some(CartesianDomain::Volume {
                    ambient_dimension: *dimensions,
                }),
                SpatialSupport::Boundary {
                    parent, dimensions, ..
                } => {
                    let (axis, side) = boundary_sides.get(identity)?;
                    Some(CartesianDomain::Boundary {
                        exact_parent: *parent,
                        ambient_dimension: *dimensions,
                        axis: *axis,
                        side: *side,
                    })
                }
                SpatialSupport::Interface { .. } => None,
            },
            |name| parent_scope.boundary_set(name).cloned(),
            membership_budget,
        )
        .map_err(|errors| contextualize_diagnostics(errors, &instance_path))?;
        let mut bindings = parent_scope
            .forwarded_parameter_resolution_bindings()
            .to_vec();
        bindings.extend_from_slice(parent_scope.forwarded_field_resolution_bindings());
        bindings.extend_from_slice(parent_scope.forwarded_boundary_set_resolution_bindings());
        bindings.extend(instance_binding_locations(instance_file, instance));
        normalize_binding_locations(&mut bindings);
        let mut forwarded_field_resolution_bindings =
            parent_scope.forwarded_field_resolution_bindings().to_vec();
        forwarded_field_resolution_bindings
            .extend(field_forwarding_locations(instance_file, instance));
        normalize_binding_locations(&mut forwarded_field_resolution_bindings);
        let mut forwarded_parameter_resolution_bindings = parent_scope
            .forwarded_parameter_resolution_bindings()
            .to_vec();
        forwarded_parameter_resolution_bindings
            .extend(parameter_forwarding_locations(instance_file, instance));
        normalize_binding_locations(&mut forwarded_parameter_resolution_bindings);
        let mut forwarded_boundary_set_resolution_bindings = parent_scope
            .forwarded_boundary_set_resolution_bindings()
            .to_vec();
        forwarded_boundary_set_resolution_bindings.extend(boundary_set_forwarding_locations(
            instance_file,
            instance,
            &support_bindings,
        ));
        normalize_binding_locations(&mut forwarded_boundary_set_resolution_bindings);
        let mut scope = Scope::child(parent_scope);
        scope.set_pure_operators(self.elaborator.visible_pure_operators(&component.namespace));
        scope.set_occurrence_bindings(bindings.clone());
        scope.set_forwarded_parameter_resolution_bindings(forwarded_parameter_resolution_bindings);
        scope.set_forwarded_field_resolution_bindings(forwarded_field_resolution_bindings);
        scope.set_forwarded_boundary_set_resolution_bindings(
            forwarded_boundary_set_resolution_bindings,
        );
        let mut identities = ScopeIdentities::default();

        for (slot, target) in support_bindings.singular_targets() {
            let Some(symbol) = parent_scope.symbol(target).cloned() else {
                return Err(vec![contextualize_diagnostic(
                    source_error(
                        codes::LANGUAGE_LOWERING_ERROR,
                        instance_file,
                        instance.range(),
                        format!("resolved support target `{target}` has no flattened symbol"),
                    ),
                    &instance_path,
                )]);
            };
            let support = support_bindings.singular_supports()[slot].clone();
            if scope.insert_symbol(slot.clone(), symbol).is_some()
                || scope
                    .insert_spatial_support(slot.clone(), support)
                    .is_some()
            {
                return Err(vec![contextualize_diagnostic(
                    hierarchy_error(format!("duplicate flattened support alias `{slot}`")),
                    &instance_path,
                )]);
            }
        }
        for (slot, set) in support_bindings.boundary_sets() {
            if scope
                .insert_boundary_set(slot.to_owned(), set.clone())
                .is_some()
            {
                return Err(vec![contextualize_diagnostic(
                    hierarchy_error(format!(
                        "duplicate flattened complete-exterior binding `{slot}`"
                    )),
                    &instance_path,
                )]);
            }
        }

        let field_interface =
            component_field_interface(component.file, component.declaration, &support_interface)
                .map_err(|errors| contextualize_diagnostics(errors, &instance_path))?;
        let field_bindings = resolve_instance_fields(
            instance_file,
            component.declaration,
            &field_interface,
            instance,
            |slot| scope.spatial_support(slot).cloned(),
            |target| {
                let symbol = parent_scope.symbol(target)?;
                if !matches!(symbol.kind, SymbolKind::Field) {
                    return None;
                }
                parent_scope
                    .field_type(target)
                    .cloned()
                    .map(FieldContract::continuum)
            },
        )
        .map_err(|errors| contextualize_diagnostics(errors, &instance_path))?;
        for (slot, target) in field_bindings {
            let symbol = parent_scope.symbol(&target).cloned().ok_or_else(|| {
                vec![contextualize_diagnostic(
                    hierarchy_error(format!(
                        "resolved Field target `{target}` has no flattened symbol"
                    )),
                    &instance_path,
                )]
            })?;
            let field_type = parent_scope.field_type(&target).cloned().ok_or_else(|| {
                vec![contextualize_diagnostic(
                    hierarchy_error(format!(
                        "resolved Field target `{target}` has no exact type"
                    )),
                    &instance_path,
                )]
            })?;
            if scope.symbol(&slot).is_some() || scope.field_type(&slot).is_some() {
                return Err(vec![contextualize_diagnostic(
                    hierarchy_error(format!("duplicate flattened Field alias `{slot}`")),
                    &instance_path,
                )]);
            }
            scope.insert_symbol(slot.clone(), symbol);
            scope.insert_field_type(slot, field_type);
        }

        for item in component.items() {
            match item {
                ComponentItem::Parameter(declaration) => {
                    let resolved = parameters[declaration.name()].clone();
                    if scope
                        .insert_parameter(declaration.name().to_owned(), resolved.clone())
                        .is_some()
                    {
                        return Err(vec![contextualize_diagnostic(
                            hierarchy_error(format!(
                                "duplicate flattened Parameter term `{}`",
                                declaration.name()
                            )),
                            &instance_path,
                        )]);
                    }
                    if let ParameterLineage::Parameter(full) = resolved.lineage {
                        let display = display_child(&display_prefix, declaration.name());
                        if self
                            .display_symbols
                            .insert(
                                display.clone(),
                                DisplayIdentity {
                                    full,
                                    kind: EntityKind::Parameter,
                                },
                            )
                            .is_some()
                        {
                            return Err(vec![contextualize_diagnostic(
                                hierarchy_error(format!(
                                    "duplicate flattened display symbol `{display}`"
                                )),
                                &instance_path,
                            )]);
                        }
                    }
                }
                ComponentItem::Port(declaration) => {
                    let identity = self
                        .entity_identity(
                            &instance_path,
                            definition_path(
                                &component.namespace,
                                "component",
                                component.name(),
                                declaration.name(),
                            ),
                            EntityKind::Port,
                            SourceLocation::new(component.file, declaration.range()),
                            SourceLocation::new(instance_file, instance.range()),
                            bindings.clone(),
                        )
                        .map_err(one_diagnostic)?;
                    self.register_symbol(
                        display_child(&display_prefix, declaration.name()),
                        declaration.name(),
                        &identity,
                        SymbolKind::Port,
                        &mut scope,
                    )
                    .map_err(one_diagnostic)?;
                    identities
                        .entities
                        .insert(declaration.name().to_owned(), identity);
                }
                ComponentItem::PortFamily(family) => {
                    let declaration = family.port();
                    let set = support_bindings
                        .boundary_set(family.binder().set())
                        .ok_or_else(|| {
                            vec![contextualize_diagnostic(
                                hierarchy_error(format!(
                                    "Port family `{}` has no resolved complete-exterior binding `{}`",
                                    declaration.name(),
                                    family.binder().set()
                                )),
                                &instance_path,
                            )]
                        })?;
                    for side in set.witness().sides() {
                        let boundary = *side.boundary();
                        let member = set.member(&boundary).ok_or_else(|| {
                            vec![contextualize_diagnostic(
                                hierarchy_error(
                                    "complete-exterior witness has no identity-keyed member locator",
                                ),
                                &instance_path,
                            )]
                        })?;
                        let member_bindings = boundary_family_bindings(
                            &bindings,
                            instance_file,
                            member.source_range(),
                        );
                        let identity = self
                            .boundary_family_entity_identity(
                                &instance_path,
                                definition_path(
                                    &component.namespace,
                                    "component",
                                    component.name(),
                                    declaration.name(),
                                ),
                                EntityKind::Port,
                                boundary,
                                EntitySourceOrigin {
                                    definition: SourceLocation::new(component.file, family.range()),
                                    instance: SourceLocation::new(instance_file, instance.range()),
                                    bindings: member_bindings,
                                },
                            )
                            .map_err(one_diagnostic)?;
                        self.register_port_family_member(
                            PortFamilyMemberRegistration {
                                file: component.file,
                                range: family.range(),
                                display_name: boundary_family_display(
                                    &display_prefix,
                                    declaration.name(),
                                    side.axis(),
                                    side.side(),
                                ),
                                family_name: declaration.name(),
                                selector_member: family.binder().member(),
                                boundary,
                                identity: &identity,
                            },
                            &mut scope,
                        )
                        .map_err(one_diagnostic)?;
                        identities
                            .boundary_family_entities
                            .insert((declaration.name().to_owned(), boundary), identity);
                    }
                }
                ComponentItem::Support(_) => {}
                ComponentItem::Representation(declaration) => {
                    let identity = self
                        .entity_identity(
                            &instance_path,
                            definition_path(
                                &component.namespace,
                                "component",
                                component.name(),
                                declaration.name(),
                            ),
                            EntityKind::Representation,
                            SourceLocation::new(component.file, declaration.range()),
                            SourceLocation::new(instance_file, instance.range()),
                            bindings.clone(),
                        )
                        .map_err(one_diagnostic)?;
                    self.register_symbol(
                        display_child(&display_prefix, declaration.name()),
                        declaration.name(),
                        &identity,
                        SymbolKind::Representation,
                        &mut scope,
                    )
                    .map_err(one_diagnostic)?;
                    identities
                        .entities
                        .insert(declaration.name().to_owned(), identity);
                }
                ComponentItem::Field(declaration) => {
                    let support = declaration
                        .domain()
                        .and_then(|domain| scope.spatial_support(domain).cloned());
                    let field_type = field_expression_type(component.file, declaration, support)
                        .map_err(one_diagnostic)?;
                    let identity = self
                        .entity_identity(
                            &instance_path,
                            definition_path(
                                &component.namespace,
                                "component",
                                component.name(),
                                declaration.name(),
                            ),
                            EntityKind::Field,
                            SourceLocation::new(component.file, declaration.range()),
                            SourceLocation::new(instance_file, instance.range()),
                            bindings.clone(),
                        )
                        .map_err(one_diagnostic)?;
                    self.register_symbol(
                        display_child(&display_prefix, declaration.name()),
                        declaration.name(),
                        &identity,
                        SymbolKind::Field,
                        &mut scope,
                    )
                    .map_err(one_diagnostic)?;
                    identities
                        .entities
                        .insert(declaration.name().to_owned(), identity);
                    if scope
                        .insert_field_type(declaration.name().to_owned(), field_type)
                        .is_some()
                    {
                        return Err(vec![hierarchy_error(format!(
                            "duplicate flattened Field type `{}`",
                            declaration.name()
                        ))]);
                    }
                }
                ComponentItem::Clock(declaration) => {
                    let identity = self
                        .entity_identity(
                            &instance_path,
                            definition_path(
                                &component.namespace,
                                "component",
                                component.name(),
                                declaration.name(),
                            ),
                            EntityKind::ClockDomain,
                            SourceLocation::new(component.file, declaration.range()),
                            SourceLocation::new(instance_file, instance.range()),
                            bindings.clone(),
                        )
                        .map_err(one_diagnostic)?;
                    self.register_symbol(
                        display_child(&display_prefix, declaration.name()),
                        declaration.name(),
                        &identity,
                        SymbolKind::Clock,
                        &mut scope,
                    )
                    .map_err(one_diagnostic)?;
                    identities
                        .entities
                        .insert(declaration.name().to_owned(), identity);
                }
                ComponentItem::Relation(declaration) => {
                    let identity = self
                        .relation_identity(
                            &instance_path,
                            definition_path(
                                &component.namespace,
                                "component",
                                component.name(),
                                declaration.name(),
                            ),
                            SourceLocation::new(component.file, declaration.range()),
                            SourceLocation::new(instance_file, instance.range()),
                            bindings.clone(),
                        )
                        .map_err(one_diagnostic)?;
                    self.register_symbol(
                        display_child(&display_prefix, declaration.name()),
                        declaration.name(),
                        &identity.entity,
                        SymbolKind::Relation,
                        &mut scope,
                    )
                    .map_err(one_diagnostic)?;
                    identities
                        .relations
                        .insert(declaration.name().to_owned(), identity);
                }
                ComponentItem::RelationFamily(family) => {
                    let declaration = family.relation();
                    let set = support_bindings
                        .boundary_set(family.binder().set())
                        .ok_or_else(|| {
                            vec![contextualize_diagnostic(
                                hierarchy_error(format!(
                                    "Relation family `{}` has no resolved complete-exterior binding `{}`",
                                    declaration.name(),
                                    family.binder().set()
                                )),
                                &instance_path,
                            )]
                        })?;
                    for side in set.witness().sides() {
                        let boundary = *side.boundary();
                        let member = set.member(&boundary).ok_or_else(|| {
                            vec![contextualize_diagnostic(
                                hierarchy_error(
                                    "complete-exterior witness has no identity-keyed member locator",
                                ),
                                &instance_path,
                            )]
                        })?;
                        let identity = self
                            .boundary_family_relation_identity(
                                &instance_path,
                                definition_path(
                                    &component.namespace,
                                    "component",
                                    component.name(),
                                    declaration.name(),
                                ),
                                boundary,
                                EntitySourceOrigin {
                                    definition: SourceLocation::new(component.file, family.range()),
                                    instance: SourceLocation::new(instance_file, instance.range()),
                                    bindings: boundary_family_bindings(
                                        &bindings,
                                        instance_file,
                                        member.source_range(),
                                    ),
                                },
                            )
                            .map_err(one_diagnostic)?;
                        self.register_family_relation_display(
                            boundary_family_display(
                                &display_prefix,
                                declaration.name(),
                                side.axis(),
                                side.side(),
                            ),
                            &identity,
                        )
                        .map_err(one_diagnostic)?;
                        identities
                            .boundary_family_relations
                            .insert((declaration.name().to_owned(), boundary), identity);
                    }
                }
                ComponentItem::FieldSlot(_)
                | ComponentItem::Connection(_)
                | ComponentItem::BoundaryConnection(_)
                | ComponentItem::Instance(_) => {}
                _ => {
                    return Err(vec![source_error(
                        codes::LANGUAGE_LOWERING_ERROR,
                        component.file,
                        component.range(),
                        "component item is newer than hierarchy elaboration",
                    )]);
                }
            }
        }

        for item in component.items() {
            match item {
                ComponentItem::Port(declaration)
                    if matches!(
                        declaration.syntax(),
                        PortSyntax::ScalarPhysicalConnector { .. }
                            | PortSyntax::FieldPhysical { .. }
                    ) =>
                {
                    let identity = identities.entities[declaration.name()].clone();
                    self.register_physical_port_occurrence(
                        identity,
                        display_child(&display_prefix, declaration.name()),
                        instance_path.clone(),
                        declaration.visibility() == VisibilitySyntax::Public,
                        None,
                    )
                    .map_err(one_diagnostic)?;
                }
                ComponentItem::PortFamily(family) => {
                    let declaration = family.port();
                    let set = support_bindings
                        .boundary_set(family.binder().set())
                        .ok_or_else(|| {
                            vec![contextualize_diagnostic(
                                hierarchy_error(format!(
                                    "Port family `{}` has no resolved complete-exterior binding `{}`",
                                    declaration.name(),
                                    family.binder().set()
                                )),
                                &instance_path,
                            )]
                        })?;
                    for side in set.witness().sides() {
                        let boundary = *side.boundary();
                        let identity = identities
                            .boundary_family_entities
                            .get(&(declaration.name().to_owned(), boundary))
                            .cloned()
                            .ok_or_else(|| {
                                vec![contextualize_diagnostic(
                                    hierarchy_error(format!(
                                        "Port family `{}` member identity was not allocated",
                                        declaration.name()
                                    )),
                                    &instance_path,
                                )]
                            })?;
                        self.register_physical_port_occurrence(
                            identity,
                            boundary_family_display(
                                &display_prefix,
                                declaration.name(),
                                side.axis(),
                                side.side(),
                            ),
                            instance_path.clone(),
                            declaration.visibility() == VisibilitySyntax::Public,
                            None,
                        )
                        .map_err(one_diagnostic)?;
                    }
                }
                _ => {}
            }
        }

        for item in component.items() {
            if let ComponentItem::Instance(child) = item {
                let child_component = self
                    .elaborator
                    .resolve_component(
                        &component.namespace,
                        child.definition(),
                        component.file,
                        child.range(),
                    )
                    .map_err(one_diagnostic)?;
                let child_path = child_instance_path(
                    &instance_path,
                    child.name(),
                    self.elaborator.limits.identity,
                )
                .map_err(one_diagnostic)?;
                let child_interface = self.expand_component(
                    child_component,
                    child,
                    component.file,
                    child_path,
                    display_child(&display_prefix, child.name()),
                    &scope,
                )?;
                scope.insert_child(child.name().to_owned(), child_interface);
            }
        }

        self.materialize_component_items(
            ComponentOccurrence {
                definition: &component,
                instance,
                instance_file,
                instance_path: &instance_path,
            },
            &scope,
            &identities,
            &support_bindings,
        )
        .map_err(|error| vec![contextualize_diagnostic(error, &instance_path)])?;

        let public_ports = component
            .items()
            .iter()
            .filter_map(|item| match item {
                ComponentItem::Port(port) if port.visibility() == VisibilitySyntax::Public => scope
                    .symbol(port.name())
                    .cloned()
                    .map(|symbol| (port.name().to_owned(), symbol)),
                _ => None,
            })
            .collect();
        let public_port_families = component
            .items()
            .iter()
            .filter_map(|item| match item {
                ComponentItem::PortFamily(family)
                    if family.port().visibility() == VisibilitySyntax::Public =>
                {
                    scope
                        .port_family(family.port().name())
                        .cloned()
                        .map(|index| (family.port().name().to_owned(), index))
                }
                _ => None,
            })
            .collect();
        Ok(InstanceInterface::with_public_port_families(
            public_ports,
            public_port_families,
        ))
    }

    fn materialize_component_items(
        &mut self,
        occurrence: ComponentOccurrence<'_, 'd>,
        scope: &Scope,
        identities: &ScopeIdentities,
        support_bindings: &ResolvedSupportBindings<FullElaborationIdentity>,
    ) -> Result<(), Diagnostic> {
        let component = occurrence.definition;
        let instance = occurrence.instance;
        for item in component.items() {
            match item {
                ComponentItem::Parameter(_) => {}
                ComponentItem::Port(declaration) => {
                    let identity = identities.entities[declaration.name()].clone();
                    let (contract, materialization) =
                        self.component_port_syntax(component, declaration, scope)?;
                    if let Some(materialization) = materialization {
                        let occurrence =
                            self.physical_ports.get_mut(&identity.full).ok_or_else(|| {
                                hierarchy_error("physical Port occurrence was not registered")
                            })?;
                        occurrence.contract = Some(materialization.contract);
                    }
                    self.items.push(FlatItemBlueprint::Port {
                        name: internal_name(identity.full),
                        contract,
                        range: declaration.range(),
                        identity,
                    });
                }
                ComponentItem::PortFamily(family) => {
                    let declaration = family.port();
                    let set = support_bindings
                        .boundary_set(family.binder().set())
                        .ok_or_else(|| {
                            hierarchy_error(format!(
                                "Port family `{}` has no resolved complete-exterior binding `{}`",
                                declaration.name(),
                                family.binder().set()
                            ))
                        })?;
                    for side in set.witness().sides() {
                        let boundary = *side.boundary();
                        let member = set.member(&boundary).ok_or_else(|| {
                            hierarchy_error(
                                "complete-exterior witness has no identity-keyed member locator",
                            )
                        })?;
                        let identity = identities
                            .boundary_family_entities
                            .get(&(declaration.name().to_owned(), boundary))
                            .cloned()
                            .ok_or_else(|| {
                                hierarchy_error(format!(
                                    "Port family `{}` member identity was not allocated",
                                    declaration.name()
                                ))
                            })?;
                        let (contract, materialization) = self.component_port_family_syntax(
                            component,
                            family,
                            boundary,
                            member.target(),
                            set.witness().ambient_dimension(),
                        )?;
                        let occurrence =
                            self.physical_ports.get_mut(&identity.full).ok_or_else(|| {
                                hierarchy_error(
                                    "physical Port-family occurrence was not registered",
                                )
                            })?;
                        occurrence.contract = Some(materialization.contract);
                        self.items.push(FlatItemBlueprint::Port {
                            name: internal_name(identity.full),
                            contract,
                            range: family.range(),
                            identity,
                        });
                    }
                }
                ComponentItem::Support(_) | ComponentItem::FieldSlot(_) => {}
                ComponentItem::Representation(declaration) => {
                    let identity = identities.entities[declaration.name()].clone();
                    self.items.push(FlatItemBlueprint::Representation {
                        name: internal_name(identity.full),
                        syntax: declaration.syntax(),
                        range: declaration.range(),
                        identity,
                    });
                }
                ComponentItem::Field(declaration) => {
                    let identity = identities.entities[declaration.name()].clone();
                    let (domain, representation) =
                        rewrite_field_scope(component.file, declaration, scope)?;
                    self.items.push(FlatItemBlueprint::Field {
                        name: internal_name(identity.full),
                        domain,
                        representation,
                        shape: declaration.shape().cloned(),
                        dimension: declaration.dimension().clone(),
                        initial: declaration.initial(),
                        range: declaration.range(),
                        identity,
                    });
                }
                ComponentItem::Clock(declaration) => {
                    let identity = identities.entities[declaration.name()].clone();
                    self.items.push(FlatItemBlueprint::Clock {
                        name: internal_name(identity.full),
                        period: declaration.period(),
                        phase: declaration.phase(),
                        range: declaration.range(),
                        identity,
                    });
                }
                ComponentItem::Relation(declaration) => {
                    let identity = identities.relations[declaration.name()].clone();
                    let (activation, domain, residuals) =
                        rewrite_relation(component.file, declaration, scope)?;
                    self.record_physical_relation_owners(
                        component.file,
                        declaration.range(),
                        identity.entity.full,
                        &residuals,
                    )?;
                    self.items.push(FlatItemBlueprint::Relation {
                        name: internal_name(identity.entity.full),
                        activation,
                        domain,
                        residuals,
                        range: declaration.range(),
                        identity,
                    });
                }
                ComponentItem::RelationFamily(family) => {
                    let declaration = family.relation();
                    let set = support_bindings
                        .boundary_set(family.binder().set())
                        .ok_or_else(|| {
                            hierarchy_error(format!(
                                "Relation family `{}` has no resolved complete-exterior binding `{}`",
                                declaration.name(),
                                family.binder().set()
                            ))
                        })?;
                    for side in set.witness().sides() {
                        let boundary = *side.boundary();
                        let member = set.member(&boundary).ok_or_else(|| {
                            hierarchy_error(
                                "complete-exterior witness has no identity-keyed member locator",
                            )
                        })?;
                        let identity = identities
                            .boundary_family_relations
                            .get(&(declaration.name().to_owned(), boundary))
                            .cloned()
                            .ok_or_else(|| {
                                hierarchy_error(format!(
                                    "Relation family `{}` member identity was not allocated",
                                    declaration.name()
                                ))
                            })?;
                        let active = Some(ActiveBoundaryMember::new(
                            family.binder().member(),
                            boundary,
                        ));
                        let residuals = declaration
                            .residuals()
                            .iter()
                            .map(|expression| {
                                rewrite_expression_with_boundary_member(
                                    component.file,
                                    expression,
                                    scope,
                                    active,
                                )
                            })
                            .collect::<Result<Vec<_>, _>>()?;
                        self.record_physical_relation_owners(
                            component.file,
                            family.range(),
                            identity.entity.full,
                            &residuals,
                        )?;
                        self.items.push(FlatItemBlueprint::Relation {
                            name: internal_name(identity.entity.full),
                            activation: eqiora_lang::ActivationSyntax::Continuous,
                            domain: Some(member.target().to_owned()),
                            residuals,
                            range: family.range(),
                            identity,
                        });
                    }
                }
                ComponentItem::Connection(declaration) => {
                    self.add_connection(
                        declaration,
                        scope,
                        occurrence.instance_path,
                        definition_path(&component.namespace, "component", component.name(), "net"),
                        ConnectionOrigin {
                            instance: SourceLocation::new(
                                occurrence.instance_file,
                                instance.range(),
                            ),
                            bindings: scope.occurrence_bindings().to_vec(),
                            definition_file: component.file.to_owned(),
                        },
                    )?;
                }
                ComponentItem::BoundaryConnection(declaration) => {
                    if let Some(binder) = declaration.binder() {
                        let set = support_bindings.boundary_set(binder.set()).ok_or_else(|| {
                            hierarchy_error(format!(
                                "Connection family has no resolved complete-exterior binding `{}`",
                                binder.set()
                            ))
                        })?;
                        for side in set.witness().sides() {
                            let boundary = *side.boundary();
                            let member = set.member(&boundary).ok_or_else(|| {
                                hierarchy_error(
                                    "complete-exterior witness has no identity-keyed member locator",
                                )
                            })?;
                            self.add_boundary_connection(
                                declaration,
                                scope,
                                Some(ActiveBoundaryMember::new(binder.member(), boundary)),
                                occurrence.instance_path,
                                definition_path(
                                    &component.namespace,
                                    "component",
                                    component.name(),
                                    "net",
                                ),
                                ConnectionOrigin {
                                    instance: SourceLocation::new(
                                        occurrence.instance_file,
                                        instance.range(),
                                    ),
                                    bindings: boundary_family_bindings(
                                        scope.occurrence_bindings(),
                                        occurrence.instance_file,
                                        member.source_range(),
                                    ),
                                    definition_file: component.file.to_owned(),
                                },
                            )?;
                        }
                    } else {
                        self.add_boundary_connection(
                            declaration,
                            scope,
                            None,
                            occurrence.instance_path,
                            definition_path(
                                &component.namespace,
                                "component",
                                component.name(),
                                "net",
                            ),
                            ConnectionOrigin {
                                instance: SourceLocation::new(
                                    occurrence.instance_file,
                                    instance.range(),
                                ),
                                bindings: scope.occurrence_bindings().to_vec(),
                                definition_file: component.file.to_owned(),
                            },
                        )?;
                    }
                }
                ComponentItem::Instance(_) => {}
                _ => {
                    return Err(source_error(
                        codes::LANGUAGE_LOWERING_ERROR,
                        component.file,
                        component.range(),
                        "component item is newer than hierarchy elaboration",
                    ));
                }
            }
        }
        Ok(())
    }

    fn component_port_syntax(
        &mut self,
        component: &ComponentDefinition<'d>,
        declaration: &ComponentPortDecl,
        scope: &Scope,
    ) -> Result<(LoweringPortContract, Option<PhysicalPortMaterialization>), Diagnostic> {
        match declaration.syntax() {
            PortSyntax::Signal {
                direction,
                dimension,
            } => Ok((
                LoweringPortContract::Source(PortSyntax::Signal {
                    direction: *direction,
                    dimension: dimension.clone(),
                }),
                None,
            )),
            PortSyntax::ScalarPhysicalConnector { connector } => {
                let connector = self.elaborator.resolve_connector(
                    &component.namespace,
                    connector,
                    component.file,
                    declaration.range(),
                )?;
                let domain = self.connector_domain(connector, None)?;
                Ok((
                    LoweringPortContract::Source(PortSyntax::ScalarPhysical {
                        domain: domain.internal_name,
                    }),
                    Some(PhysicalPortMaterialization {
                        contract: PhysicalExposureContractIdentity::ScalarPhysical {
                            connector: domain.full_identity,
                        },
                    }),
                ))
            }
            PortSyntax::FieldPhysical { connector, support } => {
                let exact_support = scope.spatial_support(support).ok_or_else(|| {
                    source_error(
                        codes::LANGUAGE_TYPE_ERROR,
                        component.file,
                        declaration.range(),
                        format!("unresolved field-physical boundary support `{support}`"),
                    )
                })?;
                let SpatialSupport::Boundary { dimensions, .. } = exact_support else {
                    return Err(source_error(
                        codes::LANGUAGE_TYPE_ERROR,
                        component.file,
                        declaration.range(),
                        "field-physical Port `over` support must resolve to an exact boundary",
                    ));
                };
                let boundary = scope.symbol(support).ok_or_else(|| {
                    source_error(
                        codes::LANGUAGE_LOWERING_ERROR,
                        component.file,
                        declaration.range(),
                        "resolved boundary support has no flattened Domain symbol",
                    )
                })?;
                let connector = self.elaborator.resolve_connector(
                    &component.namespace,
                    connector,
                    component.file,
                    declaration.range(),
                )?;
                let connector = self.connector_domain(connector, Some(*dimensions))?;
                Ok((
                    LoweringPortContract::BoundaryPhysical {
                        connector: connector.internal_name.clone(),
                        boundary: boundary.internal_name.clone(),
                    },
                    Some(PhysicalPortMaterialization {
                        contract: PhysicalExposureContractIdentity::FieldBoundary {
                            connector: connector.full_identity,
                            boundary: boundary.full_identity,
                        },
                    }),
                ))
            }
            _ => Err(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                component.file,
                declaration.range(),
                "component Port must be an explicit signal or nominal Connector interface",
            )),
        }
    }

    fn component_port_family_syntax(
        &mut self,
        component: &ComponentDefinition<'d>,
        family: &ComponentPortFamilyDecl,
        boundary: FullElaborationIdentity,
        boundary_internal_name: &str,
        dimensions: usize,
    ) -> Result<(LoweringPortContract, PhysicalPortMaterialization), Diagnostic> {
        let PortSyntax::FieldPhysical { connector, .. } = family.port().syntax() else {
            return Err(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                component.file,
                family.range(),
                "boundary Port family requires a field-physical Connector",
            ));
        };
        let connector = self.elaborator.resolve_connector(
            &component.namespace,
            connector,
            component.file,
            family.range(),
        )?;
        let connector = self.connector_domain(connector, Some(dimensions))?;
        Ok((
            LoweringPortContract::BoundaryPhysical {
                connector: connector.internal_name.clone(),
                boundary: boundary_internal_name.to_owned(),
            },
            PhysicalPortMaterialization {
                contract: PhysicalExposureContractIdentity::FieldBoundary {
                    connector: connector.full_identity,
                    boundary,
                },
            },
        ))
    }

    fn materialize_model_items(
        &mut self,
        scope: &Scope,
        identities: &ScopeIdentities,
    ) -> Result<(), Diagnostic> {
        let model = self.model.clone();
        for item in model.items() {
            match item {
                Item::Domain(declaration) => {
                    let identity = identities.entities[declaration.name()].clone();
                    let syntax = match declaration.syntax() {
                        DomainSyntax::CartesianBox(bounds) => {
                            DomainSyntax::CartesianBox(bounds.clone())
                        }
                        DomainSyntax::Boundary { parent, axis, side } => {
                            let parent = resolve_local_kind(
                                self.model.file,
                                declaration.range(),
                                scope,
                                parent,
                                |kind| matches!(kind, SymbolKind::Domain),
                                "boundary parent Domain",
                            )?;
                            DomainSyntax::Boundary {
                                parent: parent.internal_name.clone(),
                                axis: *axis,
                                side: *side,
                            }
                        }
                        DomainSyntax::ScalarPhysical {
                            across_dimension,
                            through_dimension,
                        } => DomainSyntax::ScalarPhysical {
                            across_dimension: across_dimension.clone(),
                            through_dimension: through_dimension.clone(),
                        },
                        _ => {
                            return Err(source_error(
                                codes::LANGUAGE_LOWERING_ERROR,
                                self.model.file,
                                declaration.range(),
                                "Domain syntax is newer than hierarchy elaboration",
                            ));
                        }
                    };
                    self.items.push(FlatItemBlueprint::Domain {
                        name: internal_name(identity.full),
                        contract: LoweringDomainContract::Source(syntax),
                        range: declaration.range(),
                        identity,
                    });
                }
                Item::Representation(declaration) => {
                    let identity = identities.entities[declaration.name()].clone();
                    self.items.push(FlatItemBlueprint::Representation {
                        name: internal_name(identity.full),
                        syntax: declaration.syntax(),
                        range: declaration.range(),
                        identity,
                    });
                }
                Item::Field(declaration) => {
                    let identity = identities.entities[declaration.name()].clone();
                    let (domain, representation) =
                        rewrite_field_scope(self.model.file, declaration, scope)?;
                    self.items.push(FlatItemBlueprint::Field {
                        name: internal_name(identity.full),
                        domain,
                        representation,
                        shape: declaration.shape().cloned(),
                        dimension: declaration.dimension().clone(),
                        initial: declaration.initial(),
                        range: declaration.range(),
                        identity,
                    });
                }
                Item::Parameter(declaration) => {
                    let identity = identities.entities[declaration.name()].clone();
                    self.items.push(FlatItemBlueprint::Parameter {
                        name: internal_name(identity.full),
                        dimension: declaration.dimension().clone(),
                        value: declaration.initial(),
                        range: declaration.range(),
                        identity,
                    });
                }
                Item::Port(declaration) => {
                    let identity = identities.entities[declaration.name()].clone();
                    let syntax = rewrite_model_port(
                        self.model.file,
                        declaration.syntax(),
                        declaration.range(),
                        scope,
                    )?;
                    self.items.push(FlatItemBlueprint::Port {
                        name: internal_name(identity.full),
                        contract: LoweringPortContract::Source(syntax),
                        range: declaration.range(),
                        identity,
                    });
                }
                Item::Clock(declaration) => {
                    let identity = identities.entities[declaration.name()].clone();
                    self.items.push(FlatItemBlueprint::Clock {
                        name: internal_name(identity.full),
                        period: declaration.period(),
                        phase: declaration.phase(),
                        range: declaration.range(),
                        identity,
                    });
                }
                Item::Relation(declaration) => {
                    let identity = identities.relations[declaration.name()].clone();
                    let (activation, domain, residuals) =
                        rewrite_relation(self.model.file, declaration, scope)?;
                    self.record_physical_relation_owners(
                        self.model.file,
                        declaration.range(),
                        identity.entity.full,
                        &residuals,
                    )?;
                    self.items.push(FlatItemBlueprint::Relation {
                        name: internal_name(identity.entity.full),
                        activation,
                        domain,
                        residuals,
                        range: declaration.range(),
                        identity,
                    });
                }
                Item::Connection(declaration) => {
                    self.add_connection(
                        declaration,
                        scope,
                        &self.root_path.clone(),
                        definition_path(&self.model.namespace, "model", self.model.name(), "net"),
                        ConnectionOrigin {
                            instance: SourceLocation::new(self.model.file, self.model.range()),
                            bindings: Vec::new(),
                            definition_file: self.model.file.to_owned(),
                        },
                    )?;
                }
                Item::BoundaryConnection(declaration) => {
                    self.add_boundary_connection(
                        declaration,
                        scope,
                        None,
                        &self.root_path.clone(),
                        definition_path(&self.model.namespace, "model", self.model.name(), "net"),
                        ConnectionOrigin {
                            instance: SourceLocation::new(self.model.file, self.model.range()),
                            bindings: Vec::new(),
                            definition_file: self.model.file.to_owned(),
                        },
                    )?;
                }
                Item::Boundary(declaration) => {
                    let ports =
                        resolve_visible_ports(self.model.file, declaration.port_paths(), scope)?
                            .into_iter()
                            .map(|symbol| symbol.internal_name.clone())
                            .collect();
                    self.items.push(FlatItemBlueprint::Boundary {
                        ports,
                        range: declaration.range(),
                    });
                }
                Item::Let(_) | Item::Instance(_) => {}
                _ => {
                    return Err(source_error(
                        codes::LANGUAGE_LOWERING_ERROR,
                        self.model.file,
                        self.model.range(),
                        "model item is newer than hierarchy elaboration",
                    ));
                }
            }
        }
        Ok(())
    }

    fn add_connection(
        &mut self,
        declaration: &ConnectionDecl,
        scope: &Scope,
        instance_path: &InstancePath,
        declaration_path: Vec<String>,
        origin: ConnectionOrigin,
    ) -> Result<(), Diagnostic> {
        let ports = resolve_ports(
            &origin.definition_file,
            declaration.range(),
            declaration.port_paths(),
            scope,
        )?;
        self.add_resolved_connection(
            declaration.syntax(),
            ports,
            declaration.range(),
            instance_path,
            declaration_path,
            origin,
        )
    }

    fn add_boundary_connection(
        &mut self,
        declaration: &BoundaryConnectionDecl,
        scope: &Scope,
        active: Option<ActiveBoundaryMember<'_>>,
        instance_path: &InstancePath,
        declaration_path: Vec<String>,
        origin: ConnectionOrigin,
    ) -> Result<(), Diagnostic> {
        let ports = declaration
            .ports()
            .iter()
            .map(|reference| {
                resolve_boundary_port_reference(&origin.definition_file, reference, scope, active)
            })
            .collect::<Result<Vec<_>, _>>()?;
        if ports.len() < 2 {
            return Err(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                &origin.definition_file,
                declaration.range(),
                "Connection requires at least two visible Ports",
            ));
        }
        self.add_resolved_connection(
            declaration.syntax(),
            ports,
            declaration.range(),
            instance_path,
            declaration_path,
            origin,
        )
    }

    fn add_resolved_connection(
        &mut self,
        syntax: ConnectionSyntax,
        mut ports: Vec<&FlatSymbol>,
        range: eqiora_lang::TextRange,
        instance_path: &InstancePath,
        declaration_path: Vec<String>,
        origin: ConnectionOrigin,
    ) -> Result<(), Diagnostic> {
        match syntax {
            ConnectionSyntax::Conserving | ConnectionSyntax::SpatialPeriodic => {
                ports.sort_unstable_by_key(|port| port.full_identity);
            }
            ConnectionSyntax::Signal => {
                if let Some((_, inputs)) = ports.split_first_mut() {
                    inputs.sort_unstable_by_key(|port| port.full_identity);
                }
            }
        }
        let path_display = declaration_path.join("/");
        let path = DeclarationPath::with_limits(declaration_path, self.elaborator.limits.identity)
            .map_err(|diagnostic| {
                source_error(
                    codes::LANGUAGE_LOWERING_ERROR,
                    &origin.definition_file,
                    range,
                    format!(
                        "cannot identify Connection at declaration path `{path_display}`: {}",
                        diagnostic.message()
                    ),
                )
            })?;
        let source = EntitySourceOrigin {
            definition: SourceLocation::new(&origin.definition_file, range),
            instance: origin.instance,
            bindings: origin.bindings,
        };
        if matches!(
            syntax,
            ConnectionSyntax::Conserving | ConnectionSyntax::SpatialPeriodic
        ) && ports
            .first()
            .is_some_and(|port| self.physical_ports.contains_key(&port.full_identity))
        {
            if let Some(non_physical) = ports
                .iter()
                .find(|port| !self.physical_ports.contains_key(&port.full_identity))
            {
                return Err(source_error(
                    codes::LANGUAGE_TYPE_ERROR,
                    &origin.definition_file,
                    range,
                    format!(
                        "physical Connection cannot include non-physical Port `{}`",
                        non_physical.display_name
                    ),
                ));
            }
            if syntax == ConnectionSyntax::SpatialPeriodic {
                if ports.len() != 2 {
                    return Err(source_error(
                        codes::LANGUAGE_TYPE_ERROR,
                        &origin.definition_file,
                        range,
                        "spatial-periodic Connection requires exactly two field-physical Ports",
                    ));
                }
                if ports.iter().any(|port| {
                    self.spatial_periodic_ports.contains(&port.full_identity)
                        || self.physical_connections.iter().any(|fragment| {
                            fragment.topology.members().contains(&port.full_identity)
                        })
                }) {
                    return Err(source_error(
                        codes::LANGUAGE_TYPE_ERROR,
                        &origin.definition_file,
                        range,
                        "spatial-periodic Port already belongs to another physical Connection",
                    ));
                }
                self.spatial_periodic_ports
                    .extend(ports.iter().map(|port| port.full_identity));
                let key = ElaborationKey::anonymous_connection_with_limits(
                    self.namespace.clone(),
                    instance_path.clone(),
                    path,
                    ports.iter().map(|port| port.full_identity),
                    self.elaborator.limits.identity,
                )?;
                let full = key.full_identity()?;
                self.items.push(FlatItemBlueprint::Connection {
                    syntax,
                    ports: ports
                        .into_iter()
                        .map(|port| port.internal_name.clone())
                        .collect(),
                    range,
                    identity: ConnectionIdentity {
                        key,
                        full,
                        origins: vec![source],
                    },
                });
                return Ok(());
            }
            let topology = ConnectionFragment::try_new(
                ports.iter().map(|port| port.full_identity),
                self.elaborator.limits.connection_sets,
            )
            .map_err(|error| {
                source_error(
                    codes::LANGUAGE_LOWERING_ERROR,
                    &origin.definition_file,
                    range,
                    format!("cannot stage physical Connection fragment: {error}"),
                )
            })?;
            self.physical_connections.push(StagedPhysicalConnection {
                topology,
                origin: PhysicalConnectionOrigin {
                    declaration_path: path,
                    instance_path: instance_path.clone(),
                    source,
                },
            });
            return Ok(());
        }
        let key = ElaborationKey::anonymous_connection_with_limits(
            self.namespace.clone(),
            instance_path.clone(),
            path,
            ports.iter().map(|port| port.full_identity),
            self.elaborator.limits.identity,
        )?;
        let full = key.full_identity()?;
        self.items.push(FlatItemBlueprint::Connection {
            syntax,
            ports: ports
                .into_iter()
                .map(|port| port.internal_name.clone())
                .collect(),
            range,
            identity: ConnectionIdentity {
                key,
                full,
                origins: vec![source],
            },
        });
        Ok(())
    }

    fn finalize_physical_connections(&mut self) -> Result<(), Diagnostic> {
        if self.physical_ports.is_empty() && self.physical_connections.is_empty() {
            return Ok(());
        }
        for fragment in &self.physical_connections {
            if fragment
                .topology
                .members()
                .iter()
                .any(|member| self.spatial_periodic_ports.contains(member))
            {
                return Err(source_error(
                    codes::LANGUAGE_TYPE_ERROR,
                    self.model.file,
                    self.model.range(),
                    "one field-physical Port cannot belong to both ordinary and spatial-periodic Connections",
                ));
            }
            let boundary_members = fragment
                .topology
                .members()
                .iter()
                .filter_map(|identity| {
                    self.physical_ports.get(identity).and_then(|port| {
                        let Some(PhysicalExposureContractIdentity::FieldBoundary {
                            connector,
                            boundary,
                        }) = port.contract
                        else {
                            return None;
                        };
                        Some((*identity, connector, boundary))
                    })
                })
                .collect::<Vec<_>>();
            if boundary_members.is_empty() {
                continue;
            }
            if boundary_members.len() != fragment.topology.members().len() {
                return Err(source_error(
                    codes::LANGUAGE_TYPE_ERROR,
                    self.model.file,
                    self.model.range(),
                    "conserving Connection cannot mix scalar and field-physical Ports",
                ));
            }
            let mut contracts = Vec::with_capacity(boundary_members.len());
            let mut metric_validation_deferred = false;
            for (_, connector, boundary) in &boundary_members {
                let embedding = self.boundary_embeddings.get(boundary).ok_or_else(|| {
                    source_error(
                        codes::LANGUAGE_TYPE_ERROR,
                        self.model.file,
                        self.model.range(),
                        "field-physical Port boundary has no Cartesian embedding recipe",
                    )
                })?;
                let parent = *self.boundary_parents.get(boundary).ok_or_else(|| {
                    source_error(
                        codes::LANGUAGE_TYPE_ERROR,
                        self.model.file,
                        self.model.range(),
                        "field-physical Port boundary has no exact parent identity",
                    )
                })?;
                if let Some(embedding) = embedding {
                    contracts.push(BoundaryPhysicalPortContract {
                        connector: *connector,
                        boundary: *boundary,
                        parent,
                        embedding: embedding.clone(),
                    });
                } else {
                    metric_validation_deferred = true;
                }
            }
            if !metric_validation_deferred {
                validate_boundary_physical_connection(&contracts).map_err(|violation| {
                    source_error(
                        codes::LANGUAGE_TYPE_ERROR,
                        self.model.file,
                        self.model.range(),
                        format!(
                            "field-physical Connection is incompatible before topology normalization: {violation:?}"
                        ),
                    )
                })?;
            }
        }
        let endpoints = self
            .physical_ports
            .iter()
            .filter(|(identity, _)| !self.spatial_periodic_ports.contains(identity))
            .map(|(identity, occurrence)| {
                OccurrencePhysicalEndpoint::new(
                    *identity,
                    occurrence.exposure_candidate,
                    self.physical_owner_relations.contains_key(identity),
                )
            })
            .collect::<Vec<_>>();
        let fragments = self
            .physical_connections
            .iter()
            .map(|fragment| {
                OccurrenceConnectionFragment::new(
                    fragment.topology.clone(),
                    fragment.origin.instance_path.clone(),
                )
            })
            .collect::<Vec<_>>();
        let normalized = normalize_occurrence_connections(
            &endpoints,
            &fragments,
            self.elaborator.limits.connection_sets,
        )
        .map_err(|error| {
            source_error(
                codes::LANGUAGE_TYPE_ERROR,
                self.model.file,
                self.model.range(),
                format!("invalid occurrence-level physical connection closure: {error}"),
            )
        })?;

        let projection_count = normalized
            .sets()
            .iter()
            .try_fold(0_usize, |count, set| {
                count.checked_add(set.topology().eliminated_exposures().len())
            })
            .ok_or_else(|| hierarchy_error("physical exposure projection count overflows usize"))?;
        let projection_limits = self.elaborator.limits.physical_exposures;
        if projection_count > projection_limits.max_projections {
            return Err(hierarchy_error(format!(
                "physical exposure projections total {projection_count}, exceeding the {} limit",
                projection_limits.max_projections
            )));
        }
        self.physical_exposures
            .try_reserve_exact(projection_count)
            .map_err(|_| hierarchy_error("cannot reserve physical exposure projections"))?;
        let mut cut_graph = ExposureCutIndex::new(
            self.physical_ports.keys().copied(),
            self.physical_ports.len(),
            &fragments,
        )?;
        let mut projection_memberships = 0_usize;

        let mut exposure_connections = BTreeMap::new();
        for (set_index, set) in normalized.sets().iter().enumerate() {
            let topology = set.topology();
            let owner_fragment = set
                .witness()
                .lca_owner_candidate_fragment_indices()
                .iter()
                .copied()
                .min_by(|left, right| {
                    compare_physical_connection_origins(
                        &self.physical_connections[*left].origin,
                        &self.physical_connections[*right].origin,
                    )
                })
                .expect("occurrence normalization proves one explicit LCA fragment");
            let owner = &self.physical_connections[owner_fragment].origin;
            debug_assert_eq!(owner.instance_path, *topology.owner_instance_path());
            let key = ElaborationKey::anonymous_connection_with_limits(
                self.namespace.clone(),
                topology.owner_instance_path().clone(),
                owner.declaration_path.clone(),
                topology.retained_members().iter().copied(),
                self.elaborator.limits.identity,
            )?;
            let full = key.full_identity()?;
            let origins = set
                .witness()
                .contributing_fragment_indices()
                .iter()
                .map(|index| self.physical_connections[*index].origin.source.clone())
                .collect::<Vec<_>>();
            self.items.push(FlatItemBlueprint::Connection {
                syntax: ConnectionSyntax::Conserving,
                ports: topology
                    .retained_members()
                    .iter()
                    .map(|member| self.physical_ports[member].identity.full)
                    .map(internal_name)
                    .collect(),
                range: owner.source.definition.range,
                identity: ConnectionIdentity { key, full, origins },
            });
            for exposure in topology.eliminated_exposures() {
                if exposure_connections.insert(*exposure, full).is_some() {
                    return Err(hierarchy_error(format!(
                        "physical exposure {exposure} projects to more than one canonical Connection"
                    )));
                }
                let occurrence = &self.physical_ports[exposure];
                let interior = cut_graph.derive(
                    *exposure,
                    &occurrence.instance_path,
                    topology.retained_members(),
                    &fragments,
                    projection_limits.max_traversal_memberships,
                )?;
                if interior.is_empty() || interior.len() == topology.retained_members().len() {
                    return Err(hierarchy_error(format!(
                        "physical exposure `{}` does not define a nonempty proper occurrence cut",
                        occurrence.display_name
                    )));
                }
                if interior.len() > projection_limits.max_members_per_cut {
                    return Err(hierarchy_error(format!(
                        "physical exposure `{}` cut has {} members, exceeding the {} limit",
                        occurrence.display_name,
                        interior.len(),
                        projection_limits.max_members_per_cut
                    )));
                }
                projection_memberships = projection_memberships
                    .checked_add(interior.len())
                    .ok_or_else(|| {
                        hierarchy_error("physical exposure cut membership count overflows usize")
                    })?;
                if projection_memberships > projection_limits.max_memberships {
                    return Err(hierarchy_error(format!(
                        "physical exposure cuts total {projection_memberships} memberships, exceeding the {} limit",
                        projection_limits.max_memberships
                    )));
                }
                let contract = occurrence.contract.ok_or_else(|| {
                    hierarchy_error(format!(
                        "physical exposure `{}` has no closed nominal contract",
                        occurrence.display_name
                    ))
                })?;
                self.physical_exposures
                    .push(PhysicalExposureProjectionBlueprint {
                        selector: occurrence.display_name.clone(),
                        exposure: occurrence.identity.clone(),
                        connection: full,
                        interior,
                        contract,
                    });
            }
            debug_assert!(
                normalized
                    .exposure_witnesses()
                    .iter()
                    .filter(|witness| witness.connection_set_index() == set_index)
                    .all(|witness| exposure_connections.contains_key(&witness.exposure()))
            );
        }

        self.items.retain(|item| match item {
            FlatItemBlueprint::Port { identity, .. } => {
                !exposure_connections.contains_key(&identity.full)
            }
            _ => true,
        });
        for exposure in exposure_connections.into_keys() {
            let occurrence = &self.physical_ports[&exposure];
            let removed = self.display_symbols.remove(&occurrence.display_name);
            if !matches!(removed, Some(identity) if identity.full == exposure) {
                return Err(hierarchy_error(format!(
                    "physical exposure `{}` has no exact display-symbol entry",
                    occurrence.display_name
                )));
            }
        }
        self.physical_exposures.sort_unstable_by(|left, right| {
            left.selector
                .cmp(&right.selector)
                .then_with(|| left.exposure.full.cmp(&right.exposure.full))
        });
        Ok(())
    }
}

fn compare_physical_connection_origins(
    left: &PhysicalConnectionOrigin,
    right: &PhysicalConnectionOrigin,
) -> core::cmp::Ordering {
    left.declaration_path
        .cmp(&right.declaration_path)
        .then_with(|| {
            left.source
                .definition
                .file
                .cmp(&right.source.definition.file)
        })
        .then_with(|| {
            left.source
                .definition
                .range
                .start()
                .cmp(&right.source.definition.range.start())
        })
        .then_with(|| {
            left.source
                .definition
                .range
                .end()
                .cmp(&right.source.definition.range.end())
        })
}

fn internal_name(identity: FullElaborationIdentity) -> String {
    format!("e{identity}")
}

fn child_instance_path(
    parent: &InstancePath,
    child: &str,
    limits: ElaborationIdentityLimits,
) -> Result<InstancePath, Diagnostic> {
    InstancePath::with_limits(
        parent
            .segments()
            .iter()
            .map(String::as_str)
            .chain(core::iter::once(child)),
        limits,
    )
}

fn display_child(parent: &str, child: &str) -> String {
    if parent.is_empty() {
        child.to_owned()
    } else {
        format!("{parent}.{child}")
    }
}

fn boundary_family_display(parent: &str, family: &str, axis: usize, side: BoundarySide) -> String {
    let side = match side {
        BoundarySide::Lower => "lower",
        BoundarySide::Upper => "upper",
    };
    format!("{}[axis={axis},side={side}]", display_child(parent, family))
}

fn boundary_family_bindings(
    base: &[SourceLocation],
    file: &str,
    member_range: eqiora_lang::TextRange,
) -> Vec<SourceLocation> {
    let mut bindings = base.to_vec();
    bindings.push(SourceLocation::new(file, member_range));
    normalize_binding_locations(&mut bindings);
    bindings
}

fn definition_path(
    namespace: &DefinitionNamespace,
    family: &str,
    definition: &str,
    member: &str,
) -> Vec<String> {
    let mut path = namespace.declaration_prefix();
    path.extend([family.to_owned(), definition.to_owned(), member.to_owned()]);
    path
}
