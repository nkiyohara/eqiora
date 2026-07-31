//! Draft-local spatial identities for client-neutral native construction.

use crate::ast::{BoundarySideSyntax, DomainSyntax, TextRange};
use crate::cartesian::CartesianCoordinateSyntax;
use crate::draft::DraftSymbol;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

fn fixed_cartesian_syntax(bounds: &[(f64, f64)]) -> DomainSyntax {
    let range = TextRange::new(0, 0);
    DomainSyntax::CartesianBox(
        bounds
            .iter()
            .map(|&(lower, upper)| {
                (
                    CartesianCoordinateSyntax::fixed(lower, range),
                    CartesianCoordinateSyntax::fixed(upper, range),
                )
            })
            .collect(),
    )
}

/// Lower or upper oriented side of a Cartesian Domain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DraftBoundarySide {
    /// Lower coordinate side.
    Lower,
    /// Upper coordinate side.
    Upper,
}

impl From<DraftBoundarySide> for BoundarySideSyntax {
    fn from(value: DraftBoundarySide) -> Self {
        match value {
            DraftBoundarySide::Lower => Self::Lower,
            DraftBoundarySide::Upper => Self::Upper,
        }
    }
}

/// Immutable draft-local Cartesian volume or oriented boundary Domain.
///
/// Domain references retain this handle's opaque identity until the closed
/// draft is validated. Equal names or equal geometry never substitute for the
/// exact declaration handle.
#[derive(Debug, Clone)]
pub struct DraftSpatialDomain {
    inner: Arc<DraftSpatialDomainData>,
}

#[derive(Debug)]
struct DraftSpatialDomainData {
    symbol: DraftSymbol,
    name: String,
    kind: DraftSpatialDomainKind,
}

#[derive(Debug, Clone)]
pub(crate) enum DraftSpatialDomainKind {
    CartesianBox {
        bounds: Vec<(f64, f64)>,
    },
    Boundary {
        parent: DraftSpatialDomain,
        axis: usize,
        side: DraftBoundarySide,
    },
}

impl DraftSpatialDomain {
    pub(crate) fn syntax(&self) -> DomainSyntax {
        match self.kind() {
            DraftSpatialDomainKind::CartesianBox { bounds } => fixed_cartesian_syntax(bounds),
            DraftSpatialDomainKind::Boundary { parent, axis, side } => DomainSyntax::Boundary {
                parent: parent.name().to_owned(),
                axis: *axis,
                side: (*side).into(),
            },
        }
    }

    /// Declare one Cartesian box with one lower/upper pair per axis.
    ///
    /// Geometric validity is checked by the shared compiler lowerer, exactly
    /// as it is for parsed source.
    #[must_use]
    pub fn cartesian_box(
        name: impl Into<String>,
        bounds: impl IntoIterator<Item = (f64, f64)>,
    ) -> Self {
        Self {
            inner: Arc::new(DraftSpatialDomainData {
                symbol: DraftSymbol::new(),
                name: name.into(),
                kind: DraftSpatialDomainKind::CartesianBox {
                    bounds: bounds.into_iter().collect(),
                },
            }),
        }
    }

    /// Declare one oriented side of the exact draft-local parent Domain.
    ///
    /// Parent kind and axis validity remain shared compiler checks.
    #[must_use]
    pub fn boundary(
        name: impl Into<String>,
        parent: &Self,
        axis: usize,
        side: DraftBoundarySide,
    ) -> Self {
        Self {
            inner: Arc::new(DraftSpatialDomainData {
                symbol: DraftSymbol::new(),
                name: name.into(),
                kind: DraftSpatialDomainKind::Boundary {
                    parent: parent.clone(),
                    axis,
                    side,
                },
            }),
        }
    }

    /// Declaration name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.inner.name
    }

    /// Cartesian bounds when this is a volume Domain.
    #[must_use]
    pub fn bounds(&self) -> Option<&[(f64, f64)]> {
        match &self.inner.kind {
            DraftSpatialDomainKind::CartesianBox { bounds } => Some(bounds),
            DraftSpatialDomainKind::Boundary { .. } => None,
        }
    }

    /// Exact draft-local parent when this is a boundary Domain.
    #[must_use]
    pub fn parent(&self) -> Option<&Self> {
        match &self.inner.kind {
            DraftSpatialDomainKind::CartesianBox { .. } => None,
            DraftSpatialDomainKind::Boundary { parent, .. } => Some(parent),
        }
    }

    /// Zero-based parent coordinate axis when this is a boundary Domain.
    #[must_use]
    pub fn boundary_axis(&self) -> Option<usize> {
        match &self.inner.kind {
            DraftSpatialDomainKind::CartesianBox { .. } => None,
            DraftSpatialDomainKind::Boundary { axis, .. } => Some(*axis),
        }
    }

    /// Oriented parent side when this is a boundary Domain.
    #[must_use]
    pub fn boundary_side(&self) -> Option<DraftBoundarySide> {
        match &self.inner.kind {
            DraftSpatialDomainKind::CartesianBox { .. } => None,
            DraftSpatialDomainKind::Boundary { side, .. } => Some(*side),
        }
    }

    pub(crate) fn symbol(&self) -> &DraftSymbol {
        &self.inner.symbol
    }

    pub(crate) fn kind(&self) -> &DraftSpatialDomainKind {
        &self.inner.kind
    }
}

impl PartialEq for DraftSpatialDomain {
    fn eq(&self, other: &Self) -> bool {
        self.inner.symbol == other.inner.symbol
    }
}

impl Eq for DraftSpatialDomain {}

impl Hash for DraftSpatialDomain {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.inner.symbol.hash(state);
    }
}

/// Immutable continuous pre-discretization Representation declaration.
#[derive(Debug, Clone)]
pub struct DraftRepresentation {
    pub(crate) symbol: DraftSymbol,
    pub(crate) name: String,
}

impl PartialEq for DraftRepresentation {
    fn eq(&self, other: &Self) -> bool {
        self.symbol == other.symbol
    }
}

impl Eq for DraftRepresentation {}

impl Hash for DraftRepresentation {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.symbol.hash(state);
    }
}

impl DraftRepresentation {
    /// Declare one continuum Representation.
    #[must_use]
    pub fn continuum(name: impl Into<String>) -> Self {
        Self {
            symbol: DraftSymbol::new(),
            name: name.into(),
        }
    }

    /// Declaration name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }
}
