//! Accessible projections of admitted mathematical structure.

use eqiora_compiler::{QuantityIdentity, QuantityRole};
use eqiora_core::RawId;
use eqiora_lang::NotationProfile;
use eqiora_schema::kernel::pure_operator::OperatorDefinitionDigest;

mod expression;
mod output;
mod value_type;

/// Exact targets of one rendered symbol or operator occurrence.
///
/// Multiple declarations may share one physical quantity. In that case every
/// matching declaration is retained; rendering does not choose an authored alias.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MathReference {
    graph_id: Option<RawId>,
    role: Option<QuantityRole>,
    declarations: Vec<QuantityIdentity>,
    operator: Option<OperatorDefinitionDigest>,
}

impl MathReference {
    /// Retained semantic entity, independent of its displayed label.
    #[must_use]
    pub const fn graph_id(&self) -> Option<RawId> {
        self.graph_id
    }

    /// Quantity role, absent for an operator or geometric support.
    #[must_use]
    pub const fn role(&self) -> Option<QuantityRole> {
        self.role
    }

    /// All exact declarations associated with this semantic quantity.
    #[must_use]
    pub fn declarations(&self) -> &[QuantityIdentity] {
        &self.declarations
    }

    /// Content identity of a named pure operator, never inferred from its name.
    #[must_use]
    pub const fn operator(&self) -> Option<OperatorDefinitionDigest> {
        self.operator
    }
}

/// One immutable mathematical projection with independent accessible fallbacks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MathRendering {
    profile: NotationProfile,
    text: String,
    plain: String,
    speech: String,
    references: Vec<MathReference>,
    used_fallback: bool,
}

impl MathRendering {
    /// Selected presentation profile.
    #[must_use]
    pub const fn profile(&self) -> NotationProfile {
        self.profile
    }
    /// Safe presentation text; successful MathML is a complete math element.
    /// When `used_fallback()` is true this is plain text, not rich markup.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }
    /// Bounded plain presentation, available without a rich renderer.
    #[must_use]
    pub fn plain(&self) -> &str {
        &self.plain
    }
    /// Bounded spoken presentation preserving expression grouping.
    #[must_use]
    pub fn speech(&self) -> &str {
        &self.speech
    }
    /// Exact targets in first structural appearance order.
    #[must_use]
    pub fn references(&self) -> &[MathReference] {
        &self.references
    }
    /// Whether a rich output size limit selected the retained plain text.
    #[must_use]
    pub const fn used_fallback(&self) -> bool {
        self.used_fallback
    }
}

// This tree is presentation-only: no parser, evaluator, identifier rewrite, or
// serialization authority. Every operation is selected by an admitted node.
#[derive(Debug, Clone)]
enum Math {
    Number(String),
    Label(eqiora_lang::NotationLabel),
    Function(String, Vec<Self>),
    Gradient(Box<Self>),
    Derivative(Box<Self>),
    Negative(Box<Self>),
    Inner(Box<Self>, Box<Self>),
    Binary(&'static str, Box<Self>, Box<Self>),
    Power(Box<Self>, i32),
    Index(Box<Self>, u32),
    Integral(Box<Self>, Box<Self>),
    Array(Vec<Self>),
    Type(eqiora_core::ValueType),
    Typed(Box<Self>, eqiora_core::ValueType),
}

const MAX_NODES: usize = 4096;
const MAX_DEPTH: usize = 96;
const MAX_OUTPUT: usize = 65_536;
