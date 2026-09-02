//! Source-oriented syntax tree. Semantic types are assigned during lowering.

mod compile_time;
pub(crate) mod document;
pub(crate) mod formulation;

pub(crate) use compile_time::DimensionDecl;
pub use compile_time::{LetDecl, ParameterDecl};
pub use document::Document;

use crate::ast_property::{ComponentPropertyDecl, PropertyBindingDecl};
use formulation::FormulationDecl;
use std::ops::Range;

pub use crate::cartesian::{BoundarySideSyntax, CartesianCoordinateSyntax};

/// Half-open UTF-8 byte range in one source file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct TextRange {
    pub(crate) start: u32,
    pub(crate) end: u32,
}

impl TextRange {
    /// Construct a byte range.
    #[must_use]
    pub const fn new(start: u32, end: u32) -> Self {
        Self { start, end }
    }

    /// Inclusive start byte.
    #[must_use]
    pub const fn start(self) -> u32 {
        self.start
    }

    /// Exclusive end byte.
    #[must_use]
    pub const fn end(self) -> u32 {
        self.end
    }
}

/// One nonempty source name, optionally qualified by lexical member selection.
///
/// The spelling is retained once while segment ranges make qualification
/// structural. Consumers never need to split or concatenate dotted strings.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NamePath {
    pub(crate) text: String,
    pub(crate) segments: Vec<Range<usize>>,
    pub(crate) range: TextRange,
}

impl NamePath {
    pub(crate) fn from_parsed_segments(
        segments: impl IntoIterator<Item = String>,
        range: TextRange,
    ) -> Self {
        let mut text = String::new();
        let mut ranges = Vec::new();
        for segment in segments {
            if !text.is_empty() {
                text.push('.');
            }
            let start = text.len();
            text.push_str(&segment);
            ranges.push(start..text.len());
        }
        debug_assert!(!ranges.is_empty(), "a NamePath is nonempty");
        Self {
            text,
            segments: ranges,
            range,
        }
    }

    pub(crate) fn single(name: String, range: TextRange) -> Self {
        Self::from_parsed_segments([name], range)
    }

    pub(crate) fn with_range(mut self, range: TextRange) -> Self {
        self.range = range;
        self
    }

    /// Canonical dotted source spelling.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.text
    }

    /// Identifier segments in lexical order.
    #[must_use]
    pub fn segments(&self) -> impl ExactSizeIterator<Item = &str> {
        self.segments.iter().map(|range| &self.text[range.clone()])
    }

    /// Full qualified-name range.
    #[must_use]
    pub const fn range(&self) -> TextRange {
        self.range
    }
}

/// One exact, side-effect-free operator definition in source form.
///
/// This syntax is deliberately separate from model expressions. It admits
/// only exact rationals, component selection, Kronecker deltas, and bounded
/// arithmetic, so later lowering never has to recover purity from a general
/// expression tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PureOperatorDecl {
    pub(crate) visibility: VisibilitySyntax,
    pub(crate) name: String,
    pub(crate) formals: Vec<PureOperatorFormal>,
    pub(crate) result: PureValueClassSyntax,
    pub(crate) body: PureOperatorExpr,
    pub(crate) range: TextRange,
}

impl PureOperatorDecl {
    /// Package visibility. Unqualified declarations are private by default.
    #[must_use]
    pub const fn visibility(&self) -> VisibilitySyntax {
        self.visibility
    }

    /// Source name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Ordered formal arguments.
    #[must_use]
    pub fn formals(&self) -> &[PureOperatorFormal] {
        &self.formals
    }

    /// Declared result value class.
    #[must_use]
    pub const fn result(&self) -> &PureValueClassSyntax {
        &self.result
    }

    /// Exact bounded operator body.
    #[must_use]
    pub const fn body(&self) -> &PureOperatorExpr {
        &self.body
    }

    /// Full declaration range, including visibility and trailing semicolon.
    #[must_use]
    pub const fn range(&self) -> TextRange {
        self.range
    }
}

/// One ordered pure-operator formal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PureOperatorFormal {
    pub(crate) name: String,
    pub(crate) value_class: PureValueClassSyntax,
    pub(crate) range: TextRange,
}

impl PureOperatorFormal {
    /// Formal name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Declared value class.
    #[must_use]
    pub const fn value_class(&self) -> &PureValueClassSyntax {
        &self.value_class
    }

    /// Full formal range.
    #[must_use]
    pub const fn range(&self) -> TextRange {
        self.range
    }
}

/// Closed source value classes admitted by a pure operator definition.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum PureValueClassSyntax {
    /// One scalar value.
    Scalar,
    /// A spatial value whose rank is retained as exact source syntax.
    Spatial {
        /// Exact tensor rank.
        rank: ExactIntegerSyntax,
    },
}

/// One exact nonnegative integer token with its original source spelling.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ExactIntegerSyntax {
    pub(crate) spelling: String,
    pub(crate) value: u64,
    pub(crate) range: TextRange,
}

impl ExactIntegerSyntax {
    /// Exact source spelling, without sign or radix prefix.
    #[must_use]
    pub fn spelling(&self) -> &str {
        &self.spelling
    }

    /// Parsed exact value.
    #[must_use]
    pub const fn value(&self) -> u64 {
        self.value
    }

    /// Integer-token range.
    #[must_use]
    pub const fn range(&self) -> TextRange {
        self.range
    }
}

/// Exact expression admitted inside a pure operator declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PureOperatorExpr {
    pub(crate) kind: PureOperatorExprKind,
    pub(crate) range: TextRange,
}

impl PureOperatorExpr {
    /// Exact expression form.
    #[must_use]
    pub const fn kind(&self) -> &PureOperatorExprKind {
        &self.kind
    }

    /// Full expression range, including explicit parentheses when present.
    #[must_use]
    pub const fn range(&self) -> TextRange {
        self.range
    }
}

/// Closed exact expression vocabulary for pure operators.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum PureOperatorExprKind {
    /// Exact rational literal `rational(numerator, denominator)`.
    Rational {
        /// Nonnegative numerator; sign is represented by [`Self::Neg`].
        numerator: ExactIntegerSyntax,
        /// Strictly positive denominator.
        denominator: ExactIntegerSyntax,
    },
    /// Select a formal component using one output axis per formal axis.
    Component {
        /// Referenced formal name.
        formal: String,
        /// Exact range of the formal-name occurrence.
        formal_range: TextRange,
        /// Ordered result-axis sequence; empty selects a scalar formal.
        result_axes: Vec<ExactIntegerSyntax>,
    },
    /// Kronecker delta between two result axes.
    Delta {
        /// Left result axis.
        left_axis: ExactIntegerSyntax,
        /// Right result axis.
        right_axis: ExactIntegerSyntax,
    },
    /// Exact prefix negation.
    Neg(Box<PureOperatorExpr>),
    /// Exact infix arithmetic.
    Binary {
        /// Arithmetic operator.
        op: PureOperatorBinaryOp,
        /// Left operand.
        left: Box<PureOperatorExpr>,
        /// Right operand.
        right: Box<PureOperatorExpr>,
    },
}

/// Infix operators admitted by a pure operator body.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PureOperatorBinaryOp {
    /// Addition.
    Add,
    /// Subtraction.
    Sub,
    /// Multiplication.
    Mul,
}

/// A nominal compilation-unit connector family.
#[derive(Debug, Clone, PartialEq)]
pub struct ConnectorDecl {
    pub(crate) visibility: VisibilitySyntax,
    pub(crate) name: String,
    pub(crate) syntax: ConnectorSyntax,
    pub(crate) range: TextRange,
}

impl ConnectorDecl {
    /// Package visibility. Unqualified declarations are private by default.
    #[must_use]
    pub const fn visibility(&self) -> VisibilitySyntax {
        self.visibility
    }

    /// Source name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Closed connector-family contract.
    #[must_use]
    pub const fn syntax(&self) -> &ConnectorSyntax {
        &self.syntax
    }

    /// Full connector declaration range, including a visibility modifier.
    #[must_use]
    pub const fn range(&self) -> TextRange {
        self.range
    }
}

/// Source contract for one nominal connector family.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum ConnectorSyntax {
    /// Scalar acausal connector with across and through SI dimensions.
    ScalarPhysical {
        /// Dimension of the across variable.
        across_dimension: Expr,
        /// Dimension of the through variable.
        through_dimension: Expr,
    },
    /// Field-valued trace/flux pair on one oriented boundary support.
    FieldPhysical {
        /// Pointwise trace quantity.
        trace: ConnectorQuantitySyntax,
        /// Pointwise outward-flux quantity dual to `trace`.
        flux: ConnectorQuantitySyntax,
        /// Exact value shape, or one source convenience resolved on lowering.
        shape: ValueShapeSyntax,
        /// Coordinate-frame discipline shared by both quantities.
        frame: FrameSyntax,
        /// Closed boundary duality used by conserving connection sets.
        pairing: BoundaryPairingSyntax,
    },
}

/// One named quantity member of a field-valued physical Connector.
#[derive(Debug, Clone, PartialEq)]
pub struct ConnectorQuantitySyntax {
    pub(crate) name: String,
    pub(crate) dimension: Expr,
}

impl ConnectorQuantitySyntax {
    /// Source member name. Its declaration identity is assigned on lowering.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Static SI dimension expression.
    #[must_use]
    pub const fn dimension(&self) -> &Expr {
        &self.dimension
    }
}

/// Source spelling of an exact value shape or a context-dependent convenience.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ValueShapeSyntax {
    /// Rank-zero scalar, canonically formatted as `[]`.
    Scalar,
    /// Exact positive extents, canonically spelled `[e0, e1, ...]`.
    Exact(Vec<u32>),
    /// Vector whose extent is the parent support's ambient dimension.
    SpatialVector,
}

/// Source frame discipline for field-valued physical quantities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum FrameSyntax {
    /// Frame-independent scalar or tensor components.
    Invariant,
    /// Components in the model-global Cartesian spatial frame.
    Spatial,
}

/// Source boundary pairing for one trace/flux dual pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum BoundaryPairingSyntax {
    /// Pointwise Euclidean contraction followed by boundary integration.
    EuclideanBoundaryDuality,
}

/// A reusable typed source definition before deterministic elaboration.
#[derive(Debug, Clone, PartialEq)]
pub struct ComponentDecl {
    pub(crate) visibility: VisibilitySyntax,
    pub(crate) name: String,
    pub(crate) items: Vec<ComponentItem>,
    pub(crate) formulations: Vec<FormulationDecl>,
    pub(crate) property_requirements: Vec<ComponentPropertyDecl>,
    pub(crate) range: TextRange,
}

impl ComponentDecl {
    /// Package visibility. Unqualified declarations are private by default.
    #[must_use]
    pub const fn visibility(&self) -> VisibilitySyntax {
        self.visibility
    }

    /// Source name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Component declarations in source order.
    #[must_use]
    pub fn items(&self) -> &[ComponentItem] {
        &self.items
    }
}

/// Source declaration visibility. Absence of a modifier parses as private.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum VisibilitySyntax {
    /// Visible only inside the owning component or package scope.
    #[default]
    Private,
    /// Part of the owning component or package's typed public interface.
    Public,
}

/// Component-local scalar Parameter declaration.
///
/// Visibility belongs to this type, so no Relation or instance can be made
/// public by constructing a generic decorated declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct ComponentParameterDecl {
    pub(crate) visibility: VisibilitySyntax,
    pub(crate) name: String,
    pub(crate) dimension: Expr,
    pub(crate) default: Option<Expr>,
    pub(crate) range: TextRange,
}

impl ComponentParameterDecl {
    /// Private-by-default or explicit public visibility.
    #[must_use]
    pub const fn visibility(&self) -> VisibilitySyntax {
        self.visibility
    }

    /// Source name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Static SI dimension expression.
    #[must_use]
    pub const fn dimension(&self) -> &Expr {
        &self.dimension
    }

    /// Optional compile-time default expression. `None` is a required public
    /// binding; semantic validation rejects an unbound private Parameter.
    #[must_use]
    pub const fn default(&self) -> Option<&Expr> {
        self.default.as_ref()
    }

    /// Full declaration range, including `public` when present.
    #[must_use]
    pub const fn range(&self) -> TextRange {
        self.range
    }
}

/// Component-local Port declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct ComponentPortDecl {
    pub(crate) visibility: VisibilitySyntax,
    pub(crate) name: String,
    pub(crate) syntax: PortSyntax,
    pub(crate) range: TextRange,
}

/// One field-physical Port declaration expanded over a complete exterior.
///
/// This is a hierarchy-only family. The binder is not a general collection,
/// array, or runtime loop and no family value survives component elaboration.
#[derive(Debug, Clone, PartialEq)]
pub struct ComponentPortFamilyDecl {
    pub(crate) port: ComponentPortDecl,
    pub(crate) binder: BoundaryFamilyBinderSyntax,
}

impl ComponentPortFamilyDecl {
    /// Underlying field-physical Port declaration.
    #[must_use]
    pub const fn port(&self) -> &ComponentPortDecl {
        &self.port
    }

    /// Restricted boundary-member binder.
    #[must_use]
    pub const fn binder(&self) -> &BoundaryFamilyBinderSyntax {
        &self.binder
    }

    /// Full family declaration range.
    #[must_use]
    pub const fn range(&self) -> TextRange {
        self.port.range
    }
}

impl ComponentPortDecl {
    /// Private-by-default or explicit public visibility.
    #[must_use]
    pub const fn visibility(&self) -> VisibilitySyntax {
        self.visibility
    }

    /// Source name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Causal or conserving interface contract.
    #[must_use]
    pub const fn syntax(&self) -> &PortSyntax {
        &self.syntax
    }

    /// Full declaration range, including `public` when present.
    #[must_use]
    pub const fn range(&self) -> TextRange {
        self.range
    }
}

/// Component-local spatial-support interface.
///
/// A support slot is not a scalar Parameter and never contains mesh or
/// discretization data. An instance binds it to one exact enclosing Domain
/// before deterministic component expansion.
#[derive(Debug, Clone, PartialEq)]
pub struct SupportSlotDecl {
    pub(crate) visibility: VisibilitySyntax,
    pub(crate) name: String,
    pub(crate) syntax: SupportSlotSyntax,
    pub(crate) range: TextRange,
}

impl SupportSlotDecl {
    /// Private-by-default or explicit public visibility.
    #[must_use]
    pub const fn visibility(&self) -> VisibilitySyntax {
        self.visibility
    }

    /// Source name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Closed spatial-support slot contract.
    #[must_use]
    pub const fn syntax(&self) -> &SupportSlotSyntax {
        &self.syntax
    }

    /// Full declaration range, including `public` when present.
    #[must_use]
    pub const fn range(&self) -> TextRange {
        self.range
    }
}

/// Source contract for one component spatial-support slot.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SupportSlotSyntax {
    /// A volume Domain with one exact ambient dimension.
    Volume {
        /// Required number of spatial coordinate axes.
        ambient_dimension: usize,
    },
    /// A boundary whose exact parent is supplied through another slot.
    Boundary {
        /// Name of the parent volume support slot in the same component.
        parent: String,
    },
    /// The complete exterior of an exact bound Cartesian volume.
    ///
    /// Members are supplied by one finite `boundaries(...)` occurrence
    /// binding. The set is hierarchy-only and never becomes a Kernel node.
    CompleteExterior {
        /// Name of the parent volume support slot in the same component.
        parent: String,
    },
}

/// Restricted binder for one member of a complete exterior support set.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BoundaryFamilyBinderSyntax {
    pub(crate) member: String,
    pub(crate) set: String,
    pub(crate) range: TextRange,
}

impl BoundaryFamilyBinderSyntax {
    /// Lexical name of the currently expanded boundary member.
    #[must_use]
    pub fn member(&self) -> &str {
        &self.member
    }

    /// Complete-exterior support slot traversed by this binder.
    #[must_use]
    pub fn set(&self) -> &str {
        &self.set
    }

    /// Full `[member in set]` range.
    #[must_use]
    pub const fn range(&self) -> TextRange {
        self.range
    }
}

/// Required occurrence-bound continuum Field interface.
///
/// A Field slot does not own state and does not become a Semantic Kernel node.
/// Each component occurrence binds it to one exact enclosing Field before
/// deterministic expansion. V1 slots are necessarily public and continuum,
/// so neither property is represented as mutable syntax state here.
#[derive(Debug, Clone, PartialEq)]
pub struct FieldSlotDecl {
    pub(crate) name: String,
    pub(crate) support: String,
    pub(crate) dimension: Expr,
    pub(crate) shape: Option<ValueShapeSyntax>,
    pub(crate) range: TextRange,
}

impl FieldSlotDecl {
    /// Public Field-slot name in this component definition.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Component support-slot name on which the required Field is defined.
    #[must_use]
    pub fn support(&self) -> &str {
        &self.support
    }

    /// Required physical dimension.
    #[must_use]
    pub const fn dimension(&self) -> &Expr {
        &self.dimension
    }

    /// Required value shape, with omission denoting the scalar source form.
    #[must_use]
    pub const fn shape(&self) -> Option<&ValueShapeSyntax> {
        self.shape.as_ref()
    }

    /// Full declaration range, including the required `public` modifier.
    #[must_use]
    pub const fn range(&self) -> TextRange {
        self.range
    }
}

/// Component-body declaration.
///
/// Parameter, Port, and Support carry general visibility. Field slots are
/// public by construction. Public Representations, owned Fields, Relations,
/// Connections, Clocks, and nested instances remain unrepresentable.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum ComponentItem {
    /// Scalar compile-time Parameter.
    Parameter(ComponentParameterDecl),
    /// Causal or conserving interface.
    Port(ComponentPortDecl),
    /// Field-physical Port family over one complete exterior.
    PortFamily(ComponentPortFamilyDecl),
    /// Required occurrence-bound spatial support.
    Support(SupportSlotDecl),
    /// Required occurrence-bound continuum Field.
    FieldSlot(FieldSlotDecl),
    /// Private canonical field representation.
    Representation(RepresentationDecl),
    /// Private mutable state.
    Field(FieldDecl),
    /// Private exact periodic clock.
    Clock(ClockDecl),
    /// Private implicit residual group.
    Relation(RelationDecl),
    /// Private continuous Relation family over one complete exterior.
    RelationFamily(RelationFamilyDecl),
    /// Private local connection.
    Connection(ConnectionDecl),
    /// Conserving boundary-family connection, optionally pointwise-bound.
    BoundaryConnection(BoundaryConnectionDecl),
    /// Private nested component instance.
    Instance(InstanceDecl),
}

/// A named model and its declarations.
#[derive(Debug, Clone, PartialEq)]
pub struct ModelDecl {
    pub(crate) name: String,
    pub(crate) items: Vec<Item>,
    pub(crate) range: TextRange,
}

impl ModelDecl {
    /// Source name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Declarations in source order.
    #[must_use]
    pub fn items(&self) -> &[Item] {
        &self.items
    }

    /// Full model declaration range.
    #[must_use]
    pub const fn range(&self) -> TextRange {
        self.range
    }
}

/// Model-level declaration.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum Item {
    /// Continuous spatial region or one oriented boundary portion.
    Domain(DomainDecl),
    /// Canonical field representation before discretization.
    Representation(RepresentationDecl),
    /// Mutable model state.
    Field(FieldDecl),
    /// Revision-local design value.
    Parameter(ParameterDecl),
    /// Typed compile-time expression alias expanded before Kernel lowering.
    Let(LetDecl),
    /// Causal or conserving interface.
    Port(PortDecl),
    /// Exact periodic clock.
    Clock(ClockDecl),
    /// Implicit residual group and activation.
    Relation(RelationDecl),
    /// Signal or conserving connection net.
    Connection(ConnectionDecl),
    /// Conserving connection containing exact boundary-member selectors.
    BoundaryConnection(BoundaryConnectionDecl),
    /// Public model Port list.
    Boundary(BoundaryDecl),
    /// Compile-time component instance.
    Instance(InstanceDecl),
}

/// One named compile-time component instance.
#[derive(Debug, Clone, PartialEq)]
pub struct InstanceDecl {
    pub(crate) name: String,
    pub(crate) definition: NamePath,
    pub(crate) bindings: Vec<ParameterBindingDecl>,
    pub(crate) support_bindings: Vec<SupportBindingDecl>,
    pub(crate) boundary_set_bindings: Vec<BoundarySetBindingDecl>,
    pub(crate) field_bindings: Vec<FieldBindingDecl>,
    pub(crate) property_bindings: Vec<PropertyBindingDecl>,
    pub(crate) range: TextRange,
}

impl InstanceDecl {
    /// Source-declared instance path segment.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Lexically resolved component definition name before semantic lookup.
    #[must_use]
    pub const fn definition(&self) -> &NamePath {
        &self.definition
    }

    /// Named compile-time Parameter bindings in source order.
    #[must_use]
    pub fn bindings(&self) -> &[ParameterBindingDecl] {
        &self.bindings
    }

    /// Named spatial-support bindings in source order.
    #[must_use]
    pub fn support_bindings(&self) -> &[SupportBindingDecl] {
        &self.support_bindings
    }

    /// Named complete-exterior member-set bindings in source order.
    #[must_use]
    pub fn boundary_set_bindings(&self) -> &[BoundarySetBindingDecl] {
        &self.boundary_set_bindings
    }

    /// Named occurrence-bound Field bindings in source order.
    #[must_use]
    pub fn field_bindings(&self) -> &[FieldBindingDecl] {
        &self.field_bindings
    }

    /// Full instance declaration range.
    #[must_use]
    pub const fn range(&self) -> TextRange {
        self.range
    }
}

/// One named binding in a component instantiation.
#[derive(Debug, Clone, PartialEq)]
pub struct ParameterBindingDecl {
    pub(crate) parameter: String,
    pub(crate) value: Expr,
    pub(crate) range: TextRange,
}

impl ParameterBindingDecl {
    /// Public Parameter name in the selected component definition.
    #[must_use]
    pub fn parameter(&self) -> &str {
        &self.parameter
    }

    /// Pure compile-time scalar expression before semantic validation.
    #[must_use]
    pub const fn value(&self) -> &Expr {
        &self.value
    }

    /// Complete binding range.
    #[must_use]
    pub const fn range(&self) -> TextRange {
        self.range
    }
}

/// One named spatial-support binding in a component instantiation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SupportBindingDecl {
    pub(crate) slot: String,
    pub(crate) target: String,
    pub(crate) range: TextRange,
}

impl SupportBindingDecl {
    /// Public support-slot name in the selected component definition.
    #[must_use]
    pub fn slot(&self) -> &str {
        &self.slot
    }

    /// Enclosing Domain or support-slot name bound to this occurrence.
    #[must_use]
    pub fn target(&self) -> &str {
        &self.target
    }

    /// Complete binding range, including the `support` discriminator.
    #[must_use]
    pub const fn range(&self) -> TextRange {
        self.range
    }
}

/// One finite `boundaries(...)` binding for a complete exterior support slot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundarySetBindingDecl {
    pub(crate) slot: String,
    pub(crate) members: Vec<BoundarySetMemberSyntax>,
    pub(crate) range: TextRange,
}

impl BoundarySetBindingDecl {
    /// Public complete-exterior support-slot name.
    #[must_use]
    pub fn slot(&self) -> &str {
        &self.slot
    }

    /// Explicit finite boundary members in source order.
    #[must_use]
    pub fn members(&self) -> &[BoundarySetMemberSyntax] {
        &self.members
    }

    /// Complete binding range, including the `support` discriminator.
    #[must_use]
    pub const fn range(&self) -> TextRange {
        self.range
    }
}

/// One lexically named member of a complete exterior binding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundarySetMemberSyntax {
    pub(crate) target: String,
    pub(crate) range: TextRange,
}

impl BoundarySetMemberSyntax {
    /// Enclosing boundary Domain selected by this member.
    #[must_use]
    pub fn target(&self) -> &str {
        &self.target
    }

    /// Exact source range of the member spelling.
    #[must_use]
    pub const fn range(&self) -> TextRange {
        self.range
    }
}

/// One named occurrence-bound Field binding in a component instantiation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldBindingDecl {
    pub(crate) slot: String,
    pub(crate) target: String,
    pub(crate) range: TextRange,
}

impl FieldBindingDecl {
    /// Public Field-slot name in the selected component definition.
    #[must_use]
    pub fn slot(&self) -> &str {
        &self.slot
    }

    /// Enclosing owned Field or forwarded Field-slot name.
    #[must_use]
    pub fn target(&self) -> &str {
        &self.target
    }

    /// Complete binding range, including the `field` discriminator.
    #[must_use]
    pub const fn range(&self) -> TextRange {
        self.range
    }
}

/// Named semantic Domain declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct DomainDecl {
    pub(crate) name: String,
    pub(crate) syntax: DomainSyntax,
    pub(crate) range: TextRange,
}

impl DomainDecl {
    /// Source name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Domain family and its source-level contract.
    #[must_use]
    pub const fn syntax(&self) -> &DomainSyntax {
        &self.syntax
    }

    /// Full declaration range.
    #[must_use]
    pub const fn range(&self) -> TextRange {
        self.range
    }
}

/// Source contract for one canonical Domain.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum DomainSyntax {
    /// Cartesian coordinate sources, one lower/upper pair per axis.
    CartesianBox(Vec<(CartesianCoordinateSyntax, CartesianCoordinateSyntax)>),
    /// One oriented side of a named parent Cartesian box.
    Boundary {
        /// Parent Domain name.
        parent: String,
        /// Zero-based coordinate axis.
        axis: usize,
        /// Lower or upper side.
        side: BoundarySideSyntax,
    },
    /// One nominal scalar conserving domain. The declaration identity, not
    /// dimension coincidence, determines Port compatibility.
    ScalarPhysical {
        /// Dimension of the across variable.
        across_dimension: Expr,
        /// Dimension of the through variable.
        through_dimension: Expr,
    },
}

/// Canonical Representation declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepresentationDecl {
    pub(crate) name: String,
    pub(crate) syntax: RepresentationSyntax,
    pub(crate) range: TextRange,
}

impl RepresentationDecl {
    /// Source name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Representation family.
    #[must_use]
    pub const fn syntax(&self) -> RepresentationSyntax {
        self.syntax
    }

    /// Full declaration range.
    #[must_use]
    pub const fn range(&self) -> TextRange {
        self.range
    }
}

/// Source representation family.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum RepresentationSyntax {
    /// Continuous field before a discrete function space is selected.
    Continuum,
}

/// Field source declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct FieldDecl {
    pub(crate) name: String,
    pub(crate) domain: Option<String>,
    pub(crate) representation: Option<String>,
    pub(crate) shape: Option<ValueShapeSyntax>,
    pub(crate) dimension: Expr,
    pub(crate) initial: Option<f64>,
    pub(crate) range: TextRange,
}

impl FieldDecl {
    /// Source name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Spatial Domain when this is a distributed Field.
    #[must_use]
    pub fn domain(&self) -> Option<&str> {
        self.domain.as_deref()
    }

    /// Canonical Representation when this is a distributed Field.
    #[must_use]
    pub fn representation(&self) -> Option<&str> {
        self.representation.as_deref()
    }

    /// Optional source value shape. Absence preserves legacy scalar syntax.
    #[must_use]
    pub const fn shape(&self) -> Option<&ValueShapeSyntax> {
        self.shape.as_ref()
    }

    /// Static SI dimension expression.
    #[must_use]
    pub const fn dimension(&self) -> &Expr {
        &self.dimension
    }

    /// Scalar initial literal in coherent SI units.
    ///
    /// Non-scalar Fields have no initial until a shaped-value source and wire
    /// contract exists; absence never means an implicit zero broadcast.
    #[must_use]
    pub const fn initial(&self) -> Option<f64> {
        self.initial
    }

    /// Full declaration range.
    #[must_use]
    pub const fn range(&self) -> TextRange {
        self.range
    }
}

/// Port declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct PortDecl {
    pub(crate) name: String,
    pub(crate) syntax: PortSyntax,
    pub(crate) range: TextRange,
}

impl PortDecl {
    /// Source name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Causal/conserving contract.
    #[must_use]
    pub const fn syntax(&self) -> &PortSyntax {
        &self.syntax
    }

    /// Full declaration range.
    #[must_use]
    pub const fn range(&self) -> TextRange {
        self.range
    }
}

/// Source-level Port contract.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum PortSyntax {
    /// Causal scalar signal.
    Signal {
        /// Causal direction.
        direction: SignalDirectionSyntax,
        /// Static SI dimension.
        dimension: Expr,
    },
    /// Structural-only legacy conserving marker.
    ConservingMarker {
        /// Saved scalar dimension retained for legacy model meaning.
        dimension: Expr,
    },
    /// Executable scalar conserving Port, nominally typed by one Domain.
    /// In the flat source slice, a Relation that reads this Port with
    /// `across(...)` or `through(...)` is its owner: lowering emits both
    /// `DependsOn` and `HasPort`. Cross-component physical observation is not
    /// inferred from an accessor.
    ScalarPhysical {
        /// Owning scalar physical Domain name.
        domain: String,
    },
    /// Component interface typed by a nominal scalar physical Connector.
    ScalarPhysicalConnector {
        /// Connector declaration selected in lexical scope.
        connector: NamePath,
    },
    /// Field-valued conserving interface on one component spatial support.
    FieldPhysical {
        /// Nominal field-physical Connector selected in lexical scope.
        connector: NamePath,
        /// Boundary support slot (in a Component) or Domain (in a Model).
        support: String,
    },
}

/// Source-level signal direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SignalDirectionSyntax {
    /// Value enters the model.
    Input,
    /// Value leaves the model.
    Output,
}

/// Exact periodic clock declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClockDecl {
    pub(crate) name: String,
    pub(crate) period: RationalSyntax,
    pub(crate) phase: RationalSyntax,
    pub(crate) range: TextRange,
}

impl ClockDecl {
    /// Source name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Exact period in seconds.
    #[must_use]
    pub const fn period(&self) -> RationalSyntax {
        self.period
    }

    /// Exact phase in seconds.
    #[must_use]
    pub const fn phase(&self) -> RationalSyntax {
        self.phase
    }

    /// Full declaration range.
    #[must_use]
    pub const fn range(&self) -> TextRange {
        self.range
    }
}

/// Unreduced rational literal; semantic validation occurs during lowering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RationalSyntax {
    pub(crate) numerator: u64,
    pub(crate) denominator: u64,
}

impl RationalSyntax {
    /// Numerator.
    #[must_use]
    pub const fn numerator(self) -> u64 {
        self.numerator
    }

    /// Denominator.
    #[must_use]
    pub const fn denominator(self) -> u64 {
        self.denominator
    }
}

/// Relation declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct RelationDecl {
    pub(crate) name: String,
    pub(crate) activation: ActivationSyntax,
    pub(crate) domain: Option<String>,
    pub(crate) residuals: Vec<Expr>,
    pub(crate) range: TextRange,
}

impl RelationDecl {
    /// Source name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Continuous or exact-periodic activation.
    #[must_use]
    pub const fn activation(&self) -> &ActivationSyntax {
        &self.activation
    }

    /// Domain on which the residuals hold, for a spatial Relation.
    #[must_use]
    pub fn domain(&self) -> Option<&str> {
        self.domain.as_deref()
    }

    /// Residual left-hand sides, each interpreted as `expression = 0`.
    #[must_use]
    pub fn residuals(&self) -> &[Expr] {
        &self.residuals
    }

    /// Full declaration range.
    #[must_use]
    pub const fn range(&self) -> TextRange {
        self.range
    }
}

/// One continuous Relation expanded once per complete-exterior member.
#[derive(Debug, Clone, PartialEq)]
pub struct RelationFamilyDecl {
    pub(crate) relation: RelationDecl,
    pub(crate) binder: BoundaryFamilyBinderSyntax,
}

impl RelationFamilyDecl {
    /// Underlying continuous Relation declaration.
    #[must_use]
    pub const fn relation(&self) -> &RelationDecl {
        &self.relation
    }

    /// Restricted boundary-member binder.
    #[must_use]
    pub const fn binder(&self) -> &BoundaryFamilyBinderSyntax {
        &self.binder
    }

    /// Full family declaration range.
    #[must_use]
    pub const fn range(&self) -> TextRange {
        self.relation.range
    }
}

/// Source activation syntax.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ActivationSyntax {
    /// Active throughout model time.
    Continuous,
    /// Active at ticks of the named ClockDomain.
    Periodic(String),
}

/// Connection declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionDecl {
    pub(crate) syntax: ConnectionSyntax,
    pub(crate) ports: Vec<NamePath>,
    pub(crate) range: TextRange,
}

impl ConnectionDecl {
    /// Signal or conserving syntax.
    #[must_use]
    pub const fn syntax(&self) -> ConnectionSyntax {
        self.syntax
    }

    /// Port spellings. Signal order is output followed by inputs.
    ///
    /// This compatibility view preserves flat-source consumers. New code that
    /// resolves component members should use [`Self::port_paths`].
    #[must_use]
    pub fn ports(&self) -> impl ExactSizeIterator<Item = &str> {
        self.ports.iter().map(NamePath::as_str)
    }

    /// Structurally segmented Port selections in source order.
    #[must_use]
    pub fn port_paths(&self) -> &[NamePath] {
        &self.ports
    }

    /// Full declaration range.
    #[must_use]
    pub const fn range(&self) -> TextRange {
        self.range
    }
}

/// Source connection kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConnectionSyntax {
    /// One causal output followed by one or more inputs.
    Signal,
    /// Acausal connection net.
    Conserving,
    /// Exact spatial identification of two field-valued boundary Ports.
    SpatialPeriodic,
}

/// One boundary Connection containing field-valued Port references.
///
/// A binder denotes pointwise expansion over one complete exterior. Without
/// a binder, at least one Port reference carries an exact boundary selector.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundaryConnectionDecl {
    pub(crate) syntax: ConnectionSyntax,
    pub(crate) binder: Option<BoundaryFamilyBinderSyntax>,
    pub(crate) ports: Vec<BoundaryPortReferenceSyntax>,
    pub(crate) range: TextRange,
}

impl BoundaryConnectionDecl {
    /// Conserving or spatial-periodic boundary semantics.
    #[must_use]
    pub const fn syntax(&self) -> ConnectionSyntax {
        self.syntax
    }

    /// Optional pointwise family binder.
    #[must_use]
    pub const fn binder(&self) -> Option<&BoundaryFamilyBinderSyntax> {
        self.binder.as_ref()
    }

    /// Conserving Port references in source order.
    #[must_use]
    pub fn ports(&self) -> &[BoundaryPortReferenceSyntax] {
        &self.ports
    }

    /// Full Connection range.
    #[must_use]
    pub const fn range(&self) -> TextRange {
        self.range
    }
}

/// One Port path with an optional exact boundary-member selector.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundaryPortReferenceSyntax {
    pub(crate) port: NamePath,
    pub(crate) selector: Option<BoundaryPortSelectorSyntax>,
}

impl BoundaryPortReferenceSyntax {
    /// Structurally segmented Port path.
    #[must_use]
    pub const fn port(&self) -> &NamePath {
        &self.port
    }

    /// Exact boundary-member selector, when present.
    #[must_use]
    pub const fn selector(&self) -> Option<&BoundaryPortSelectorSyntax> {
        self.selector.as_ref()
    }
}

/// Closed `[member = target]` selector on one boundary-family Port.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BoundaryPortSelectorSyntax {
    pub(crate) member: String,
    pub(crate) target: String,
    pub(crate) range: TextRange,
}

impl BoundaryPortSelectorSyntax {
    /// Family binder name declared by the selected Port family.
    #[must_use]
    pub fn member(&self) -> &str {
        &self.member
    }

    /// Exact enclosing boundary Domain or active binder member.
    #[must_use]
    pub fn target(&self) -> &str {
        &self.target
    }

    /// Full `[member = target]` range.
    #[must_use]
    pub const fn range(&self) -> TextRange {
        self.range
    }
}

/// Model boundary declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundaryDecl {
    pub(crate) ports: Vec<NamePath>,
    pub(crate) range: TextRange,
}

impl BoundaryDecl {
    /// Boundary Port spellings.
    #[must_use]
    pub fn ports(&self) -> impl ExactSizeIterator<Item = &str> {
        self.ports.iter().map(NamePath::as_str)
    }

    /// Structurally segmented boundary Port selections.
    #[must_use]
    pub fn port_paths(&self) -> &[NamePath] {
        &self.ports
    }

    /// Full declaration range.
    #[must_use]
    pub const fn range(&self) -> TextRange {
        self.range
    }
}

/// Source expression with its exact byte range.
#[derive(Debug, Clone, PartialEq)]
pub struct Expr {
    pub(crate) kind: ExprKind,
    pub(crate) range: TextRange,
}

impl Expr {
    /// Expression form.
    #[must_use]
    pub const fn kind(&self) -> &ExprKind {
        &self.kind
    }

    /// Full expression range.
    #[must_use]
    pub const fn range(&self) -> TextRange {
        self.range
    }

    /// Rewrite structurally named references without reparsing source text.
    ///
    /// The callback visits bare [`ExprKind::Name`] and qualified
    /// [`ExprKind::Path`] occurrences as one [`NamePath`] abstraction. Returning
    /// `None` retains the occurrence; returning a path replaces it. Expression
    /// topology and every expression/source range are preserved. Call callees
    /// are structural [`NamePath`] values and are visited before their ordered
    /// arguments.
    #[must_use]
    pub fn rewrite_name_paths(
        &self,
        mut rewrite: impl FnMut(&NamePath) -> Option<NamePath>,
    ) -> Self {
        self.rewrite_name_paths_with(&mut rewrite)
    }

    fn rewrite_name_paths_with(
        &self,
        rewrite: &mut impl FnMut(&NamePath) -> Option<NamePath>,
    ) -> Self {
        let kind = match &self.kind {
            ExprKind::Number(value) => ExprKind::Number(*value),
            ExprKind::Name(name) => {
                let path = NamePath::single(name.clone(), self.range);
                rewrite(&path).map_or_else(
                    || ExprKind::Name(name.clone()),
                    |replacement| expression_name(replacement.with_range(self.range)),
                )
            }
            ExprKind::Path(path) => rewrite(path).map_or_else(
                || ExprKind::Path(path.clone()),
                |replacement| expression_name(replacement.with_range(self.range)),
            ),
            ExprKind::BoundaryPortSelection { port, selector } => ExprKind::BoundaryPortSelection {
                port: Box::new(rewrite(port).unwrap_or_else(|| port.as_ref().clone())),
                selector: selector.clone(),
            },
            ExprKind::Unary { op, value } => ExprKind::Unary {
                op: *op,
                value: Box::new(value.rewrite_name_paths_with(rewrite)),
            },
            ExprKind::Binary { op, left, right } => ExprKind::Binary {
                op: *op,
                left: Box::new(left.rewrite_name_paths_with(rewrite)),
                right: Box::new(right.rewrite_name_paths_with(rewrite)),
            },
            ExprKind::Call { callee, arguments } => ExprKind::Call {
                callee: rewrite(callee).map_or_else(
                    || callee.clone(),
                    |replacement| replacement.with_range(callee.range()),
                ),
                arguments: arguments
                    .iter()
                    .map(|argument| argument.rewrite_name_paths_with(rewrite))
                    .collect(),
            },
        };
        Self {
            kind,
            range: self.range,
        }
    }
}

fn expression_name(path: NamePath) -> ExprKind {
    if path.is_qualified() {
        ExprKind::Path(path)
    } else {
        ExprKind::Name(path.as_str().to_owned())
    }
}

/// Recursive parser AST. Canonical residual storage is the lowered DAG.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum ExprKind {
    /// Floating-point literal.
    Number(f64),
    /// Source identifier.
    Name(String),
    /// Qualified lexical or instance-member name.
    Path(NamePath),
    /// One boundary-family Port selected by an exact boundary spelling.
    BoundaryPortSelection {
        /// Port family path.
        port: Box<NamePath>,
        /// Closed boundary selector.
        selector: Box<BoundaryPortSelectorSyntax>,
    },
    /// Prefix operator.
    Unary {
        /// Operator.
        op: UnaryOp,
        /// Operand.
        value: Box<Expr>,
    },
    /// Infix operator.
    Binary {
        /// Operator.
        op: BinaryOp,
        /// Left operand.
        left: Box<Expr>,
        /// Right operand.
        right: Box<Expr>,
    },
    /// Qualified named operator with one or more ordered arguments.
    Call {
        /// Structurally qualified operator name.
        callee: NamePath,
        /// Nonempty ordered arguments.
        arguments: Vec<Expr>,
    },
}

/// Prefix expression operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnaryOp {
    /// Arithmetic negation.
    Neg,
}

/// Infix expression operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BinaryOp {
    /// Addition.
    Add,
    /// Subtraction.
    Sub,
    /// Multiplication.
    Mul,
    /// Division.
    Div,
    /// Integer power (validated during lowering).
    Pow,
}
