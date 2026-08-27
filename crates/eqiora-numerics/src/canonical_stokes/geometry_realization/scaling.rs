//! Private automatic scaling owned by the authenticated exact-cylinder binding.

#![allow(dead_code)]

use eqiora_artifact::{ArtifactDigest, ModelEnvelope};
use eqiora_core::{Diagnostic, DimExponents, DynQuantity};
use eqiora_graph::{GraphStore, InMemoryGraphStore};
use eqiora_sem::KernelProgram;

use super::{SteadyStokesGeometryBinding2d, invalid};
use crate::canonical_stokes::api::StokesBoundaryKey2d;
use crate::canonical_stokes::realization::IncompressibleFlowScaleProfile2d;

const LENGTH: DimExponents = DimExponents {
    length: 1,
    ..DimExponents::DIMENSIONLESS
};
const VELOCITY: DimExponents = DimExponents {
    length: 1,
    time: -1,
    ..DimExponents::DIMENSIONLESS
};
const PRESSURE: DimExponents = DimExponents {
    mass: 1,
    length: -1,
    time: -2,
    ..DimExponents::DIMENSIONLESS
};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct IncompressibleScalingRequest2d {
    length: Option<DynQuantity>,
    velocity: Option<DynQuantity>,
    pressure: Option<DynQuantity>,
}

impl IncompressibleScalingRequest2d {
    pub(super) fn new(
        length: Option<DynQuantity>,
        velocity: Option<DynQuantity>,
        pressure: Option<DynQuantity>,
    ) -> Result<Self, Diagnostic> {
        validate_component(length, LENGTH, "incompressible length scale L")?;
        validate_component(velocity, VELOCITY, "incompressible velocity scale U")?;
        validate_component(pressure, PRESSURE, "incompressible pressure scale P")?;
        Ok(Self {
            length,
            velocity,
            pressure,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ScalingComponent2d {
    Length,
    Velocity,
    Pressure,
    Gauge,
    WeakFunctional,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ScalingMode2d {
    Manual,
    Automatic,
    Derived,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ScalingRule2d {
    ManualOverrideV1,
    ExactChannelHeightV1,
    ExactInletMaximumV1,
    ViscousStokesPressureV1,
    GaugeRateV1,
    WeakFunctionalV1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ScalingDependencies2d {
    None,
    Two([ScalingComponent2d; 2]),
    Three([ScalingComponent2d; 3]),
}

impl ScalingDependencies2d {
    pub(super) const fn as_slice(&self) -> &[ScalingComponent2d] {
        match self {
            Self::None => &[],
            Self::Two(values) => values,
            Self::Three(values) => values,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum ScalingAuthority2d {
    ManualRequest,
    ExactGeometrySpan {
        axis: usize,
        lower_m: f64,
        upper_m: f64,
    },
    ModelInletMaximum {
        coordinate_m: [f64; 2],
        outward_normal: [f64; 2],
        velocity_m_per_s: [f64; 2],
    },
    ModelDynamicViscosity {
        dynamic_viscosity_pa_s: f64,
    },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum ScalingAuthorities2d {
    None,
    One([ScalingAuthority2d; 1]),
    Two([ScalingAuthority2d; 2]),
}

impl ScalingAuthorities2d {
    pub(super) const fn as_slice(&self) -> &[ScalingAuthority2d] {
        match self {
            Self::None => &[],
            Self::One(values) => values,
            Self::Two(values) => values,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct ScalingComponentRecord2d {
    component: ScalingComponent2d,
    value: DynQuantity,
    mode: ScalingMode2d,
    rule: ScalingRule2d,
    dependencies: ScalingDependencies2d,
    authorities: ScalingAuthorities2d,
}

impl ScalingComponentRecord2d {
    pub(super) const fn component(self) -> ScalingComponent2d {
        self.component
    }

    pub(super) const fn value(self) -> DynQuantity {
        self.value
    }

    pub(super) const fn mode(self) -> ScalingMode2d {
        self.mode
    }

    pub(super) const fn rule(self) -> ScalingRule2d {
        self.rule
    }

    pub(super) const fn dependencies(self) -> ScalingDependencies2d {
        self.dependencies
    }

    pub(super) const fn authorities(self) -> ScalingAuthorities2d {
        self.authorities
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct IncompressibleScalingReceipt2d {
    model: ArtifactDigest,
    geometry: ArtifactDigest,
    correspondence: ArtifactDigest,
    mesh: ArtifactDigest,
    components: [ScalingComponentRecord2d; 5],
}

impl IncompressibleScalingReceipt2d {
    pub(super) const fn model(&self) -> &ArtifactDigest {
        &self.model
    }

    pub(super) const fn geometry(&self) -> &ArtifactDigest {
        &self.geometry
    }

    pub(super) const fn correspondence(&self) -> &ArtifactDigest {
        &self.correspondence
    }

    pub(super) const fn mesh(&self) -> &ArtifactDigest {
        &self.mesh
    }

    pub(super) const fn components(&self) -> &[ScalingComponentRecord2d; 5] {
        &self.components
    }

    pub(super) const fn component(
        &self,
        component: ScalingComponent2d,
    ) -> ScalingComponentRecord2d {
        self.components[component as usize]
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ResolvedIncompressibleScaling2d {
    scales: IncompressibleFlowScaleProfile2d,
    receipt: IncompressibleScalingReceipt2d,
}

impl ResolvedIncompressibleScaling2d {
    pub(crate) const fn scales(&self) -> IncompressibleFlowScaleProfile2d {
        self.scales
    }

    pub(super) const fn receipt(&self) -> &IncompressibleScalingReceipt2d {
        &self.receipt
    }
}

impl SteadyStokesGeometryBinding2d {
    /// Resolve the closed exact-cylinder scaling family after this binding has
    /// authenticated Model meaning and the Geometry/correspondence/Mesh chain.
    pub(crate) fn resolve_incompressible_scaling(
        &self,
        model: &ModelEnvelope,
        request: Option<IncompressibleScalingRequest2d>,
    ) -> Result<ResolvedIncompressibleScaling2d, Diagnostic> {
        let replayed = replay_model(model, &self.source)?;
        if replayed != self.program {
            return Err(invalid(
                "automatic scaling Model meaning differs from the authenticated Stokes binding",
            ));
        }
        let request = request.unwrap_or_default();

        let (length, length_mode, length_rule, length_authorities) = match request.length {
            Some(value) => manual(value),
            None => {
                let bounds = self.exact_bounds()?;
                let value = positive(bounds[1][1] - bounds[1][0], "exact channel height")?;
                (
                    DynQuantity::new(value, LENGTH),
                    ScalingMode2d::Automatic,
                    ScalingRule2d::ExactChannelHeightV1,
                    ScalingAuthorities2d::One([ScalingAuthority2d::ExactGeometrySpan {
                        axis: 1,
                        lower_m: bounds[1][0],
                        upper_m: bounds[1][1],
                    }]),
                )
            }
        };

        let (velocity, velocity_mode, velocity_rule, velocity_authorities) = match request.velocity
        {
            Some(value) => manual(value),
            None => {
                let bounds = self.exact_bounds()?;
                let height = positive(bounds[1][1] - bounds[1][0], "exact channel height")?;
                let outward_normal = self
                    .source
                    .constant_parent_outward_normal("inlet")
                    .ok_or_else(|| {
                        invalid("automatic U requires one exact fixed-side `inlet` normal")
                    })?;
                let coordinate_m = [bounds[0][0], bounds[1][0] + 0.5 * height];
                let inlet_velocity = self
                    .model
                    .prescribed_velocity(
                        &StokesBoundaryKey2d::NamedEntitySet("inlet".to_owned()),
                        Some(outward_normal),
                        &coordinate_m,
                    )?
                    .ok_or_else(|| {
                        invalid("automatic U requires the Model-owned `inlet` velocity law")
                    })?;
                let value = positive(inlet_velocity[0], "Model inlet maximum")?;
                if inlet_velocity[1] != 0.0 {
                    return Err(invalid(
                        "automatic U requires the inlet maximum to align with the channel axis",
                    ));
                }
                (
                    DynQuantity::new(value, VELOCITY),
                    ScalingMode2d::Automatic,
                    ScalingRule2d::ExactInletMaximumV1,
                    ScalingAuthorities2d::Two([
                        ScalingAuthority2d::ExactGeometrySpan {
                            axis: 1,
                            lower_m: bounds[1][0],
                            upper_m: bounds[1][1],
                        },
                        ScalingAuthority2d::ModelInletMaximum {
                            coordinate_m,
                            outward_normal,
                            velocity_m_per_s: inlet_velocity,
                        },
                    ]),
                )
            }
        };

        let (pressure, pressure_mode, pressure_rule, pressure_dependencies, pressure_authorities) =
            match request.pressure {
                Some(value) => {
                    let (value, mode, rule, authorities) = manual(value);
                    (value, mode, rule, ScalingDependencies2d::None, authorities)
                }
                None => {
                    let viscosity =
                        positive(self.model.dynamic_viscosity(), "Model dynamic viscosity")?;
                    (
                        DynQuantity::new(viscosity * velocity.value() / length.value(), PRESSURE),
                        ScalingMode2d::Automatic,
                        ScalingRule2d::ViscousStokesPressureV1,
                        ScalingDependencies2d::Two([
                            ScalingComponent2d::Length,
                            ScalingComponent2d::Velocity,
                        ]),
                        ScalingAuthorities2d::One([ScalingAuthority2d::ModelDynamicViscosity {
                            dynamic_viscosity_pa_s: viscosity,
                        }]),
                    )
                }
            };

        let scales = IncompressibleFlowScaleProfile2d::new(length, velocity, pressure)?;
        let components = [
            record(
                ScalingComponent2d::Length,
                scales.length(),
                length_mode,
                length_rule,
                ScalingDependencies2d::None,
                length_authorities,
            ),
            record(
                ScalingComponent2d::Velocity,
                scales.velocity(),
                velocity_mode,
                velocity_rule,
                ScalingDependencies2d::None,
                velocity_authorities,
            ),
            record(
                ScalingComponent2d::Pressure,
                scales.pressure(),
                pressure_mode,
                pressure_rule,
                pressure_dependencies,
                pressure_authorities,
            ),
            record(
                ScalingComponent2d::Gauge,
                scales.gauge(),
                ScalingMode2d::Derived,
                ScalingRule2d::GaugeRateV1,
                ScalingDependencies2d::Two([
                    ScalingComponent2d::Velocity,
                    ScalingComponent2d::Length,
                ]),
                ScalingAuthorities2d::None,
            ),
            record(
                ScalingComponent2d::WeakFunctional,
                scales.weak_functional(),
                ScalingMode2d::Derived,
                ScalingRule2d::WeakFunctionalV1,
                ScalingDependencies2d::Three([
                    ScalingComponent2d::Pressure,
                    ScalingComponent2d::Velocity,
                    ScalingComponent2d::Length,
                ]),
                ScalingAuthorities2d::None,
            ),
        ];
        Ok(ResolvedIncompressibleScaling2d {
            scales,
            receipt: IncompressibleScalingReceipt2d {
                model: model.digest()?,
                geometry: ArtifactDigest::from_sha256(self.source.digest_bytes()),
                correspondence: self.correspondence.digest()?,
                mesh: self.mesh.digest()?,
                components,
            },
        })
    }

    fn exact_bounds(&self) -> Result<&[[f64; 2]; 2], Diagnostic> {
        let source_bounds = self
            .source
            .circular_hole_bounds()
            .ok_or_else(|| invalid("automatic scaling requires exact circular-hole bounds"))?;
        if self.model.bounds() != source_bounds {
            return Err(invalid(
                "automatic scaling Model bounds differ from authenticated exact Geometry",
            ));
        }
        Ok(source_bounds)
    }
}

fn replay_model(
    model: &ModelEnvelope,
    source: &eqiora_geometry::CanonicalGeometryV1,
) -> Result<KernelProgram, Diagnostic> {
    let (transaction, model_id) = model.to_transaction().map_err(first_diagnostic)?;
    let mut store = InMemoryGraphStore::new();
    store.commit(transaction).map_err(first_diagnostic)?;
    KernelProgram::from_snapshot_with_geometry(&store.snapshot(), model_id, &[source.into()])
        .map_err(first_diagnostic)
}

fn first_diagnostic(diagnostics: Vec<Diagnostic>) -> Diagnostic {
    diagnostics
        .into_iter()
        .next()
        .unwrap_or_else(|| invalid("automatic scaling Model replay failed without a diagnostic"))
}

fn manual(
    value: DynQuantity,
) -> (
    DynQuantity,
    ScalingMode2d,
    ScalingRule2d,
    ScalingAuthorities2d,
) {
    (
        value,
        ScalingMode2d::Manual,
        ScalingRule2d::ManualOverrideV1,
        ScalingAuthorities2d::One([ScalingAuthority2d::ManualRequest]),
    )
}

fn positive(value: f64, label: &str) -> Result<f64, Diagnostic> {
    if value.is_finite() && value > 0.0 {
        Ok(value)
    } else {
        Err(invalid(format!(
            "{label} must be finite and strictly positive"
        )))
    }
}

fn validate_component(
    value: Option<DynQuantity>,
    dimension: DimExponents,
    label: &str,
) -> Result<(), Diagnostic> {
    let Some(value) = value else {
        return Ok(());
    };
    if value.dim() != dimension {
        return Err(invalid(format!(
            "{label} has incompatible physical dimension"
        )));
    }
    positive(value.value(), label).map(|_| ())
}

const fn record(
    component: ScalingComponent2d,
    value: DynQuantity,
    mode: ScalingMode2d,
    rule: ScalingRule2d,
    dependencies: ScalingDependencies2d,
    authorities: ScalingAuthorities2d,
) -> ScalingComponentRecord2d {
    ScalingComponentRecord2d {
        component,
        value,
        mode,
        rule,
        dependencies,
        authorities,
    }
}

#[cfg(test)]
mod tests;
