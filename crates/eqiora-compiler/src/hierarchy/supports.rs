//! Occurrence-bound spatial-support interfaces for reusable Components.
//!
//! Support slots are definition-time obligations. They are resolved to exact
//! existing Domain identities before occurrence expansion and never become
//! Kernel entities, values, mesh handles, or inference rules.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use eqiora_core::Diagnostic;
use eqiora_core::diagnostic::codes;
use eqiora_lang::{
    BoundarySetBindingDecl, ComponentDecl, ComponentItem, DomainSyntax, InstanceDecl, Item,
    ModelDecl, SupportSlotSyntax, TextRange, VisibilitySyntax,
};
use eqiora_schema::kernel::{BoundarySide, typing::SpatialSupport};

use crate::lower::source_error;

use super::CompleteExteriorLimits;
use super::complete_exterior::{
    CartesianDomain, CompleteExteriorError, CompleteExteriorWitness,
    prove_complete_cartesian_exterior,
};

#[derive(Clone, Debug)]
pub(super) struct SupportSlotContract {
    visibility: VisibilitySyntax,
    support: SpatialSupport<String>,
}

impl SupportSlotContract {
    pub(super) const fn visibility(&self) -> VisibilitySyntax {
        self.visibility
    }

    pub(super) const fn support(&self) -> &SpatialSupport<String> {
        &self.support
    }
}

/// Definition-time obligation for a complete Cartesian exterior.
///
/// The parent remains a slot name until occurrence binding. No inferred set,
/// geometry handle, or Kernel node is stored in a component interface.
#[derive(Clone, Debug)]
pub(super) struct CompleteExteriorSlotContract {
    visibility: VisibilitySyntax,
    parent_slot: String,
    ambient_dimension: usize,
}

impl CompleteExteriorSlotContract {
    pub(super) const fn visibility(&self) -> VisibilitySyntax {
        self.visibility
    }

    pub(super) fn parent_slot(&self) -> &str {
        &self.parent_slot
    }

    pub(super) const fn ambient_dimension(&self) -> usize {
        self.ambient_dimension
    }
}

#[derive(Clone, Debug, Default)]
pub(super) struct SupportInterface {
    slots: BTreeMap<String, SupportSlotContract>,
    complete_exteriors: BTreeMap<String, CompleteExteriorSlotContract>,
}

impl SupportInterface {
    pub(super) fn get(&self, name: &str) -> Option<&SupportSlotContract> {
        self.slots.get(name)
    }

    pub(super) fn iter(&self) -> impl Iterator<Item = (&str, &SupportSlotContract)> {
        self.slots
            .iter()
            .map(|(name, contract)| (name.as_str(), contract))
    }

    pub(super) fn complete_exterior(&self, name: &str) -> Option<&CompleteExteriorSlotContract> {
        self.complete_exteriors.get(name)
    }

    pub(super) fn complete_exteriors(
        &self,
    ) -> impl Iterator<Item = (&str, &CompleteExteriorSlotContract)> {
        self.complete_exteriors
            .iter()
            .map(|(name, contract)| (name.as_str(), contract))
    }

    pub(super) fn visible_support(&self, name: &str) -> Option<&SpatialSupport<String>> {
        self.slots.get(name).and_then(|contract| {
            (contract.visibility == VisibilitySyntax::Public).then_some(&contract.support)
        })
    }
}

pub(super) fn component_support_interface(
    file: &str,
    component: &ComponentDecl,
) -> Result<SupportInterface, Vec<Diagnostic>> {
    let declarations = component
        .items()
        .iter()
        .filter_map(|item| match item {
            ComponentItem::Support(declaration) => {
                Some((declaration.name().to_owned(), declaration))
            }
            _ => None,
        })
        .collect::<BTreeMap<_, _>>();
    let mut slots = BTreeMap::new();
    let mut complete_exteriors = BTreeMap::new();
    let mut diagnostics = Vec::new();

    for (name, declaration) in &declarations {
        if declaration.visibility() != VisibilitySyntax::Public {
            diagnostics.push(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                file,
                declaration.range(),
                format!(
                    "support slot `{name}` must be public; private occurrence support is not representable in v1"
                ),
            ));
            continue;
        }
        let SupportSlotSyntax::Volume { ambient_dimension } = declaration.syntax() else {
            continue;
        };
        if *ambient_dimension == 0 {
            diagnostics.push(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                file,
                declaration.range(),
                format!("volume support slot `{name}` requires a positive ambient dimension"),
            ));
            continue;
        }
        slots.insert(
            name.clone(),
            SupportSlotContract {
                visibility: declaration.visibility(),
                support: SpatialSupport::Volume {
                    domain: name.clone(),
                    dimensions: *ambient_dimension,
                },
            },
        );
    }

    for (name, declaration) in &declarations {
        if declaration.visibility() != VisibilitySyntax::Public {
            continue;
        }
        let SupportSlotSyntax::Boundary { parent } = declaration.syntax() else {
            continue;
        };
        let Some(parent_contract) = slots.get(parent) else {
            let message = match declarations.get(parent) {
                Some(_) => {
                    format!("boundary support slot `{name}` requires volume parent slot `{parent}`")
                }
                None => format!(
                    "boundary support slot `{name}` refers to unknown parent slot `{parent}`"
                ),
            };
            diagnostics.push(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                file,
                declaration.range(),
                message,
            ));
            continue;
        };
        let SpatialSupport::Volume { dimensions, .. } = parent_contract.support() else {
            diagnostics.push(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                file,
                declaration.range(),
                format!("boundary support slot `{name}` requires volume parent slot `{parent}`"),
            ));
            continue;
        };
        if declaration.visibility() == VisibilitySyntax::Public
            && parent_contract.visibility() != VisibilitySyntax::Public
        {
            diagnostics.push(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                file,
                declaration.range(),
                format!(
                    "public boundary support slot `{name}` requires public parent slot `{parent}`"
                ),
            ));
            continue;
        }
        slots.insert(
            name.clone(),
            SupportSlotContract {
                visibility: declaration.visibility(),
                support: SpatialSupport::Boundary {
                    domain: name.clone(),
                    parent: parent.clone(),
                    dimensions: *dimensions,
                },
            },
        );
    }

    for (name, declaration) in &declarations {
        if declaration.visibility() != VisibilitySyntax::Public {
            continue;
        }
        let SupportSlotSyntax::CompleteExterior { parent } = declaration.syntax() else {
            continue;
        };
        let Some(parent_contract) = slots.get(parent) else {
            let message = match declarations.get(parent) {
                Some(_) => format!(
                    "complete-exterior support slot `{name}` requires volume parent slot `{parent}`"
                ),
                None => format!(
                    "complete-exterior support slot `{name}` refers to unknown parent slot `{parent}`"
                ),
            };
            diagnostics.push(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                file,
                declaration.range(),
                message,
            ));
            continue;
        };
        let SpatialSupport::Volume { dimensions, .. } = parent_contract.support() else {
            diagnostics.push(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                file,
                declaration.range(),
                format!(
                    "complete-exterior support slot `{name}` requires volume parent slot `{parent}`"
                ),
            ));
            continue;
        };
        complete_exteriors.insert(
            name.clone(),
            CompleteExteriorSlotContract {
                visibility: declaration.visibility(),
                parent_slot: parent.clone(),
                ambient_dimension: *dimensions,
            },
        );
    }

    if diagnostics.is_empty() {
        Ok(SupportInterface {
            slots,
            complete_exteriors,
        })
    } else {
        Err(diagnostics)
    }
}

pub(super) fn model_spatial_supports(
    file: &str,
    model: &ModelDecl,
) -> Result<BTreeMap<String, SpatialSupport<String>>, Vec<Diagnostic>> {
    let mut supports = BTreeMap::new();
    let mut boundaries = Vec::new();
    for item in model.items() {
        let Item::Domain(declaration) = item else {
            continue;
        };
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

/// Per-elaboration accounting for explicit complete-exterior memberships.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct CompleteExteriorMembershipBudget {
    limits: CompleteExteriorLimits,
    total_memberships: usize,
}

impl CompleteExteriorMembershipBudget {
    pub(super) const fn new(limits: CompleteExteriorLimits) -> Self {
        Self {
            limits,
            total_memberships: 0,
        }
    }

    #[cfg(test)]
    pub(super) const fn total_memberships(&self) -> usize {
        self.total_memberships
    }

    fn charge(&mut self, count: usize) -> Result<(), CompleteExteriorBudgetError> {
        if count > self.limits.max_members_per_set {
            return Err(CompleteExteriorBudgetError::SetTooLarge {
                count,
                limit: self.limits.max_members_per_set,
            });
        }
        let total = self
            .total_memberships
            .checked_add(count)
            .ok_or(CompleteExteriorBudgetError::TotalOverflow)?;
        if total > self.limits.max_total_memberships {
            return Err(CompleteExteriorBudgetError::TotalExceeded {
                total,
                limit: self.limits.max_total_memberships,
            });
        }
        self.total_memberships = total;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CompleteExteriorBudgetError {
    SetTooLarge { count: usize, limit: usize },
    TotalOverflow,
    TotalExceeded { total: usize, limit: usize },
}

/// One enclosing-scope boundary locator resolved to an exact identity.
///
/// Flattening supplies a stable internal symbol locator when one is available;
/// symbolic definition checking may use the enclosing lexical name.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ResolvedBoundaryTarget<I> {
    target: String,
    exact_identity: I,
}

impl<I> ResolvedBoundaryTarget<I> {
    pub(super) fn new(target: String, exact_identity: I) -> Self {
        Self {
            target,
            exact_identity,
        }
    }
}

/// One explicit source member resolved to its stable locator and exact
/// elaboration identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ResolvedBoundaryMember<I> {
    target: String,
    exact_identity: I,
    source_range: TextRange,
}

impl<I> ResolvedBoundaryMember<I> {
    pub(super) fn target(&self) -> &str {
        &self.target
    }

    pub(super) const fn exact_identity(&self) -> &I {
        &self.exact_identity
    }

    pub(super) const fn source_range(&self) -> TextRange {
        self.source_range
    }
}

/// Shared, immutable result of one complete-exterior proof.
///
/// Forwarding shares this exact catalog rather than copying or re-proving it.
/// Members are sorted by exact identity; position has no semantic meaning.
#[derive(Clone, Debug, Eq, PartialEq)]
struct ProvedBoundarySet<I> {
    members: Box<[ResolvedBoundaryMember<I>]>,
    witness: CompleteExteriorWitness<I>,
}

impl<I: Ord> ProvedBoundarySet<I> {
    fn member(&self, exact_identity: &I) -> Option<&ResolvedBoundaryMember<I>> {
        self.members
            .binary_search_by(|member| member.exact_identity().cmp(exact_identity))
            .ok()
            .map(|index| &self.members[index])
    }
}

/// A source `boundaries(...)` binding and its geometric proof.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ExplicitBoundarySetBinding<I> {
    proved: Arc<ProvedBoundarySet<I>>,
    source_range: TextRange,
}

impl<I: Ord> ExplicitBoundarySetBinding<I> {
    /// Members in exact-identity order. Position is not semantic.
    #[cfg(test)]
    pub(super) fn members(&self) -> &[ResolvedBoundaryMember<I>] {
        &self.proved.members
    }

    pub(super) fn member(&self, exact_identity: &I) -> Option<&ResolvedBoundaryMember<I>> {
        self.proved.member(exact_identity)
    }

    /// Canonical axis-major side bijection consumed by flattening.
    pub(super) fn witness(&self) -> &CompleteExteriorWitness<I> {
        &self.proved.witness
    }
}

/// A binding that forwards an already proved enclosing complete exterior.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ForwardedBoundarySetBinding<I> {
    target: String,
    proved: Arc<ProvedBoundarySet<I>>,
    source_range: TextRange,
}

impl<I: Ord> ForwardedBoundarySetBinding<I> {
    #[cfg(test)]
    pub(super) fn target(&self) -> &str {
        &self.target
    }

    pub(super) fn witness(&self) -> &CompleteExteriorWitness<I> {
        &self.proved.witness
    }

    #[cfg(test)]
    pub(super) fn members(&self) -> &[ResolvedBoundaryMember<I>] {
        &self.proved.members
    }

    pub(super) fn member(&self, exact_identity: &I) -> Option<&ResolvedBoundaryMember<I>> {
        self.proved.member(exact_identity)
    }
}

/// The two intentionally distinct ways an occurrence can satisfy a set slot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum ResolvedBoundarySet<I> {
    Explicit(ExplicitBoundarySetBinding<I>),
    Forwarded(ForwardedBoundarySetBinding<I>),
}

impl<I: Ord> ResolvedBoundarySet<I> {
    pub(super) fn witness(&self) -> &CompleteExteriorWitness<I> {
        match self {
            Self::Explicit(binding) => binding.witness(),
            Self::Forwarded(binding) => binding.witness(),
        }
    }

    /// Exact-identity keyed locator records; slice order is non-semantic.
    #[cfg(test)]
    pub(super) fn members(&self) -> &[ResolvedBoundaryMember<I>] {
        match self {
            Self::Explicit(binding) => binding.members(),
            Self::Forwarded(binding) => binding.members(),
        }
    }

    pub(super) fn member(&self, exact_identity: &I) -> Option<&ResolvedBoundaryMember<I>> {
        match self {
            Self::Explicit(binding) => binding.member(exact_identity),
            Self::Forwarded(binding) => binding.member(exact_identity),
        }
    }

    /// Source range of the explicit or forwarded binding that established
    /// this occurrence's complete-exterior proof.
    pub(super) const fn source_range(&self) -> TextRange {
        match self {
            Self::Explicit(binding) => binding.source_range,
            Self::Forwarded(binding) => binding.source_range,
        }
    }

    fn proved(&self) -> &Arc<ProvedBoundarySet<I>> {
        match self {
            Self::Explicit(binding) => &binding.proved,
            Self::Forwarded(binding) => &binding.proved,
        }
    }
}

/// Fully checked occurrence support bindings.
///
/// Lexical singular targets are retained for the existing symbol-forwarding
/// path. Exact singular supports and proved sets are the typed inputs for new
/// boundary-family flattening.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ResolvedSupportBindings<I> {
    singular_targets: BTreeMap<String, String>,
    singular_supports: BTreeMap<String, SpatialSupport<I>>,
    boundary_sets: BTreeMap<String, ResolvedBoundarySet<I>>,
}

impl<I: Ord> ResolvedSupportBindings<I> {
    pub(super) const fn singular_targets(&self) -> &BTreeMap<String, String> {
        &self.singular_targets
    }

    pub(super) const fn singular_supports(&self) -> &BTreeMap<String, SpatialSupport<I>> {
        &self.singular_supports
    }

    pub(super) fn boundary_set(&self, slot: &str) -> Option<&ResolvedBoundarySet<I>> {
        self.boundary_sets.get(slot)
    }

    pub(super) fn boundary_sets(&self) -> impl Iterator<Item = (&str, &ResolvedBoundarySet<I>)> {
        self.boundary_sets
            .iter()
            .map(|(slot, set)| (slot.as_str(), set))
    }

    #[cfg(test)]
    fn into_singular_targets(self) -> BTreeMap<String, String> {
        self.singular_targets
    }
}

/// Build a proved stand-in for an enclosing complete-exterior obligation
/// during occurrence-free reusable-definition checking.
///
/// The identities are compiler-private and deterministic. They are never
/// staged, projected, serialized, or consumed by flattening. Importantly, the
/// stand-in still passes through the same pure Cartesian proof as a concrete
/// occurrence, so symbolic checking cannot accept a weaker set shape.
pub(super) fn symbolic_complete_exterior_set(
    file: &str,
    slot_name: &str,
    contract: &CompleteExteriorSlotContract,
    source_range: TextRange,
) -> Result<ResolvedBoundarySet<String>, Diagnostic> {
    let side_count = contract.ambient_dimension().checked_mul(2).ok_or_else(|| {
        source_error(
            codes::LANGUAGE_LOWERING_ERROR,
            file,
            source_range,
            format!("symbolic complete-exterior side count overflows usize for slot `{slot_name}`"),
        )
    })?;
    let mut exact_members = Vec::new();
    let mut resolved_members = Vec::new();
    let mut metadata = Vec::new();
    if exact_members.try_reserve_exact(side_count).is_err()
        || resolved_members.try_reserve_exact(side_count).is_err()
        || metadata.try_reserve_exact(side_count).is_err()
    {
        return Err(source_error(
            codes::LANGUAGE_LOWERING_ERROR,
            file,
            source_range,
            format!(
                "cannot reserve {side_count} symbolic complete-exterior members for slot `{slot_name}`"
            ),
        ));
    }
    for axis in 0..contract.ambient_dimension() {
        for side in [BoundarySide::Lower, BoundarySide::Upper] {
            let side_name = boundary_side_name(side);
            let identity = format!("@symbolic/{slot_name}/axis/{axis}/{side_name}");
            exact_members.push(identity.clone());
            resolved_members.push(ResolvedBoundaryMember {
                target: identity.clone(),
                exact_identity: identity.clone(),
                source_range,
            });
            metadata.push((
                identity,
                CartesianDomain::Boundary {
                    exact_parent: contract.parent_slot().to_owned(),
                    ambient_dimension: contract.ambient_dimension(),
                    axis,
                    side,
                },
            ));
        }
    }
    metadata.sort_unstable_by(|left, right| left.0.cmp(&right.0));
    let exact_parent = contract.parent_slot().to_owned();
    let witness =
        prove_complete_cartesian_exterior(exact_parent.clone(), exact_members, |identity| {
            if identity == &exact_parent {
                Some(CartesianDomain::Volume {
                    ambient_dimension: contract.ambient_dimension(),
                })
            } else {
                metadata
                    .binary_search_by(|candidate| candidate.0.cmp(identity))
                    .ok()
                    .map(|index| metadata[index].1.clone())
            }
        })
        .map_err(|error| {
            complete_exterior_proof_diagnostic(
                file,
                slot_name,
                source_range,
                &resolved_members,
                error,
            )
        })?;
    resolved_members
        .sort_unstable_by(|left, right| left.exact_identity().cmp(right.exact_identity()));
    Ok(ResolvedBoundarySet::Forwarded(
        ForwardedBoundarySetBinding {
            target: slot_name.to_owned(),
            proved: Arc::new(ProvedBoundarySet {
                members: resolved_members.into_boxed_slice(),
                witness,
            }),
            source_range,
        },
    ))
}

/// Resolve singular supports and complete-exterior occurrence bindings.
///
/// `resolve_boundary_name` maps one lexical member to a stable enclosing-scope
/// locator and exact elaboration identity. `resolve_cartesian_domain` supplies
/// authoritative metadata by exact identity. `resolve_forwarded_set` only
/// accepts an already proved enclosing set; forwarding therefore cannot weaken
/// the proof obligation.
#[allow(clippy::too_many_arguments)]
pub(super) fn resolve_instance_support_bindings<I: Clone + Ord>(
    binding_file: &str,
    component: &ComponentDecl,
    interface: &SupportInterface,
    instance: &InstanceDecl,
    mut resolve_singular: impl FnMut(&str) -> Option<SpatialSupport<I>>,
    mut resolve_boundary_name: impl FnMut(&str) -> Option<ResolvedBoundaryTarget<I>>,
    mut resolve_cartesian_domain: impl FnMut(&I) -> Option<CartesianDomain<I>>,
    mut resolve_forwarded_set: impl FnMut(&str) -> Option<ResolvedBoundarySet<I>>,
    membership_budget: &mut CompleteExteriorMembershipBudget,
) -> Result<ResolvedSupportBindings<I>, Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    let mut targets = BTreeMap::new();
    let mut actual = BTreeMap::new();
    let mut boundary_sets = BTreeMap::new();
    let mut forwarded = Vec::new();
    let mut explicit = Vec::new();
    let mut seen = BTreeSet::new();

    for binding in instance.support_bindings() {
        if !seen.insert(binding.slot()) {
            diagnostics.push(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                binding_file,
                binding.range(),
                format!(
                    "duplicate binding for support slot `{}` in instance `{}`",
                    binding.slot(),
                    instance.name()
                ),
            ));
            continue;
        }
        if let Some(contract) = interface.complete_exterior(binding.slot()) {
            if contract.visibility() != VisibilitySyntax::Public {
                diagnostics.push(source_error(
                    codes::LANGUAGE_TYPE_ERROR,
                    binding_file,
                    binding.range(),
                    format!(
                        "private complete-exterior support slot `{}` cannot be bound on instance `{}`",
                        binding.slot(),
                        instance.name()
                    ),
                ));
                continue;
            }
            let Some(source_set) = resolve_forwarded_set(binding.target()) else {
                diagnostics.push(source_error(
                    codes::LANGUAGE_TYPE_ERROR,
                    binding_file,
                    binding.range(),
                    format!(
                        "complete-exterior binding target `{}` is not a proved enclosing BoundarySet",
                        binding.target()
                    ),
                ));
                continue;
            };
            forwarded.push((binding, contract, source_set));
            continue;
        }
        let Some(slot) = interface.get(binding.slot()) else {
            diagnostics.push(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                binding_file,
                binding.range(),
                format!(
                    "unknown support slot `{}` on component `{}`",
                    binding.slot(),
                    component.name()
                ),
            ));
            continue;
        };
        if slot.visibility() != VisibilitySyntax::Public {
            diagnostics.push(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                binding_file,
                binding.range(),
                format!(
                    "private support slot `{}` cannot be bound on instance `{}`",
                    binding.slot(),
                    instance.name()
                ),
            ));
            continue;
        }
        let Some(support) = resolve_singular(binding.target()) else {
            diagnostics.push(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                binding_file,
                binding.range(),
                format!(
                    "support binding target `{}` is not an enclosing spatial support",
                    binding.target()
                ),
            ));
            continue;
        };
        targets.insert(binding.slot().to_owned(), binding.target().to_owned());
        actual.insert(binding.slot().to_owned(), (support, binding.range()));
    }

    for binding in instance.boundary_set_bindings() {
        if !seen.insert(binding.slot()) {
            diagnostics.push(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                binding_file,
                binding.range(),
                format!(
                    "duplicate binding for support slot `{}` in instance `{}`",
                    binding.slot(),
                    instance.name()
                ),
            ));
            continue;
        }
        let Some(contract) = interface.complete_exterior(binding.slot()) else {
            let message = if interface.get(binding.slot()).is_some() {
                format!(
                    "singular support slot `{}` cannot be bound with `boundaries(...)`",
                    binding.slot()
                )
            } else {
                format!(
                    "unknown support slot `{}` on component `{}`",
                    binding.slot(),
                    component.name()
                )
            };
            diagnostics.push(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                binding_file,
                binding.range(),
                message,
            ));
            continue;
        };
        explicit.push((binding, contract));
    }

    validate_singular_support_shapes(binding_file, interface, &actual, &mut diagnostics);

    for (binding, contract, source_set) in forwarded {
        let Some((SpatialSupport::Volume { domain, dimensions }, _)) =
            actual.get(contract.parent_slot())
        else {
            continue;
        };
        if dimensions != &contract.ambient_dimension() {
            diagnostics.push(source_error(
                codes::LANGUAGE_LOWERING_ERROR,
                binding_file,
                binding.range(),
                format!(
                    "complete-exterior slot `{}` has inconsistent definition-time dimension",
                    binding.slot()
                ),
            ));
            continue;
        }
        if source_set.witness().exact_parent() != domain {
            diagnostics.push(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                binding_file,
                binding.range(),
                format!(
                    "forwarded BoundarySet for slot `{}` does not have the exact bound parent slot `{}`",
                    binding.slot(),
                    contract.parent_slot()
                ),
            ));
            continue;
        }
        if source_set.witness().ambient_dimension() != *dimensions {
            diagnostics.push(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                binding_file,
                binding.range(),
                format!(
                    "forwarded BoundarySet for slot `{}` requires ambient dimension {}",
                    binding.slot(),
                    dimensions
                ),
            ));
            continue;
        }
        boundary_sets.insert(
            binding.slot().to_owned(),
            ResolvedBoundarySet::Forwarded(ForwardedBoundarySetBinding {
                target: binding.target().to_owned(),
                proved: Arc::clone(source_set.proved()),
                source_range: binding.range(),
            }),
        );
    }

    for (binding, contract) in explicit {
        if let Err(error) = membership_budget.charge(binding.members().len()) {
            diagnostics.push(complete_exterior_budget_diagnostic(
                binding_file,
                binding,
                error,
            ));
            continue;
        }
        let Some((SpatialSupport::Volume { domain, dimensions }, _)) =
            actual.get(contract.parent_slot())
        else {
            continue;
        };
        if dimensions != &contract.ambient_dimension() {
            diagnostics.push(source_error(
                codes::LANGUAGE_LOWERING_ERROR,
                binding_file,
                binding.range(),
                format!(
                    "complete-exterior slot `{}` has inconsistent definition-time dimension",
                    binding.slot()
                ),
            ));
            continue;
        }

        let member_count = binding.members().len();
        let mut exact_members = Vec::new();
        let mut resolved_members = Vec::new();
        if exact_members.try_reserve_exact(member_count).is_err()
            || resolved_members.try_reserve_exact(member_count).is_err()
        {
            diagnostics.push(source_error(
                codes::LANGUAGE_LOWERING_ERROR,
                binding_file,
                binding.range(),
                format!(
                    "cannot reserve {member_count} exact members for complete-exterior slot `{}`",
                    binding.slot()
                ),
            ));
            continue;
        }
        for member in binding.members() {
            let Some(target) = resolve_boundary_name(member.target()) else {
                diagnostics.push(source_error(
                    codes::LANGUAGE_TYPE_ERROR,
                    binding_file,
                    member.range(),
                    format!(
                        "BoundarySet member `{}` is not an enclosing Cartesian Domain",
                        member.target()
                    ),
                ));
                continue;
            };
            exact_members.push(target.exact_identity.clone());
            resolved_members.push(ResolvedBoundaryMember {
                target: target.target,
                exact_identity: target.exact_identity,
                source_range: member.range(),
            });
        }
        if exact_members.len() != member_count {
            continue;
        }

        let exact_parent = domain.clone();
        let result =
            prove_complete_cartesian_exterior(exact_parent.clone(), exact_members, |identity| {
                if identity == &exact_parent {
                    Some(CartesianDomain::Volume {
                        ambient_dimension: *dimensions,
                    })
                } else {
                    resolve_cartesian_domain(identity)
                }
            });
        match result {
            Ok(witness) => {
                resolved_members.sort_unstable_by(|left, right| {
                    left.exact_identity().cmp(right.exact_identity())
                });
                boundary_sets.insert(
                    binding.slot().to_owned(),
                    ResolvedBoundarySet::Explicit(ExplicitBoundarySetBinding {
                        proved: Arc::new(ProvedBoundarySet {
                            members: resolved_members.into_boxed_slice(),
                            witness,
                        }),
                        source_range: binding.range(),
                    }),
                );
            }
            Err(error) => diagnostics.push(complete_exterior_proof_diagnostic(
                binding_file,
                binding.slot(),
                binding.range(),
                &resolved_members,
                error,
            )),
        }
    }

    for (name, slot) in interface.iter() {
        if slot.visibility() == VisibilitySyntax::Public && !actual.contains_key(name) {
            diagnostics.push(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                binding_file,
                instance.range(),
                format!(
                    "instance `{}` has no binding for required support slot `{name}`",
                    instance.name()
                ),
            ));
        }
    }
    for (name, contract) in interface.complete_exteriors() {
        if contract.visibility() == VisibilitySyntax::Public && !boundary_sets.contains_key(name) {
            diagnostics.push(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                binding_file,
                instance.range(),
                format!(
                    "instance `{}` has no binding for required complete-exterior support slot `{name}`",
                    instance.name()
                ),
            ));
        }
    }

    if diagnostics.is_empty() {
        Ok(ResolvedSupportBindings {
            singular_targets: targets,
            singular_supports: actual
                .into_iter()
                .map(|(slot, (support, _))| (slot, support))
                .collect(),
            boundary_sets,
        })
    } else {
        Err(diagnostics)
    }
}

/// Compatibility surface for singular occurrence supports.
///
/// The richer resolver is the sole implementation. Existing callers retain
/// their lexical target map until boundary-family flattening consumes the
/// typed result directly.
#[cfg(test)]
pub(super) fn resolve_instance_supports<I: Clone + Ord>(
    binding_file: &str,
    component: &ComponentDecl,
    interface: &SupportInterface,
    instance: &InstanceDecl,
    resolve_singular: impl FnMut(&str) -> Option<SpatialSupport<I>>,
) -> Result<BTreeMap<String, String>, Vec<Diagnostic>> {
    let mut membership_budget =
        CompleteExteriorMembershipBudget::new(CompleteExteriorLimits::default());
    resolve_instance_support_bindings(
        binding_file,
        component,
        interface,
        instance,
        resolve_singular,
        |_| None,
        |_| None,
        |_| None,
        &mut membership_budget,
    )
    .map(ResolvedSupportBindings::into_singular_targets)
}

fn validate_singular_support_shapes<I: Eq>(
    binding_file: &str,
    interface: &SupportInterface,
    actual: &BTreeMap<String, (SpatialSupport<I>, TextRange)>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (name, slot) in interface.iter() {
        let Some((bound, range)) = actual.get(name) else {
            continue;
        };
        match (slot.support(), bound) {
            (
                SpatialSupport::Volume {
                    dimensions: expected,
                    ..
                },
                SpatialSupport::Volume {
                    dimensions: actual, ..
                },
            ) if expected == actual => {}
            (SpatialSupport::Volume { dimensions, .. }, SpatialSupport::Volume { .. }) => {
                diagnostics.push(source_error(
                    codes::LANGUAGE_TYPE_ERROR,
                    binding_file,
                    *range,
                    format!("volume support slot `{name}` requires ambient dimension {dimensions}"),
                ));
            }
            (SpatialSupport::Volume { .. }, SpatialSupport::Boundary { .. }) => {
                diagnostics.push(kind_mismatch(
                    binding_file,
                    *range,
                    name,
                    "volume",
                    "boundary",
                ));
            }
            (
                SpatialSupport::Boundary {
                    parent: parent_slot,
                    dimensions: expected,
                    ..
                },
                SpatialSupport::Boundary {
                    parent: actual_parent,
                    dimensions: actual_dimensions,
                    ..
                },
            ) => {
                let expected_parent =
                    actual
                        .get(parent_slot)
                        .and_then(|(support, _)| match support {
                            SpatialSupport::Volume { domain, .. } => Some(domain),
                            SpatialSupport::Boundary { .. } | SpatialSupport::Interface { .. } => {
                                None
                            }
                        });
                if expected != actual_dimensions {
                    diagnostics.push(source_error(
                        codes::LANGUAGE_TYPE_ERROR,
                        binding_file,
                        *range,
                        format!(
                            "boundary support slot `{name}` requires ambient dimension {expected}"
                        ),
                    ));
                } else if expected_parent != Some(actual_parent) {
                    diagnostics.push(source_error(
                        codes::LANGUAGE_TYPE_ERROR,
                        binding_file,
                        *range,
                        format!(
                            "boundary support slot `{name}` is not BoundaryOf its exact bound parent slot `{parent_slot}`"
                        ),
                    ));
                }
            }
            (SpatialSupport::Boundary { .. }, SpatialSupport::Volume { .. }) => {
                diagnostics.push(kind_mismatch(
                    binding_file,
                    *range,
                    name,
                    "boundary",
                    "volume",
                ));
            }
            (SpatialSupport::Interface { .. }, _) | (_, SpatialSupport::Interface { .. }) => {
                diagnostics.push(source_error(
                    codes::LANGUAGE_LOWERING_ERROR,
                    binding_file,
                    *range,
                    "derived interface support cannot be bound as a source support slot",
                ));
            }
        }
    }
}

fn complete_exterior_budget_diagnostic(
    file: &str,
    binding: &BoundarySetBindingDecl,
    error: CompleteExteriorBudgetError,
) -> Diagnostic {
    let message = match error {
        CompleteExteriorBudgetError::SetTooLarge { count, limit } => format!(
            "complete-exterior binding for slot `{}` has {count} members, exceeding the {limit} member limit",
            binding.slot()
        ),
        CompleteExteriorBudgetError::TotalOverflow => {
            "complete-exterior membership count overflows usize".to_owned()
        }
        CompleteExteriorBudgetError::TotalExceeded { total, limit } => format!(
            "complete-exterior bindings require {total} total memberships, exceeding the {limit} membership limit"
        ),
    };
    source_error(
        codes::LANGUAGE_LOWERING_ERROR,
        file,
        binding.range(),
        message,
    )
}

fn complete_exterior_proof_diagnostic<I: Eq>(
    file: &str,
    slot: &str,
    binding_range: TextRange,
    members: &[ResolvedBoundaryMember<I>],
    error: CompleteExteriorError<I>,
) -> Diagnostic {
    let range = complete_exterior_error_range(&error, members).unwrap_or(binding_range);
    let (code, message) = match &error {
        CompleteExteriorError::Empty => (
            codes::LANGUAGE_TYPE_ERROR,
            format!("complete-exterior slot `{slot}` requires a nonempty BoundarySet"),
        ),
        CompleteExteriorError::DuplicateExactMember { .. } => (
            codes::LANGUAGE_TYPE_ERROR,
            format!("complete-exterior slot `{slot}` contains one exact Boundary more than once"),
        ),
        CompleteExteriorError::UnknownDomain { .. } => (
            codes::LANGUAGE_TYPE_ERROR,
            format!(
                "complete-exterior slot `{slot}` contains unresolved Cartesian Domain metadata"
            ),
        ),
        CompleteExteriorError::ExactParentIsNotVolume { .. } => (
            codes::LANGUAGE_TYPE_ERROR,
            format!("complete-exterior slot `{slot}` does not have an exact volume parent"),
        ),
        CompleteExteriorError::ExactParentHasZeroDimension { .. } => (
            codes::LANGUAGE_TYPE_ERROR,
            format!("complete-exterior slot `{slot}` has a zero-dimensional parent"),
        ),
        CompleteExteriorError::MemberIsVolume { .. } => (
            codes::LANGUAGE_TYPE_ERROR,
            format!("complete-exterior slot `{slot}` contains a volume instead of a Boundary"),
        ),
        CompleteExteriorError::BoundaryOfBoundary { .. } => (
            codes::LANGUAGE_TYPE_ERROR,
            format!("complete-exterior slot `{slot}` contains a boundary-of-boundary"),
        ),
        CompleteExteriorError::WrongParent { .. } => (
            codes::LANGUAGE_TYPE_ERROR,
            format!(
                "complete-exterior slot `{slot}` contains a Boundary of a different exact parent"
            ),
        ),
        CompleteExteriorError::WrongDimension {
            expected, actual, ..
        } => (
            codes::LANGUAGE_TYPE_ERROR,
            format!(
                "complete-exterior slot `{slot}` requires ambient dimension {expected}, found {actual}"
            ),
        ),
        CompleteExteriorError::AxisOutsideParentDimension {
            axis,
            ambient_dimension,
            ..
        } => (
            codes::LANGUAGE_TYPE_ERROR,
            format!(
                "complete-exterior slot `{slot}` contains axis {axis}, outside parent dimension {ambient_dimension}"
            ),
        ),
        CompleteExteriorError::DuplicateGeometry { axis, side, .. } => (
            codes::LANGUAGE_TYPE_ERROR,
            format!(
                "complete-exterior slot `{slot}` contains Cartesian side ({axis}, {}) more than once",
                boundary_side_name(*side)
            ),
        ),
        CompleteExteriorError::MissingSide { axis, side } => (
            codes::LANGUAGE_TYPE_ERROR,
            format!(
                "complete-exterior slot `{slot}` is missing Cartesian side ({axis}, {})",
                boundary_side_name(*side)
            ),
        ),
        CompleteExteriorError::SideCountOverflow { ambient_dimension } => (
            codes::LANGUAGE_LOWERING_ERROR,
            format!(
                "complete-exterior side count overflows usize for dimension {ambient_dimension}"
            ),
        ),
        CompleteExteriorError::Allocation { resource, count } => (
            codes::LANGUAGE_LOWERING_ERROR,
            format!("cannot reserve {count} entries for {resource}"),
        ),
    };
    source_error(code, file, range, message)
}

fn complete_exterior_error_range<I: Eq>(
    error: &CompleteExteriorError<I>,
    members: &[ResolvedBoundaryMember<I>],
) -> Option<TextRange> {
    let identity = match error {
        CompleteExteriorError::DuplicateExactMember { member }
        | CompleteExteriorError::MemberIsVolume { member }
        | CompleteExteriorError::BoundaryOfBoundary { member, .. }
        | CompleteExteriorError::WrongParent { member, .. }
        | CompleteExteriorError::WrongDimension { member, .. }
        | CompleteExteriorError::AxisOutsideParentDimension { member, .. } => Some(member),
        CompleteExteriorError::DuplicateGeometry { duplicate, .. } => Some(duplicate),
        CompleteExteriorError::UnknownDomain { domain } => Some(domain),
        CompleteExteriorError::Empty
        | CompleteExteriorError::ExactParentIsNotVolume { .. }
        | CompleteExteriorError::ExactParentHasZeroDimension { .. }
        | CompleteExteriorError::MissingSide { .. }
        | CompleteExteriorError::SideCountOverflow { .. }
        | CompleteExteriorError::Allocation { .. } => None,
    };
    identity.and_then(|identity| {
        members
            .iter()
            .rev()
            .find(|member| member.exact_identity() == identity)
            .map(ResolvedBoundaryMember::source_range)
    })
}

const fn boundary_side_name(side: BoundarySide) -> &'static str {
    match side {
        BoundarySide::Lower => "lower",
        BoundarySide::Upper => "upper",
    }
}

fn kind_mismatch(
    file: &str,
    range: TextRange,
    slot: &str,
    expected: &str,
    actual: &str,
) -> Diagnostic {
    source_error(
        codes::LANGUAGE_TYPE_ERROR,
        file,
        range,
        format!("support slot `{slot}` requires {expected} support, found {actual}"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use eqiora_lang::{BoundarySideSyntax, Document};

    fn parse(source: &str) -> Document {
        eqiora_lang::parse("supports.eqi", source)
            .into_document()
            .expect("support-contract fixture parses")
    }

    fn component<'a>(document: &'a Document, name: &str) -> &'a ComponentDecl {
        document
            .components()
            .iter()
            .find(|component| component.name() == name)
            .expect("fixture component exists")
    }

    fn instance<'a>(document: &'a Document, name: &str) -> &'a InstanceDecl {
        document
            .models()
            .iter()
            .flat_map(ModelDecl::items)
            .find_map(|item| match item {
                Item::Instance(instance) if instance.name() == name => Some(instance),
                _ => None,
            })
            .expect("fixture instance exists")
    }

    fn interface(document: &Document, component_name: &str) -> SupportInterface {
        component_support_interface("supports.eqi", component(document, component_name))
            .expect("fixture component has a valid support interface")
    }

    fn spatial_supports(document: &Document) -> BTreeMap<String, SpatialSupport<String>> {
        let model = document.models().first().expect("fixture model exists");
        model_spatial_supports("supports.eqi", model)
            .expect("fixture model has valid spatial supports")
    }

    fn cartesian_domains(document: &Document) -> BTreeMap<String, CartesianDomain<String>> {
        let model = document.models().first().expect("fixture model exists");
        let mut domains = BTreeMap::new();
        for declaration in model.items().iter().filter_map(|item| match item {
            Item::Domain(declaration) => Some(declaration),
            _ => None,
        }) {
            if let DomainSyntax::CartesianBox(bounds) = declaration.syntax() {
                domains.insert(
                    declaration.name().to_owned(),
                    CartesianDomain::Volume {
                        ambient_dimension: bounds.len(),
                    },
                );
            }
        }
        for declaration in model.items().iter().filter_map(|item| match item {
            Item::Domain(declaration) => Some(declaration),
            _ => None,
        }) {
            let DomainSyntax::Boundary { parent, axis, side } = declaration.syntax() else {
                continue;
            };
            let CartesianDomain::Volume { ambient_dimension } =
                domains.get(parent).expect("fixture boundary parent exists")
            else {
                panic!("fixture boundary parent is a volume");
            };
            domains.insert(
                declaration.name().to_owned(),
                CartesianDomain::Boundary {
                    exact_parent: parent.clone(),
                    ambient_dimension: *ambient_dimension,
                    axis: *axis,
                    side: match side {
                        BoundarySideSyntax::Lower => BoundarySide::Lower,
                        BoundarySideSyntax::Upper => BoundarySide::Upper,
                    },
                },
            );
        }
        domains
    }

    fn resolve_complete_exterior(
        document: &Document,
        instance_name: &str,
        budget: &mut CompleteExteriorMembershipBudget,
        resolve_forwarded: impl FnMut(&str) -> Option<ResolvedBoundarySet<String>>,
    ) -> Result<ResolvedSupportBindings<String>, Vec<Diagnostic>> {
        let component = component(document, "BoundaryFamily");
        let interface = interface(document, "BoundaryFamily");
        let supports = spatial_supports(document);
        let domains = cartesian_domains(document);
        resolve_instance_support_bindings(
            "supports.eqi",
            component,
            &interface,
            instance(document, instance_name),
            |target| supports.get(target).cloned(),
            |target| {
                domains.contains_key(target).then(|| {
                    ResolvedBoundaryTarget::new(format!("flat::{target}"), target.to_owned())
                })
            },
            |identity| domains.get(identity).cloned(),
            resolve_forwarded,
            budget,
        )
    }

    fn messages(diagnostics: &[Diagnostic]) -> Vec<&str> {
        diagnostics
            .iter()
            .map(|diagnostic| diagnostic.message())
            .collect()
    }

    const COMPONENT: &str = r#"
component BoundaryState {
  public support body: volume(ambient_dimension = 2);
  public support interface: boundary(parent = body);
}
"#;

    const EXTERIOR_COMPONENT: &str = r#"
component BoundaryFamily {
  public support body: volume(ambient_dimension = 2);
  public support exterior: complete_exterior(parent = body);
}
"#;

    const EXTERIOR_MODEL: &str = r#"
model Use {
  domain body = box(0, 1, 0, 1);
  domain x_lower = boundary(body, axis = 0, side = lower);
  domain x_upper = boundary(body, axis = 0, side = upper);
  domain y_lower = boundary(body, axis = 1, side = lower);
  domain y_upper = boundary(body, axis = 1, side = upper);
  instance explicit: BoundaryFamily(
    support body = body,
    support exterior = boundaries(y_upper, x_lower, y_lower, x_upper)
  );
  instance forwarded: BoundaryFamily(
    support body = body,
    support exterior = enclosing_exterior
  );
}
"#;

    #[test]
    fn exact_volume_and_boundary_bindings_are_accepted_independent_of_order() {
        let source = format!(
            r#"{COMPONENT}
model Use {{
  domain fluid = box(0, 1, 0, 1);
  domain wall = boundary(fluid, axis = 0, side = lower);
  instance forward: BoundaryState(
    support body = fluid,
    support interface = wall
  );
  instance reverse: BoundaryState(
    support interface = wall,
    support body = fluid
  );
}}
"#
        );
        let document = parse(&source);
        let component = component(&document, "BoundaryState");
        let interface = interface(&document, "BoundaryState");
        let supports = spatial_supports(&document);

        let resolve = |instance_name| {
            resolve_instance_supports(
                "supports.eqi",
                component,
                &interface,
                instance(&document, instance_name),
                |name| supports.get(name).cloned(),
            )
            .expect("exact support binding is valid")
        };
        let forward = resolve("forward");
        let reverse = resolve("reverse");

        assert_eq!(forward, reverse);
        assert_eq!(forward.get("body").map(String::as_str), Some("fluid"));
        assert_eq!(forward.get("interface").map(String::as_str), Some("wall"));
    }

    #[test]
    fn complete_exterior_obligation_is_separate_from_singular_supports() {
        let document = parse(&format!("{EXTERIOR_COMPONENT}{EXTERIOR_MODEL}"));
        let interface = interface(&document, "BoundaryFamily");

        assert!(interface.get("exterior").is_none());
        let exterior = interface
            .complete_exterior("exterior")
            .expect("complete exterior is retained as its own obligation");
        assert_eq!(exterior.parent_slot(), "body");
        assert_eq!(exterior.ambient_dimension(), 2);
        assert_eq!(interface.complete_exteriors().count(), 1);

        let symbolic = symbolic_complete_exterior_set(
            "supports.eqi",
            "exterior",
            exterior,
            component(&document, "BoundaryFamily").range(),
        )
        .expect("definition checking receives a proved symbolic exterior");
        assert_eq!(symbolic.witness().exact_parent(), "body");
        assert_eq!(symbolic.witness().sides().len(), 4);
        assert_eq!(symbolic.members().len(), 4);
    }

    #[test]
    fn explicit_complete_exterior_resolves_exact_members_and_canonical_sides() {
        let document = parse(&format!("{EXTERIOR_COMPONENT}{EXTERIOR_MODEL}"));
        let mut budget = CompleteExteriorMembershipBudget::new(CompleteExteriorLimits::default());
        let resolved = resolve_complete_exterior(&document, "explicit", &mut budget, |_| None)
            .expect("unordered exact exterior proves successfully");

        assert_eq!(budget.total_memberships(), 4);
        assert_eq!(
            resolved.singular_targets().get("body").map(String::as_str),
            Some("body")
        );
        assert!(matches!(
            resolved.singular_supports().get("body"),
            Some(SpatialSupport::Volume { dimensions: 2, .. })
        ));
        let set = resolved
            .boundary_set("exterior")
            .expect("proved exterior is retained");
        let ResolvedBoundarySet::Explicit(explicit) = set else {
            panic!("source boundaries binding remains explicitly typed");
        };
        assert_eq!(explicit.members().len(), 4);
        assert_eq!(
            explicit
                .member(&"y_upper".to_owned())
                .map(|member| member.target()),
            Some("flat::y_upper")
        );
        assert_eq!(
            explicit
                .witness()
                .sides()
                .iter()
                .map(|side| (side.axis(), side.side(), side.boundary().as_str()))
                .collect::<Vec<_>>(),
            vec![
                (0, BoundarySide::Lower, "x_lower"),
                (0, BoundarySide::Upper, "x_upper"),
                (1, BoundarySide::Lower, "y_lower"),
                (1, BoundarySide::Upper, "y_upper"),
            ]
        );
    }

    #[test]
    fn forwarding_preserves_the_proved_identity_keyed_member_catalog() {
        let document = parse(&format!("{EXTERIOR_COMPONENT}{EXTERIOR_MODEL}"));
        let mut budget = CompleteExteriorMembershipBudget::new(CompleteExteriorLimits::default());
        let explicit = resolve_complete_exterior(&document, "explicit", &mut budget, |_| None)
            .expect("source exterior proves")
            .boundary_set("exterior")
            .expect("source set exists")
            .clone();
        let forwarded = resolve_complete_exterior(&document, "forwarded", &mut budget, |target| {
            (target == "enclosing_exterior").then(|| explicit.clone())
        })
        .expect("proved exterior may be forwarded");

        assert_eq!(
            budget.total_memberships(),
            4,
            "forwarding shares a proof and consumes no explicit membership budget"
        );
        let set = forwarded
            .boundary_set("exterior")
            .expect("forwarded set exists");
        let ResolvedBoundarySet::Forwarded(binding) = set else {
            panic!("forwarding remains a distinct binding kind");
        };
        assert_eq!(binding.target(), "enclosing_exterior");
        assert_eq!(binding.members(), explicit.members());
        assert_eq!(
            binding
                .member(&"x_lower".to_owned())
                .map(|member| member.target()),
            Some("flat::x_lower")
        );
        assert_eq!(binding.witness(), explicit.witness());
    }

    #[test]
    fn complete_exterior_memberships_have_independent_per_set_and_total_limits() {
        let document = parse(&format!("{EXTERIOR_COMPONENT}{EXTERIOR_MODEL}"));
        let mut per_set = CompleteExteriorMembershipBudget::new(CompleteExteriorLimits {
            max_members_per_set: 3,
            max_total_memberships: 100,
        });
        let diagnostics = resolve_complete_exterior(&document, "explicit", &mut per_set, |_| None)
            .expect_err("four members exceed the independent per-set limit");
        assert_eq!(per_set.total_memberships(), 0);
        assert!(
            messages(&diagnostics)
                .iter()
                .any(|message| { message.contains("has 4 members, exceeding the 3 member limit") })
        );

        let mut total = CompleteExteriorMembershipBudget::new(CompleteExteriorLimits {
            max_members_per_set: 4,
            max_total_memberships: 4,
        });
        resolve_complete_exterior(&document, "explicit", &mut total, |_| None)
            .expect("first set fits total budget");
        let diagnostics = resolve_complete_exterior(&document, "explicit", &mut total, |_| None)
            .expect_err("second set exceeds total budget");
        assert_eq!(total.total_memberships(), 4);
        assert!(messages(&diagnostics).iter().any(|message| {
            message.contains("require 8 total memberships, exceeding the 4 membership limit")
        }));
    }

    #[test]
    fn complete_exterior_proof_failures_remain_source_located() {
        let source = format!(
            r#"{EXTERIOR_COMPONENT}
model Use {{
  domain body = box(0, 1, 0, 1);
  domain other = box(0, 1, 0, 1);
  domain x_lower = boundary(body, axis = 0, side = lower);
  domain x_upper = boundary(body, axis = 0, side = upper);
  domain y_lower = boundary(body, axis = 1, side = lower);
  domain other_upper = boundary(other, axis = 1, side = upper);
  instance invalid: BoundaryFamily(
    support body = body,
    support exterior = boundaries(x_lower, x_upper, y_lower, other_upper)
  );
}}
"#
        );
        let document = parse(&source);
        let mut budget = CompleteExteriorMembershipBudget::new(CompleteExteriorLimits::default());
        let diagnostics = resolve_complete_exterior(&document, "invalid", &mut budget, |_| None)
            .expect_err("a same-dimensional side of another parent is rejected");

        let diagnostic = diagnostics
            .iter()
            .find(|diagnostic| diagnostic.message().contains("different exact parent"))
            .expect("wrong-parent proof diagnostic exists");
        let expected_range = instance(&document, "invalid")
            .boundary_set_bindings()
            .first()
            .expect("set binding exists")
            .members()
            .last()
            .expect("wrong member exists")
            .range();
        let span = diagnostic
            .source_span()
            .expect("proof failure retains the exact member span");
        assert_eq!(
            (span.start, span.end),
            (expected_range.start(), expected_range.end())
        );
    }

    #[test]
    fn required_unknown_and_duplicate_bindings_fail_closed() {
        let cases = [
            (
                "missing",
                "support body = fluid",
                "has no binding for required support slot `interface`",
            ),
            (
                "unknown",
                "support body = fluid, support interface = wall, support ghost = wall",
                "unknown support slot `ghost`",
            ),
            (
                "duplicate",
                "support body = fluid, support body = fluid, support interface = wall",
                "duplicate binding for support slot `body`",
            ),
        ];

        for (name, bindings, expected) in cases {
            let source = format!(
                r#"{COMPONENT}
model Use {{
  domain fluid = box(0, 1, 0, 1);
  domain wall = boundary(fluid, axis = 0, side = lower);
  instance probe: BoundaryState({bindings});
}}
"#
            );
            let document = parse(&source);
            let component = component(&document, "BoundaryState");
            let interface = interface(&document, "BoundaryState");
            let supports = spatial_supports(&document);
            let diagnostics = resolve_instance_supports(
                "supports.eqi",
                component,
                &interface,
                instance(&document, "probe"),
                |target| supports.get(target).cloned(),
            )
            .expect_err(name);

            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message().contains(expected)),
                "{name}: {:?}",
                messages(&diagnostics)
            );
        }
    }

    #[test]
    fn private_support_slots_are_rejected_at_the_definition_boundary() {
        let document = parse(
            r#"
component HiddenSupport {
  support body: volume(ambient_dimension = 2);
}
model Use {}
"#,
        );
        let diagnostics =
            component_support_interface("supports.eqi", component(&document, "HiddenSupport"))
                .expect_err("private support slots are uninhabitable in v1");

        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message()
                .contains("support slot `body` must be public")
        }));
    }

    #[test]
    fn volume_and_boundary_kind_mismatches_fail_symmetrically() {
        let cases = [
            (
                "volume bound to boundary",
                "support body = wall, support interface = wall",
                "support slot `body` requires volume support, found boundary",
            ),
            (
                "boundary bound to volume",
                "support body = fluid, support interface = fluid",
                "support slot `interface` requires boundary support, found volume",
            ),
        ];

        for (name, bindings, expected) in cases {
            let source = format!(
                r#"{COMPONENT}
model Use {{
  domain fluid = box(0, 1, 0, 1);
  domain wall = boundary(fluid, axis = 0, side = lower);
  instance probe: BoundaryState({bindings});
}}
"#
            );
            let document = parse(&source);
            let component = component(&document, "BoundaryState");
            let interface = interface(&document, "BoundaryState");
            let supports = spatial_supports(&document);
            let diagnostics = resolve_instance_supports(
                "supports.eqi",
                component,
                &interface,
                instance(&document, "probe"),
                |target| supports.get(target).cloned(),
            )
            .expect_err(name);

            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message().contains(expected)),
                "{name}: {:?}",
                messages(&diagnostics)
            );
        }
    }

    #[test]
    fn ambient_dimension_mismatches_fail_for_volume_and_boundary_slots() {
        let cases = [
            (
                "volume dimension",
                "support body = line, support interface = wall",
                "volume support slot `body` requires ambient dimension 2",
            ),
            (
                "boundary dimension",
                "support body = fluid, support interface = point",
                "boundary support slot `interface` requires ambient dimension 2",
            ),
        ];

        for (name, bindings, expected) in cases {
            let source = format!(
                r#"{COMPONENT}
model Use {{
  domain fluid = box(0, 1, 0, 1);
  domain wall = boundary(fluid, axis = 0, side = lower);
  domain line = box(0, 1);
  domain point = boundary(line, axis = 0, side = lower);
  instance probe: BoundaryState({bindings});
}}
"#
            );
            let document = parse(&source);
            let component = component(&document, "BoundaryState");
            let interface = interface(&document, "BoundaryState");
            let supports = spatial_supports(&document);
            let diagnostics = resolve_instance_supports(
                "supports.eqi",
                component,
                &interface,
                instance(&document, "probe"),
                |target| supports.get(target).cloned(),
            )
            .expect_err(name);

            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message().contains(expected)),
                "{name}: {:?}",
                messages(&diagnostics)
            );
        }
    }

    #[test]
    fn boundary_binding_must_share_the_exact_bound_parent() {
        let source = format!(
            r#"{COMPONENT}
model Use {{
  domain fluid = box(0, 1, 0, 1);
  domain wall = boundary(fluid, axis = 0, side = lower);
  domain other = box(0, 1, 0, 1);
  domain other_wall = boundary(other, axis = 0, side = lower);
  instance probe: BoundaryState(
    support body = fluid,
    support interface = other_wall
  );
}}
"#
        );
        let document = parse(&source);
        let component = component(&document, "BoundaryState");
        let interface = interface(&document, "BoundaryState");
        let supports = spatial_supports(&document);
        let diagnostics = resolve_instance_supports(
            "supports.eqi",
            component,
            &interface,
            instance(&document, "probe"),
            |target| supports.get(target).cloned(),
        )
        .expect_err("a same-dimensional boundary of another volume is not interchangeable");

        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message()
                .contains("is not BoundaryOf its exact bound parent slot `body`")
        }));
    }

    #[test]
    fn support_declaration_order_does_not_change_the_interface() {
        let body_first = parse(
            r#"
component C {
  public support body: volume(ambient_dimension = 2);
  public support wall: boundary(parent = body);
}
model Use {}
"#,
        );
        let boundary_first = parse(
            r#"
component C {
  public support wall: boundary(parent = body);
  public support body: volume(ambient_dimension = 2);
}
model Use {}
"#,
        );
        let body_first = interface(&body_first, "C");
        let boundary_first = interface(&boundary_first, "C");

        let shape = |interface: &SupportInterface| {
            interface
                .iter()
                .map(|(name, contract)| (name.to_owned(), contract.support().clone()))
                .collect::<BTreeMap<_, _>>()
        };
        assert_eq!(shape(&body_first), shape(&boundary_first));
    }
}
