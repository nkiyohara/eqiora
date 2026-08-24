//! Client-neutral inputs for one ephemeral external-spatial occurrence.

use eqiora_schema::kernel::GeometryDigest;

/// One exact external Geometry support supplied to a Component occurrence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExternalGeometrySupportBinding {
    /// A full-dimensional named region.
    Region {
        /// Public Component support slot.
        slot: String,
        /// Exact Geometry artifact identity.
        geometry: GeometryDigest,
        /// Named full-dimensional entity set in that Geometry.
        entity_set: String,
        /// Exact ambient dimension.
        ambient_dimension: usize,
    },
    /// A named boundary of another bound region support.
    Boundary {
        /// Public Component support slot.
        slot: String,
        /// Exact Geometry artifact identity.
        geometry: GeometryDigest,
        /// Named boundary entity set in that Geometry.
        entity_set: String,
        /// Public Component slot naming the exact parent region.
        parent_slot: String,
    },
}

impl ExternalGeometrySupportBinding {
    /// Construct one full-dimensional region binding.
    #[must_use]
    pub fn region(
        slot: impl Into<String>,
        geometry: GeometryDigest,
        entity_set: impl Into<String>,
        ambient_dimension: usize,
    ) -> Self {
        Self::Region {
            slot: slot.into(),
            geometry,
            entity_set: entity_set.into(),
            ambient_dimension,
        }
    }

    /// Construct one boundary binding with an explicit parent support slot.
    #[must_use]
    pub fn boundary(
        slot: impl Into<String>,
        geometry: GeometryDigest,
        entity_set: impl Into<String>,
        parent_slot: impl Into<String>,
    ) -> Self {
        Self::Boundary {
            slot: slot.into(),
            geometry,
            entity_set: entity_set.into(),
            parent_slot: parent_slot.into(),
        }
    }

    /// Bound Component support slot.
    #[must_use]
    pub fn slot(&self) -> &str {
        match self {
            Self::Region { slot, .. } | Self::Boundary { slot, .. } => slot,
        }
    }

    /// Exact Geometry identity.
    #[must_use]
    pub const fn geometry(&self) -> GeometryDigest {
        match self {
            Self::Region { geometry, .. } | Self::Boundary { geometry, .. } => *geometry,
        }
    }

    /// Exact named Geometry entity set.
    #[must_use]
    pub fn entity_set(&self) -> &str {
        match self {
            Self::Region { entity_set, .. } | Self::Boundary { entity_set, .. } => entity_set,
        }
    }
}

/// One explicit coherent-SI value for a public Component Parameter.
#[derive(Clone, Debug, PartialEq)]
pub struct ExternalParameterBinding {
    parameter: String,
    value: f64,
}

impl ExternalParameterBinding {
    /// Construct a named scalar value. Declaration-owned SI dimensions are
    /// applied by the ordinary Component binding checker.
    #[must_use]
    pub fn new(parameter: impl Into<String>, value: f64) -> Self {
        Self {
            parameter: parameter.into(),
            value,
        }
    }

    /// Public Component Parameter name.
    #[must_use]
    pub fn parameter(&self) -> &str {
        &self.parameter
    }

    /// Explicit coherent-SI scalar value.
    #[must_use]
    pub const fn value(&self) -> f64 {
        self.value
    }
}

/// Closed input selecting one local Component occurrence to materialize.
#[derive(Clone, Debug, PartialEq)]
pub struct ExternalComponentBinding {
    model: String,
    component: String,
    supports: Vec<ExternalGeometrySupportBinding>,
    parameters: Vec<ExternalParameterBinding>,
}

impl ExternalComponentBinding {
    /// Construct one bounded local Component occurrence.
    #[must_use]
    pub fn new(
        model: impl Into<String>,
        component: impl Into<String>,
        supports: Vec<ExternalGeometrySupportBinding>,
        parameters: Vec<ExternalParameterBinding>,
    ) -> Self {
        Self {
            model: model.into(),
            component: component.into(),
            supports,
            parameters,
        }
    }

    pub(crate) fn model(&self) -> &str {
        &self.model
    }

    pub(crate) fn component(&self) -> &str {
        &self.component
    }

    /// Exact external Geometry support bindings.
    #[must_use]
    pub fn supports(&self) -> &[ExternalGeometrySupportBinding] {
        &self.supports
    }

    /// Explicit public Parameter bindings.
    #[must_use]
    pub fn parameters(&self) -> &[ExternalParameterBinding] {
        &self.parameters
    }
}
