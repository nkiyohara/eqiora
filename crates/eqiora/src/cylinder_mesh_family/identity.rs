//! Exact in-memory identities for the private cylinder mesh family.

use std::ops::{Index, IndexMut};

use eqiora_core::Diagnostic;

use super::invalid;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum CylinderBenchmark {
    S1,
    S2,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ProviderFamilyRole {
    Primary,
    Bias,
}

impl ProviderFamilyRole {
    pub(super) fn parse(value: &str) -> Result<Self, Diagnostic> {
        match value {
            "primary" => Ok(Self::Primary),
            "bias" => Ok(Self::Bias),
            _ => Err(invalid("unknown cylinder mesh provider-family role")),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ProviderFamilyIdentity {
    pub(super) family_role: ProviderFamilyRole,
    pub(super) generator_name: String,
    pub(super) generator_exact_version: String,
    pub(super) generator_executable_sha256: [u8; 32],
    pub(super) recipe_template_sha256: [u8; 32],
}

impl ProviderFamilyIdentity {
    pub(super) fn new(
        family_role: ProviderFamilyRole,
        generator_name: &str,
        generator_exact_version: &str,
        generator_executable_sha256: &[u8],
        recipe_template_sha256: &[u8],
    ) -> Result<Self, Diagnostic> {
        let executable = generator_executable_sha256
            .try_into()
            .map_err(|_| invalid("generator executable identity must contain exactly 32 bytes"))?;
        let recipe = recipe_template_sha256
            .try_into()
            .map_err(|_| invalid("recipe template identity must contain exactly 32 bytes"))?;
        let identity = Self {
            family_role,
            generator_name: generator_name.to_owned(),
            generator_exact_version: generator_exact_version.to_owned(),
            generator_executable_sha256: executable,
            recipe_template_sha256: recipe,
        };
        identity.revalidate()?;
        Ok(identity)
    }

    pub(super) fn revalidate(&self) -> Result<(), Diagnostic> {
        if self.generator_name.is_empty()
            || self.generator_name.len() > 64
            || self.generator_exact_version.is_empty()
            || self.generator_exact_version.len() > 128
        {
            return Err(invalid(
                "generator name or exact version is empty or exceeds its byte bound",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct ProbeIdentity {
    pub(super) label: String,
    pub(super) source_boundary: String,
    pub(super) coordinate: [f64; 2],
    pub(super) eta_s: [f64; 2],
}

impl ProbeIdentity {
    pub(super) fn new(
        label: &str,
        source_boundary: &str,
        coordinate: [f64; 2],
        eta_s: [f64; 2],
    ) -> Self {
        Self {
            label: label.to_owned(),
            source_boundary: source_boundary.to_owned(),
            coordinate: coordinate.map(normalize_zero),
            eta_s: eta_s.map(normalize_zero),
        }
    }

    fn normalized_bits(&self) -> ([u64; 2], [u64; 2]) {
        (
            self.coordinate.map(normalized_bits),
            self.eta_s.map(normalized_bits),
        )
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct ProbeInventoryIdentity([ProbeIdentity; 2]);

impl ProbeInventoryIdentity {
    pub(super) fn new(probes: [ProbeIdentity; 2]) -> Result<Self, Diagnostic> {
        let inventory = Self(probes);
        inventory.revalidate()?;
        Ok(inventory)
    }

    pub(super) fn revalidate(&self) -> Result<(), Diagnostic> {
        if self.0.iter().any(|probe| {
            probe.label.is_empty()
                || probe.source_boundary.is_empty()
                || probe
                    .coordinate
                    .iter()
                    .chain(probe.eta_s.iter())
                    .any(|value| !value.is_finite())
        }) {
            return Err(invalid("probe inventory contains invalid content"));
        }
        Ok(())
    }

    pub(super) fn reverse(&mut self) {
        self.0.reverse();
    }

    pub(super) fn exact_dfg() -> Self {
        Self([
            ProbeIdentity::new("front", "cylinder", [0.15, 0.2], [-1.0, 0.0]),
            ProbeIdentity::new("rear", "cylinder", [0.25, 0.2], [1.0, 0.0]),
        ])
    }

    pub(super) fn is_exact_dfg(&self) -> bool {
        self.0
            .iter()
            .zip(Self::exact_dfg().0)
            .all(|(actual, expected)| {
                actual.label == expected.label
                    && actual.source_boundary == expected.source_boundary
                    && actual.normalized_bits() == expected.normalized_bits()
            })
    }
}

impl Index<usize> for ProbeInventoryIdentity {
    type Output = ProbeIdentity;

    fn index(&self, index: usize) -> &Self::Output {
        &self.0[index]
    }
}

impl IndexMut<usize> for ProbeInventoryIdentity {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.0[index]
    }
}

#[derive(Clone, Debug)]
pub(super) struct TimeMemberInput {
    pub(super) ordinal: usize,
    pub(super) method: Vec<u8>,
    pub(super) step: f64,
}

#[derive(Clone, Debug)]
pub(super) struct TimeFamilyInput {
    pub(super) members: Vec<TimeMemberInput>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct TimeMemberIdentity {
    pub(super) ordinal: usize,
    pub(super) method: [u8; 32],
    pub(super) step_bits: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct TimeFamilyIdentity {
    pub(super) method: [u8; 32],
    pub(super) members: Vec<TimeMemberIdentity>,
}

#[derive(Clone, Debug)]
pub(super) struct SpaceTimeCellInput {
    pub(super) spatial_ordinal: usize,
    pub(super) time_ordinal: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct SpaceTimeCellIdentity {
    pub(super) spatial: SpatialMemberIdentity,
    pub(super) time: TimeMemberIdentity,
}

impl SpaceTimeCellIdentity {
    pub(super) const fn spatial_ordinal(&self) -> usize {
        self.spatial.ordinal
    }

    pub(super) const fn time_ordinal(&self) -> usize {
        self.time.ordinal
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct SpatialMemberIdentity {
    pub(super) provider: ProviderFamilyIdentity,
    pub(super) ordinal: usize,
    pub(super) source_sha256: [u8; 32],
    pub(super) realized_geometry_sha256: [u8; 32],
    pub(super) mesh_sha256: [u8; 32],
    pub(super) correspondence_sha256: [u8; 32],
    pub(super) realization_binding_sha256: [u8; 32],
    pub(super) requested_boundary_error_bits: u64,
    pub(super) accepted_boundary_error_bits: u64,
    pub(super) circle_segments: usize,
    pub(super) max_cylinder_chord_bits: u64,
    pub(super) max_triangle_diameter_bits: u64,
    pub(super) provider_seed: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct SpatialRefinementIdentity {
    pub(super) source_sha256: [u8; 32],
    pub(super) coarse: SpatialMemberIdentity,
    pub(super) fine: SpatialMemberIdentity,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct SpatialFamilyIdentity {
    pub(super) source_sha256: [u8; 32],
    pub(super) primary_provider: ProviderFamilyIdentity,
    pub(super) bias_provider: ProviderFamilyIdentity,
    pub(super) probes: ProbeInventoryIdentity,
    pub(super) primary: Vec<SpatialMemberIdentity>,
    pub(super) refinements: Vec<SpatialRefinementIdentity>,
    pub(super) bias: SpatialMemberIdentity,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct CanonicalTopology {
    pub(super) coordinate_bits: Vec<[u64; 2]>,
    pub(super) triangles: Vec<[usize; 3]>,
}

pub(super) fn normalize_zero(value: f64) -> f64 {
    if value == 0.0 { 0.0 } else { value }
}

pub(super) fn normalized_bits(value: f64) -> u64 {
    normalize_zero(value).to_bits()
}
