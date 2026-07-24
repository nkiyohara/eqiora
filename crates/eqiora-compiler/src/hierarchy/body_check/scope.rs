use std::collections::{BTreeMap, BTreeSet};

use eqiora_core::diagnostic::codes;
use eqiora_core::{Diagnostic, DimExponents, ValueShape};
use eqiora_lang::{
    BoundaryConnectionDecl, BoundaryFamilyBinderSyntax, BoundaryPairingSyntax,
    BoundaryPortReferenceSyntax, BoundaryPortSelectorSyntax, ComponentItem, ComponentPortDecl,
    ComponentPortFamilyDecl, ConnectionDecl, ConnectionSyntax, ConnectorSyntax, Expr, FieldDecl,
    FrameSyntax, InstanceDecl, NamePath, PortDecl, PortSyntax, SignalDirectionSyntax,
    SupportSlotSyntax, TextRange, ValueShapeSyntax, VisibilitySyntax,
};
use eqiora_schema::kernel::scalar_connection::{
    ScalarConnectionKind, ScalarConnectionViolation, ScalarPortContract, validate_scalar_connection,
};
use eqiora_schema::kernel::typing::{ExpressionType, SpatialSupport};
use eqiora_schema::kernel::{
    BoundaryPairing, BoundaryPhysicalConnector, SignalDirection, ValueFrame,
};

use crate::connection_sets::{ConnectionFragment, ConnectionSetError, ConnectionSetLimits};
use crate::diagnostics::source_error;
use crate::dimensions::lower_dimension;

use super::super::preflight::{
    ComponentDefinition, DefinitionKey, DefinitionNamespace, Elaborator,
};
use super::{PhysicalConnectionFragment, ResolvedPhysicalEndpoint};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum PhysicalNominal {
    Connector(DefinitionKey),
    BoundaryConnector {
        definition: DefinitionKey,
        shape: ValueShape,
    },
    ModelDomain(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum PortContract {
    Signal {
        direction: SignalDirectionSyntax,
        dimension: DimExponents,
    },
    ConservingMarker {
        dimension: DimExponents,
    },
    Physical {
        nominal: PhysicalNominal,
        across_dimension: DimExponents,
        through_dimension: DimExponents,
    },
    BoundaryPhysical {
        nominal: PhysicalNominal,
        connector: BoundaryPhysicalConnector,
        support: SpatialSupport<String>,
    },
}

#[derive(Debug, Clone)]
pub(super) struct BoundaryPortFamilyContract {
    binder: BoundaryFamilyBinderSyntax,
    port: PortContract,
}

impl BoundaryPortFamilyContract {
    fn selected(
        &self,
        file: &str,
        port: &NamePath,
        selector: &BoundaryPortSelectorSyntax,
        active: &BoundaryFamilyScope,
    ) -> Result<PortContract, Diagnostic> {
        if selector.member() != self.binder.member() {
            return Err(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                file,
                selector.range(),
                format!(
                    "boundary selector member `{}` does not match Port family member `{}`",
                    selector.member(),
                    self.binder.member()
                ),
            ));
        }
        if selector.target() != active.binder.member() {
            return Err(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                file,
                selector.range(),
                format!(
                    "boundary selector target `{}` does not name active family member `{}`",
                    selector.target(),
                    active.binder.member()
                ),
            ));
        }
        self.selected_on_support(file, port, selector, active.support.clone())
    }

    fn selected_on_support(
        &self,
        file: &str,
        port: &NamePath,
        selector: &BoundaryPortSelectorSyntax,
        support: SpatialSupport<String>,
    ) -> Result<PortContract, Diagnostic> {
        if selector.member() != self.binder.member() {
            return Err(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                file,
                selector.range(),
                format!(
                    "boundary selector member `{}` does not match Port family member `{}`",
                    selector.member(),
                    self.binder.member()
                ),
            ));
        }
        let PortContract::BoundaryPhysical {
            nominal, connector, ..
        } = &self.port
        else {
            return Err(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                file,
                port.range(),
                "only a field-physical Port can be selected as a boundary family",
            ));
        };
        Ok(PortContract::BoundaryPhysical {
            nominal: nominal.clone(),
            connector: connector.clone(),
            support,
        })
    }
}

/// One lexically scoped, identity-parametric member of a complete exterior.
///
/// The synthetic identity is used only while checking a reusable definition;
/// occurrence expansion replaces it with one exact bound boundary identity.
#[derive(Debug, Clone)]
pub(super) struct BoundaryFamilyScope {
    binder: BoundaryFamilyBinderSyntax,
    support: SpatialSupport<String>,
}

impl BoundaryFamilyScope {
    pub(super) const fn binder(&self) -> &BoundaryFamilyBinderSyntax {
        &self.binder
    }

    pub(super) fn support(&self) -> SpatialSupport<String> {
        self.support.clone()
    }
}

impl PortContract {
    pub(super) fn scalar_type(&self) -> Option<ExpressionType<String>> {
        match self {
            Self::Signal { dimension, .. } | Self::ConservingMarker { dimension } => {
                Some(ExpressionType::scalar(*dimension, None))
            }
            Self::Physical { .. } => None,
            Self::BoundaryPhysical { .. } => None,
        }
    }

    pub(super) const fn is_physical(&self) -> bool {
        matches!(self, Self::Physical { .. } | Self::BoundaryPhysical { .. })
    }
}

#[derive(Debug, Clone)]
pub(super) enum DomainContract {
    Spatial(SpatialSupport<String>),
    Physical {
        across_dimension: DimExponents,
        through_dimension: DimExponents,
    },
}

impl DomainContract {
    pub(super) fn spatial_support(&self) -> Option<SpatialSupport<String>> {
        match self {
            Self::Spatial(support) => Some(support.clone()),
            Self::Physical { .. } => None,
        }
    }
}

#[derive(Debug, Clone)]
pub(super) enum SymbolContract {
    Domain(DomainContract),
    Support(SpatialSupport<String>),
    Representation,
    Field(ExpressionType<String>),
    Parameter(ExpressionType<String>),
    Port(PortContract),
    PortFamily(BoundaryPortFamilyContract),
    CompleteExterior { parent: SpatialSupport<String> },
    Clock,
    Relation,
}

pub(super) struct DefinitionScope<'e, 'd> {
    pub(super) elaborator: &'e Elaborator<'d>,
    pub(super) namespace: DefinitionNamespace,
    pub(super) file: &'d str,
    pub(super) symbols: BTreeMap<String, SymbolContract>,
    pub(super) children: BTreeMap<String, ComponentDefinition<'d>>,
    pub(super) child_instances: BTreeMap<String, &'d InstanceDecl>,
}

impl<'e, 'd> DefinitionScope<'e, 'd> {
    pub(super) fn new(
        elaborator: &'e Elaborator<'d>,
        namespace: DefinitionNamespace,
        file: &'d str,
    ) -> Self {
        Self {
            elaborator,
            namespace,
            file,
            symbols: BTreeMap::new(),
            children: BTreeMap::new(),
            child_instances: BTreeMap::new(),
        }
    }

    pub(super) fn spatial_support(&self, name: &str) -> Option<SpatialSupport<String>> {
        match self.symbols.get(name) {
            Some(SymbolContract::Domain(contract)) => contract.spatial_support(),
            Some(SymbolContract::Support(support)) => Some(support.clone()),
            _ => None,
        }
    }

    pub(super) fn boundary_family_scope(
        &self,
        binder: &BoundaryFamilyBinderSyntax,
    ) -> Result<BoundaryFamilyScope, Diagnostic> {
        let Some(SymbolContract::CompleteExterior { parent }) = self.symbols.get(binder.set())
        else {
            return Err(self.wrong_local_kind(
                binder.range(),
                binder.set(),
                "complete-exterior support set",
            ));
        };
        let SpatialSupport::Volume { domain, dimensions } = parent else {
            return Err(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                self.file,
                binder.range(),
                format!(
                    "complete-exterior support set `{}` requires a volume parent",
                    binder.set()
                ),
            ));
        };
        Ok(BoundaryFamilyScope {
            binder: binder.clone(),
            support: SpatialSupport::Boundary {
                domain: synthetic_boundary_member_identity(binder),
                parent: domain.clone(),
                dimensions: *dimensions,
            },
        })
    }

    pub(super) fn resolve_symbol(&self, path: &NamePath) -> Result<SymbolContract, Diagnostic> {
        let segments = path.segments().collect::<Vec<_>>();
        match segments.as_slice() {
            [name] => self
                .symbols
                .get(*name)
                .cloned()
                .ok_or_else(|| unresolved(self.file, path.range(), name, "expression symbol")),
            [instance, member] => {
                let Some(child) = self.children.get(*instance) else {
                    return Err(self.invalid_public_port_selection(path));
                };
                let port = child
                    .declaration
                    .items()
                    .iter()
                    .find_map(|item| match item {
                        ComponentItem::Port(port)
                            if port.name() == *member
                                && port.visibility() == VisibilitySyntax::Public =>
                        {
                            Some(port)
                        }
                        _ => None,
                    })
                    .ok_or_else(|| self.invalid_public_port_selection(path))?;
                component_port_contract(self.elaborator, child, port)
                    .map(SymbolContract::Port)
                    .map_err(|mut errors| {
                        errors.pop().unwrap_or_else(|| {
                            source_error(
                                codes::LANGUAGE_LOWERING_ERROR,
                                self.file,
                                path.range(),
                                "child Port contract validation failed without a diagnostic",
                            )
                        })
                    })
            }
            _ => Err(self.invalid_public_port_selection(path)),
        }
    }

    pub(super) fn resolve_port(&self, path: &NamePath) -> Result<PortContract, Diagnostic> {
        match self.resolve_symbol(path)? {
            SymbolContract::Port(contract) => Ok(contract),
            _ => Err(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                self.file,
                path.range(),
                format!("`{path}` does not select a visible Port in this scope"),
            )),
        }
    }

    pub(super) fn resolve_boundary_port_reference(
        &self,
        reference: &BoundaryPortReferenceSyntax,
        active: &BoundaryFamilyScope,
    ) -> Result<PortContract, Diagnostic> {
        let Some(selector) = reference.selector() else {
            return Err(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                self.file,
                reference.port().range(),
                format!(
                    "boundary-family Port `{}` requires an exact `[member = target]` selector",
                    reference.port()
                ),
            ));
        };
        let family = self.resolve_boundary_port_family(reference.port())?;
        self.validate_boundary_family_mapping(reference.port(), &family, active, selector.range())?;
        family.selected(self.file, reference.port(), selector, active)
    }

    pub(super) fn resolve_boundary_port_selection(
        &self,
        port: &NamePath,
        selector: &BoundaryPortSelectorSyntax,
        active: &BoundaryFamilyScope,
    ) -> Result<PortContract, Diagnostic> {
        let family = self.resolve_boundary_port_family(port)?;
        self.validate_boundary_family_mapping(port, &family, active, selector.range())?;
        family.selected(self.file, port, selector, active)
    }

    fn validate_boundary_family_mapping(
        &self,
        port: &NamePath,
        family: &BoundaryPortFamilyContract,
        active: &BoundaryFamilyScope,
        range: TextRange,
    ) -> Result<(), Diagnostic> {
        let segments = port.segments().collect::<Vec<_>>();
        match segments.as_slice() {
            [_] if family.binder.set() == active.binder.set() => Ok(()),
            [_] => Err(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                self.file,
                range,
                format!(
                    "local Port family `{port}` belongs to complete exterior `{}`, not active exterior `{}`",
                    family.binder.set(),
                    active.binder.set()
                ),
            )),
            [instance, _] => {
                let Some(occurrence) = self.child_instances.get(*instance) else {
                    return Err(self.invalid_public_port_selection(port));
                };
                if occurrence.support_bindings().iter().any(|binding| {
                    binding.slot() == family.binder.set() && binding.target() == active.binder.set()
                }) {
                    Ok(())
                } else {
                    Err(source_error(
                        codes::LANGUAGE_TYPE_ERROR,
                        self.file,
                        range,
                        format!(
                            "child Port family `{port}` is not forwarded from active complete exterior `{}`",
                            active.binder.set()
                        ),
                    ))
                }
            }
            _ => Err(self.invalid_public_port_selection(port)),
        }
    }

    fn resolve_boundary_port_family(
        &self,
        path: &NamePath,
    ) -> Result<BoundaryPortFamilyContract, Diagnostic> {
        let segments = path.segments().collect::<Vec<_>>();
        match segments.as_slice() {
            [name] => match self.symbols.get(*name) {
                Some(SymbolContract::PortFamily(contract)) => Ok(contract.clone()),
                _ => Err(source_error(
                    codes::LANGUAGE_TYPE_ERROR,
                    self.file,
                    path.range(),
                    format!("`{path}` does not select a boundary Port family in this scope"),
                )),
            },
            [instance, member] => {
                let child = self
                    .children
                    .get(*instance)
                    .ok_or_else(|| self.invalid_public_port_selection(path))?;
                let family = child
                    .declaration
                    .items()
                    .iter()
                    .find_map(|item| match item {
                        ComponentItem::PortFamily(family)
                            if family.port().name() == *member
                                && family.port().visibility() == VisibilitySyntax::Public =>
                        {
                            Some(family)
                        }
                        _ => None,
                    })
                    .ok_or_else(|| self.invalid_public_port_selection(path))?;
                let support = synthetic_component_family_support(child, family.binder())?;
                component_port_family_contract(self.elaborator, child, family, support).map_err(
                    |mut errors| {
                        errors.pop().unwrap_or_else(|| {
                            source_error(
                                codes::LANGUAGE_LOWERING_ERROR,
                                self.file,
                                path.range(),
                                "child Port-family contract validation failed without a diagnostic",
                            )
                        })
                    },
                )
            }
            _ => Err(self.invalid_public_port_selection(path)),
        }
    }

    pub(super) fn wrong_local_kind(
        &self,
        range: TextRange,
        name: &str,
        expected: &str,
    ) -> Diagnostic {
        if self.symbols.contains_key(name) || self.children.contains_key(name) {
            source_error(
                codes::LANGUAGE_TYPE_ERROR,
                self.file,
                range,
                format!("`{name}` is not a {expected}"),
            )
        } else {
            unresolved(self.file, range, name, expected)
        }
    }

    fn invalid_public_port_selection(&self, path: &NamePath) -> Diagnostic {
        source_error(
            codes::LANGUAGE_TYPE_ERROR,
            self.file,
            path.range(),
            format!("qualified name `{path}` does not select a public Port in this scope"),
        )
    }
}

pub(in crate::hierarchy) fn field_expression_type<I>(
    file: &str,
    declaration: &FieldDecl,
    support: Option<SpatialSupport<I>>,
) -> Result<ExpressionType<I>, Diagnostic> {
    let inferred = field_value_type(
        file,
        declaration.range(),
        declaration.dimension(),
        declaration.shape(),
        support,
    )?;
    match (inferred.shape.is_scalar(), declaration.initial()) {
        (true, Some(_)) | (false, None) => {}
        (true, None) => {
            return Err(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                file,
                declaration.range(),
                "scalar Field requires one scalar initial value",
            ));
        }
        (false, Some(_)) => {
            return Err(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                file,
                declaration.range(),
                "non-scalar Field cannot receive a scalar initial value",
            ));
        }
    }
    Ok(inferred)
}

/// Construct the one identity-parametric Field value type shared by owned
/// Fields and occurrence-bound Field slots.
pub(in crate::hierarchy) fn field_value_type<I>(
    file: &str,
    range: TextRange,
    dimension: &Expr,
    shape: Option<&ValueShapeSyntax>,
    support: Option<SpatialSupport<I>>,
) -> Result<ExpressionType<I>, Diagnostic> {
    let dimension = lower_dimension(file, dimension)?;
    let (shape, frame) = match shape {
        None | Some(ValueShapeSyntax::Scalar) => (ValueShape::scalar(), ValueFrame::Invariant),
        Some(ValueShapeSyntax::Exact(extents)) => (
            ValueShape::new(extents.iter().copied()).map_err(|error| {
                source_error(codes::LANGUAGE_TYPE_ERROR, file, range, error.to_string())
            })?,
            ValueFrame::Invariant,
        ),
        Some(ValueShapeSyntax::SpatialVector) => {
            let Some(SpatialSupport::Volume { dimensions, .. }) = support.as_ref() else {
                return Err(source_error(
                    codes::LANGUAGE_TYPE_ERROR,
                    file,
                    range,
                    "`spatial_vector` Field shape requires an exact volume support",
                ));
            };
            let extent = u32::try_from(*dimensions).map_err(|_| {
                source_error(
                    codes::LANGUAGE_TYPE_ERROR,
                    file,
                    range,
                    "support ambient dimension exceeds portable u32 shape range",
                )
            })?;
            (
                ValueShape::new([extent]).map_err(|error| {
                    source_error(codes::LANGUAGE_TYPE_ERROR, file, range, error.to_string())
                })?,
                ValueFrame::SpatialCartesian,
            )
        }
        Some(_) => {
            return Err(source_error(
                codes::LANGUAGE_LOWERING_ERROR,
                file,
                range,
                "Field value shape is newer than definition-body validation",
            ));
        }
    };
    Ok(ExpressionType::shaped(dimension, shape, frame, support))
}

pub(super) fn component_port_contract(
    elaborator: &Elaborator<'_>,
    owner: &ComponentDefinition<'_>,
    declaration: &ComponentPortDecl,
) -> Result<PortContract, Vec<Diagnostic>> {
    let file = owner.file;
    match declaration.syntax() {
        PortSyntax::Signal {
            direction,
            dimension,
        } => lower_dimension(file, dimension)
            .map(|dimension| PortContract::Signal {
                direction: *direction,
                dimension,
            })
            .map_err(|error| vec![error]),
        PortSyntax::ScalarPhysicalConnector { connector } => {
            let connector = elaborator
                .resolve_connector(&owner.namespace, connector, file, declaration.range())
                .map_err(|error| vec![error])?;
            let eqiora_lang::ConnectorSyntax::ScalarPhysical {
                across_dimension,
                through_dimension,
            } = connector.declaration.syntax()
            else {
                return Err(vec![source_error(
                    codes::LANGUAGE_LOWERING_ERROR,
                    connector.file,
                    connector.declaration.range(),
                    "Connector syntax is newer than definition-body validation",
                )]);
            };
            let mut diagnostics = Vec::new();
            let across_dimension = lower_dimension(connector.file, across_dimension)
                .map_err(|error| diagnostics.push(error))
                .ok();
            let through_dimension = lower_dimension(connector.file, through_dimension)
                .map_err(|error| diagnostics.push(error))
                .ok();
            match (across_dimension, through_dimension) {
                (Some(across_dimension), Some(through_dimension)) => Ok(PortContract::Physical {
                    nominal: PhysicalNominal::Connector(DefinitionKey {
                        namespace: connector.namespace,
                        name: connector.declaration.name().to_owned(),
                    }),
                    across_dimension,
                    through_dimension,
                }),
                _ => Err(diagnostics),
            }
        }
        PortSyntax::FieldPhysical { connector, support } => {
            let connector = elaborator
                .resolve_connector(&owner.namespace, connector, file, declaration.range())
                .map_err(|error| vec![error])?;
            let ConnectorSyntax::FieldPhysical {
                trace,
                flux,
                shape,
                frame,
                pairing,
            } = connector.declaration.syntax()
            else {
                return Err(vec![source_error(
                    codes::LANGUAGE_TYPE_ERROR,
                    file,
                    declaration.range(),
                    "field-physical Port requires a field-physical Connector",
                )]);
            };
            let interface =
                super::super::supports::component_support_interface(file, owner.declaration)?;
            let support_contract =
                interface.visible_support(support).cloned().ok_or_else(|| {
                    vec![source_error(
                        codes::LANGUAGE_TYPE_ERROR,
                        file,
                        declaration.range(),
                        format!(
                            "field-physical Port support `{support}` is not a public support slot"
                        ),
                    )]
                })?;
            boundary_port_contract(
                connector,
                support_contract,
                file,
                declaration.range(),
                trace,
                flux,
                shape,
                *frame,
                *pairing,
            )
        }
        _ => Err(vec![source_error(
            codes::LANGUAGE_TYPE_ERROR,
            file,
            declaration.range(),
            "component Port must be an explicit signal or nominal Connector interface",
        )]),
    }
}

pub(super) fn component_port_family_contract(
    elaborator: &Elaborator<'_>,
    owner: &ComponentDefinition<'_>,
    declaration: &ComponentPortFamilyDecl,
    support: SpatialSupport<String>,
) -> Result<BoundaryPortFamilyContract, Vec<Diagnostic>> {
    let port = declaration.port();
    let PortSyntax::FieldPhysical {
        connector,
        support: declared_support,
    } = port.syntax()
    else {
        return Err(vec![source_error(
            codes::LANGUAGE_TYPE_ERROR,
            owner.file,
            declaration.range(),
            "only a field-physical Port can declare a boundary family",
        )]);
    };
    if declared_support != declaration.binder().member() {
        return Err(vec![source_error(
            codes::LANGUAGE_TYPE_ERROR,
            owner.file,
            declaration.range(),
            "field-physical Port-family support must name its binder member",
        )]);
    }
    let connector_definition = elaborator
        .resolve_connector(&owner.namespace, connector, owner.file, declaration.range())
        .map_err(|error| vec![error])?;
    let ConnectorSyntax::FieldPhysical {
        trace,
        flux,
        shape,
        frame,
        pairing,
    } = connector_definition.declaration.syntax()
    else {
        return Err(vec![source_error(
            codes::LANGUAGE_TYPE_ERROR,
            owner.file,
            declaration.range(),
            "field-physical Port family requires a field-physical Connector",
        )]);
    };
    let port = boundary_port_contract(
        connector_definition,
        support,
        owner.file,
        declaration.range(),
        trace,
        flux,
        shape,
        *frame,
        *pairing,
    )?;
    Ok(BoundaryPortFamilyContract {
        binder: declaration.binder().clone(),
        port,
    })
}

fn synthetic_component_family_support(
    owner: &ComponentDefinition<'_>,
    binder: &BoundaryFamilyBinderSyntax,
) -> Result<SpatialSupport<String>, Diagnostic> {
    let exterior = owner
        .declaration
        .items()
        .iter()
        .find_map(|item| match item {
            ComponentItem::Support(declaration) if declaration.name() == binder.set() => {
                Some(declaration)
            }
            _ => None,
        })
        .ok_or_else(|| {
            unresolved(
                owner.file,
                binder.range(),
                binder.set(),
                "complete-exterior support set",
            )
        })?;
    let SupportSlotSyntax::CompleteExterior { parent } = exterior.syntax() else {
        return Err(source_error(
            codes::LANGUAGE_TYPE_ERROR,
            owner.file,
            exterior.range(),
            format!("support `{}` is not a complete exterior", binder.set()),
        ));
    };
    let parent_declaration = owner
        .declaration
        .items()
        .iter()
        .find_map(|item| match item {
            ComponentItem::Support(declaration) if declaration.name() == parent => {
                Some(declaration)
            }
            _ => None,
        })
        .ok_or_else(|| {
            unresolved(
                owner.file,
                exterior.range(),
                parent,
                "complete-exterior volume parent",
            )
        })?;
    let SupportSlotSyntax::Volume { ambient_dimension } = parent_declaration.syntax() else {
        return Err(source_error(
            codes::LANGUAGE_TYPE_ERROR,
            owner.file,
            exterior.range(),
            format!(
                "complete-exterior support `{}` requires volume parent `{parent}`",
                binder.set()
            ),
        ));
    };
    Ok(SpatialSupport::Boundary {
        domain: synthetic_boundary_member_identity(binder),
        parent: parent.clone(),
        dimensions: *ambient_dimension,
    })
}

fn synthetic_boundary_member_identity(binder: &BoundaryFamilyBinderSyntax) -> String {
    format!("@complete-exterior/{}/{}", binder.set(), binder.member())
}

pub(super) fn validate_boundary_connection(
    scope: &DefinitionScope<'_, '_>,
    declaration: &BoundaryConnectionDecl,
) -> Result<(), Diagnostic> {
    if declaration.syntax() != ConnectionSyntax::Conserving {
        return Err(source_error(
            codes::LANGUAGE_TYPE_ERROR,
            scope.file,
            declaration.range(),
            "spatial-periodic Connections are not supported inside reusable Components",
        ));
    }
    let Some(binder) = declaration.binder() else {
        return Err(source_error(
            codes::LANGUAGE_TYPE_ERROR,
            scope.file,
            declaration.range(),
            "boundary Connection in a reusable Component requires a family binder",
        ));
    };
    let active = scope.boundary_family_scope(binder)?;
    if declaration.ports().len() < 2 {
        return Err(source_error(
            codes::LANGUAGE_TYPE_ERROR,
            scope.file,
            declaration.range(),
            "Connection requires at least two Ports",
        ));
    }
    let mut exact_references = BTreeSet::new();
    let mut contracts = Vec::with_capacity(declaration.ports().len());
    for reference in declaration.ports() {
        let Some(selector) = reference.selector() else {
            return Err(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                scope.file,
                reference.port().range(),
                "every Port in a boundary-family Connection requires an exact selector",
            ));
        };
        let key = (
            reference
                .port()
                .segments()
                .map(str::to_owned)
                .collect::<Vec<_>>(),
            selector.member().to_owned(),
            selector.target().to_owned(),
        );
        if !exact_references.insert(key) {
            return Err(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                scope.file,
                declaration.range(),
                "Connection repeats the same selected Port",
            ));
        }
        contracts.push(scope.resolve_boundary_port_reference(reference, &active)?);
    }
    let Some(PortContract::BoundaryPhysical { nominal, .. }) = contracts.first() else {
        return Err(source_error(
            codes::LANGUAGE_TYPE_ERROR,
            scope.file,
            declaration.range(),
            "boundary-family Connection requires field-physical Ports",
        ));
    };
    if contracts.iter().skip(1).any(|contract| {
        !matches!(contract, PortContract::BoundaryPhysical { nominal: candidate, .. } if candidate == nominal)
    }) {
        return Err(source_error(
            codes::LANGUAGE_TYPE_ERROR,
            scope.file,
            declaration.range(),
            "boundary-family Connection requires the exact same specialized Connector",
        ));
    }
    Ok(())
}

/// Validate one exact Boundary Connection in a closed Model.
///
/// Family members are specialized to the selected Model Boundary for type
/// checking. Ordinary endpoints are returned separately: their membership is
/// definition-independent, while the equivalence class containing the exact
/// family member is deliberately deferred to occurrence expansion.
pub(super) fn validate_model_boundary_connection(
    scope: &DefinitionScope<'_, '_>,
    declaration: &BoundaryConnectionDecl,
) -> Result<super::PhysicalEndpointSelections, Diagnostic> {
    if declaration.binder().is_some() {
        return Err(source_error(
            codes::LANGUAGE_TYPE_ERROR,
            scope.file,
            declaration.range(),
            "Model boundary Connection cannot declare a family binder",
        ));
    }
    if declaration.syntax() == ConnectionSyntax::SpatialPeriodic && declaration.ports().len() != 2 {
        return Err(source_error(
            codes::LANGUAGE_TYPE_ERROR,
            scope.file,
            declaration.range(),
            "spatial-periodic Connection requires exactly two Ports",
        ));
    }
    if declaration.syntax() != ConnectionSyntax::SpatialPeriodic && declaration.ports().len() < 2 {
        return Err(source_error(
            codes::LANGUAGE_TYPE_ERROR,
            scope.file,
            declaration.range(),
            "Connection requires at least two Ports",
        ));
    }

    let mut exact_references = BTreeSet::new();
    let mut contracts = Vec::with_capacity(declaration.ports().len());
    let mut deferred_memberships = super::PhysicalEndpointSelections::new();
    for reference in declaration.ports() {
        let key = (
            reference
                .port()
                .segments()
                .map(str::to_owned)
                .collect::<Vec<_>>(),
            reference
                .selector()
                .map(|selector| (selector.member().to_owned(), selector.target().to_owned())),
        );
        if !exact_references.insert(key) {
            return Err(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                scope.file,
                declaration.range(),
                "Connection repeats the same selected Port",
            ));
        }

        let contract = if let Some(selector) = reference.selector() {
            let support = scope.spatial_support(selector.target()).ok_or_else(|| {
                unresolved(
                    scope.file,
                    selector.range(),
                    selector.target(),
                    "selected boundary Domain",
                )
            })?;
            if !matches!(support, SpatialSupport::Boundary { .. }) {
                return Err(source_error(
                    codes::LANGUAGE_TYPE_ERROR,
                    scope.file,
                    selector.range(),
                    format!(
                        "boundary selector target `{}` is not a boundary Domain",
                        selector.target()
                    ),
                ));
            }
            scope
                .resolve_boundary_port_family(reference.port())?
                .selected_on_support(scope.file, reference.port(), selector, support)?
        } else {
            let endpoint =
                ResolvedPhysicalEndpoint::from_path(reference.port()).ok_or_else(|| {
                    source_error(
                        codes::LANGUAGE_TYPE_ERROR,
                        scope.file,
                        reference.port().range(),
                        format!(
                            "`{}` is not a local or child Port selection",
                            reference.port()
                        ),
                    )
                })?;
            deferred_memberships.insert(endpoint);
            scope.resolve_port(reference.port())?
        };
        contracts.push(contract);
    }

    let Some(PortContract::BoundaryPhysical { nominal, .. }) = contracts.first() else {
        return Err(source_error(
            codes::LANGUAGE_TYPE_ERROR,
            scope.file,
            declaration.range(),
            "exact boundary Connection requires field-physical Ports",
        ));
    };
    if contracts.iter().skip(1).any(|contract| {
        !matches!(contract, PortContract::BoundaryPhysical { nominal: candidate, .. } if candidate == nominal)
    }) {
        return Err(source_error(
            codes::LANGUAGE_TYPE_ERROR,
            scope.file,
            declaration.range(),
            "exact boundary Connection requires the same specialized Connector",
        ));
    }
    Ok(deferred_memberships)
}

pub(super) fn model_port_contract(
    scope: &DefinitionScope<'_, '_>,
    declaration: &PortDecl,
) -> Result<PortContract, Diagnostic> {
    match declaration.syntax() {
        PortSyntax::Signal {
            direction,
            dimension,
        } => Ok(PortContract::Signal {
            direction: *direction,
            dimension: lower_dimension(scope.file, dimension)?,
        }),
        PortSyntax::ConservingMarker { dimension } => Ok(PortContract::ConservingMarker {
            dimension: lower_dimension(scope.file, dimension)?,
        }),
        PortSyntax::ScalarPhysical { domain } => match scope.symbols.get(domain) {
            Some(SymbolContract::Domain(DomainContract::Physical {
                across_dimension,
                through_dimension,
            })) => Ok(PortContract::Physical {
                nominal: PhysicalNominal::ModelDomain(domain.clone()),
                across_dimension: *across_dimension,
                through_dimension: *through_dimension,
            }),
            Some(_) => Err(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                scope.file,
                declaration.range(),
                format!("physical Port Domain `{domain}` is not scalar physical"),
            )),
            None => Err(unresolved(
                scope.file,
                declaration.range(),
                domain,
                "scalar physical Domain",
            )),
        },
        PortSyntax::ScalarPhysicalConnector { .. } => Err(source_error(
            codes::LANGUAGE_TYPE_ERROR,
            scope.file,
            declaration.range(),
            "model-level Port cannot use a component Connector declaration directly",
        )),
        PortSyntax::FieldPhysical { .. } => Err(source_error(
            codes::LANGUAGE_TYPE_ERROR,
            scope.file,
            declaration.range(),
            "model-level field-physical Port requires hierarchy specialization",
        )),
        _ => Err(source_error(
            codes::LANGUAGE_LOWERING_ERROR,
            scope.file,
            declaration.range(),
            "Port syntax is newer than definition-body validation",
        )),
    }
}

pub(super) fn validate_connection(
    scope: &DefinitionScope<'_, '_>,
    declaration: &ConnectionDecl,
    connected_ports: &mut BTreeSet<Vec<String>>,
    connection_limits: ConnectionSetLimits,
) -> Result<Option<PhysicalConnectionFragment>, Diagnostic> {
    let mut keys = Vec::with_capacity(declaration.port_paths().len());
    let mut contracts = Vec::with_capacity(declaration.port_paths().len());
    for path in declaration.port_paths() {
        keys.push(path.segments().map(str::to_owned).collect::<Vec<_>>());
        contracts.push(scope.resolve_port(path)?);
    }
    if keys.iter().collect::<BTreeSet<_>>().len() != keys.len() {
        return Err(source_error(
            codes::LANGUAGE_TYPE_ERROR,
            scope.file,
            declaration.range(),
            "Connection repeats the same Port",
        ));
    }
    let scalar_physical = matches!(contracts.first(), Some(PortContract::Physical { .. }))
        && contracts
            .iter()
            .all(|contract| matches!(contract, PortContract::Physical { .. }));
    if scalar_physical {
        validate_connection_contract(declaration, &contracts, scope.file)?;
        let endpoints = declaration.port_paths().iter().map(|path| {
            ResolvedPhysicalEndpoint::from_path(path)
                .expect("resolved visible Port paths have one or two segments")
        });
        return ConnectionFragment::try_new(endpoints, connection_limits)
            .map(Some)
            .map_err(|error| connection_fragment_error(scope.file, declaration.range(), error));
    }
    let boundary_physical = matches!(
        contracts.first(),
        Some(PortContract::BoundaryPhysical { .. })
    ) && contracts
        .iter()
        .all(|contract| matches!(contract, PortContract::BoundaryPhysical { .. }));
    if boundary_physical {
        if declaration.syntax() != ConnectionSyntax::Conserving {
            return Err(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                scope.file,
                declaration.range(),
                "field-physical Ports require a conserving Connection",
            ));
        }
        let Some(PortContract::BoundaryPhysical { nominal, .. }) = contracts.first() else {
            unreachable!("boundary-physical family was established");
        };
        if contracts.iter().skip(1).any(|contract| {
            !matches!(contract, PortContract::BoundaryPhysical { nominal: candidate, .. } if candidate == nominal)
        }) {
            return Err(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                scope.file,
                declaration.range(),
                "field-physical Connection requires the exact same specialized Connector",
            ));
        }
        let endpoints = declaration.port_paths().iter().map(|path| {
            ResolvedPhysicalEndpoint::from_path(path)
                .expect("resolved visible Port paths have one or two segments")
        });
        return ConnectionFragment::try_new(endpoints, connection_limits)
            .map(Some)
            .map_err(|error| connection_fragment_error(scope.file, declaration.range(), error));
    }
    if let Some(key) = keys
        .iter()
        .find(|key| connected_ports.contains(key.as_slice()))
    {
        return Err(source_error(
            codes::LANGUAGE_TYPE_ERROR,
            scope.file,
            declaration.range(),
            format!(
                "Port `{}` already belongs to another Connection",
                key.join(".")
            ),
        ));
    }
    validate_connection_contract(declaration, &contracts, scope.file)?;
    connected_ports.extend(keys);
    Ok(None)
}

fn connection_fragment_error(
    file: &str,
    range: TextRange,
    error: ConnectionSetError,
) -> Diagnostic {
    let code = match error {
        ConnectionSetError::TooFewMembers { .. } | ConnectionSetError::DuplicateMember => {
            codes::LANGUAGE_TYPE_ERROR
        }
        ConnectionSetError::LimitExceeded { .. }
        | ConnectionSetError::CountOverflow { .. }
        | ConnectionSetError::Allocation { .. } => codes::LANGUAGE_LOWERING_ERROR,
    };
    source_error(code, file, range, error.to_string())
}

fn validate_connection_contract(
    declaration: &ConnectionDecl,
    contracts: &[PortContract],
    file: &str,
) -> Result<(), Diagnostic> {
    let kind = match declaration.syntax() {
        ConnectionSyntax::Signal => ScalarConnectionKind::Signal,
        ConnectionSyntax::Conserving => ScalarConnectionKind::Conserving,
        ConnectionSyntax::SpatialPeriodic => {
            return Err(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                file,
                declaration.range(),
                "spatial-periodic Connection requires exact boundary Port references",
            ));
        }
    };
    let ports = contracts
        .iter()
        .map(connection_port_contract)
        .collect::<Vec<_>>();
    validate_scalar_connection(kind, &ports).map_err(|violation| {
        source_error(
            codes::LANGUAGE_TYPE_ERROR,
            file,
            declaration.range(),
            connection_violation_message(violation),
        )
    })?;
    if kind == ScalarConnectionKind::Signal
        && !matches!(
            contracts.first(),
            Some(PortContract::Signal {
                direction: SignalDirectionSyntax::Output,
                ..
            })
        )
    {
        return Err(source_error(
            codes::LANGUAGE_TYPE_ERROR,
            file,
            declaration.range(),
            "signal Connection source before `->` must be its output Port",
        ));
    }
    Ok(())
}

fn connection_port_contract(contract: &PortContract) -> ScalarPortContract<&PhysicalNominal> {
    match contract {
        PortContract::Signal {
            direction,
            dimension,
        } => ScalarPortContract::Signal {
            direction: match direction {
                SignalDirectionSyntax::Input => SignalDirection::Input,
                SignalDirectionSyntax::Output => SignalDirection::Output,
            },
            dimension: *dimension,
        },
        PortContract::ConservingMarker { dimension } => ScalarPortContract::ConservingMarker {
            dimension: *dimension,
        },
        PortContract::Physical { nominal, .. } => ScalarPortContract::ScalarPhysical { nominal },
        PortContract::BoundaryPhysical { nominal, .. } => {
            ScalarPortContract::ScalarPhysical { nominal }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn boundary_port_contract(
    connector_definition: super::super::preflight::ConnectorDefinition<'_>,
    support: SpatialSupport<String>,
    file: &str,
    range: TextRange,
    trace: &eqiora_lang::ConnectorQuantitySyntax,
    flux: &eqiora_lang::ConnectorQuantitySyntax,
    shape: &ValueShapeSyntax,
    frame: FrameSyntax,
    pairing: BoundaryPairingSyntax,
) -> Result<PortContract, Vec<Diagnostic>> {
    let SpatialSupport::Boundary { dimensions, .. } = &support else {
        return Err(vec![source_error(
            codes::LANGUAGE_TYPE_ERROR,
            file,
            range,
            "field-physical Port `over` support must be an exact boundary",
        )]);
    };
    let dimensions = *dimensions;
    let shape = resolve_value_shape(file, range, shape, dimensions)?;
    let frame = resolve_frame(file, range, frame)?;
    if frame == ValueFrame::SpatialCartesian
        && (shape.is_scalar()
            || shape
                .extents()
                .iter()
                .any(|extent| usize::try_from(extent.get()).ok() != Some(dimensions)))
    {
        return Err(vec![source_error(
            codes::LANGUAGE_TYPE_ERROR,
            file,
            range,
            "spatial Connector shape must equal the exact support ambient dimension",
        )]);
    }
    let pairing = match pairing {
        BoundaryPairingSyntax::EuclideanBoundaryDuality => {
            BoundaryPairing::EuclideanBoundaryDuality
        }
        _ => {
            return Err(vec![source_error(
                codes::LANGUAGE_LOWERING_ERROR,
                file,
                range,
                "boundary pairing is newer than this compiler",
            )]);
        }
    };
    let connector = BoundaryPhysicalConnector::new(
        lower_dimension(connector_definition.file, trace.dimension()).map_err(|e| vec![e])?,
        lower_dimension(connector_definition.file, flux.dimension()).map_err(|e| vec![e])?,
        shape.clone(),
        frame,
        pairing,
    )
    .map_err(|violation| {
        vec![source_error(
            codes::LANGUAGE_TYPE_ERROR,
            file,
            range,
            format!("invalid field-physical Connector contract: {violation:?}"),
        )]
    })?;
    Ok(PortContract::BoundaryPhysical {
        nominal: PhysicalNominal::BoundaryConnector {
            definition: DefinitionKey {
                namespace: connector_definition.namespace,
                name: connector_definition.declaration.name().to_owned(),
            },
            shape,
        },
        connector,
        support,
    })
}

pub(in crate::hierarchy) fn resolve_value_shape(
    file: &str,
    range: TextRange,
    syntax: &ValueShapeSyntax,
    ambient_dimension: usize,
) -> Result<ValueShape, Vec<Diagnostic>> {
    let extents = match syntax {
        ValueShapeSyntax::Scalar => return Ok(ValueShape::scalar()),
        ValueShapeSyntax::Exact(extents) => extents.clone(),
        ValueShapeSyntax::SpatialVector => {
            vec![u32::try_from(ambient_dimension).map_err(|_| {
                vec![source_error(
                    codes::LANGUAGE_TYPE_ERROR,
                    file,
                    range,
                    "support ambient dimension exceeds portable u32 shape range",
                )]
            })?]
        }
        _ => {
            return Err(vec![source_error(
                codes::LANGUAGE_LOWERING_ERROR,
                file,
                range,
                "value shape is newer than this compiler",
            )]);
        }
    };
    ValueShape::new(extents).map_err(|error| {
        vec![source_error(
            codes::LANGUAGE_TYPE_ERROR,
            file,
            range,
            error.to_string(),
        )]
    })
}

fn resolve_frame(
    file: &str,
    range: TextRange,
    syntax: FrameSyntax,
) -> Result<ValueFrame, Vec<Diagnostic>> {
    match syntax {
        FrameSyntax::Invariant => Ok(ValueFrame::Invariant),
        FrameSyntax::Spatial => Ok(ValueFrame::SpatialCartesian),
        _ => Err(vec![source_error(
            codes::LANGUAGE_LOWERING_ERROR,
            file,
            range,
            "frame syntax is newer than this compiler",
        )]),
    }
}

fn connection_violation_message(violation: ScalarConnectionViolation) -> &'static str {
    match violation {
        ScalarConnectionViolation::TooFewPorts { .. } => {
            "Connection requires at least two visible Ports"
        }
        ScalarConnectionViolation::SignalDirections { .. } => {
            "signal Connection requires exactly one output and one or more inputs"
        }
        ScalarConnectionViolation::SignalDimensionMismatch => {
            "signal Connection requires dimension-matched inputs"
        }
        ScalarConnectionViolation::MixedConservingFamilies => {
            "conserving Connection cannot mix signal, marker, and scalar physical Ports"
        }
        ScalarConnectionViolation::MarkerDimensionMismatch => {
            "conserving Connection marker Ports must have identical physical dimensions"
        }
        ScalarConnectionViolation::PhysicalNominalMismatch => {
            "conserving Connection requires scalar physical Ports on the exact same nominal Connector or Domain"
        }
    }
}

pub(super) fn unresolved(file: &str, range: TextRange, name: &str, expected: &str) -> Diagnostic {
    source_error(
        codes::LANGUAGE_TYPE_ERROR,
        file,
        range,
        format!("unresolved {expected} `{name}`"),
    )
}
