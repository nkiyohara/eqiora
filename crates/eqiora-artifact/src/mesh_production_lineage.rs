//! Provider-owned production lineage for one accepted common Mesh.

use eqiora_core::Diagnostic;
use eqiora_geometry::CanonicalGeometryV1;
use eqiora_meshing::MeshQualityGate;
use serde::{Deserialize, Serialize};

use crate::{
    ArtifactDigest, CANONICAL_ENCODING, CartesianMeshEnvelopeV1,
    GeometryMeshCorrespondenceEnvelopeV1, JsonDecoderLimits, SimplicialMeshEnvelopeV1,
    check_json_limits, invalid_artifact,
};

const SCHEMA: &str = "eqiora.mesh-production-lineage-envelope/v1";
const GMSH_IDENTITY: &str = "eqiora.gmsh-cli";
const GMSH_VERSION: &str = "4.15.2";
const REFERENCE_IDENTITY: &str = "eqiora.reference-planar-circular-hole";
const REFERENCE_VERSION: &str = "1";
const CARTESIAN_IDENTITY: &str = "eqiora.structured-cartesian";
const CARTESIAN_VERSION: &str = "1";
const AFFINE_TRIANGLE_IDENTITY: &str = "eqiora.affine-triangle-rectangle";
const AFFINE_TRIANGLE_VERSION: &str = "1";
const AFFINE_TRIANGLE_DIAGONAL: &str = "lower-left-to-upper-right";

/// Closed identity of a provider that currently produces a common Mesh.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MeshProductionProvider {
    /// Exact external Gmsh CLI 4.15.2 adapter.
    Gmsh4152,
    /// Deterministic in-process planar circular-hole reference producer v1.
    PlanarCircularHoleReferenceV1,
    /// Deterministic structured Cartesian producer v1.
    StructuredCartesianV1,
    /// Deterministic rectangle affine-triangle producer v1.
    AffineTriangleRectangleV1,
}

impl MeshProductionProvider {
    /// Stable provider identity independent of effective numerical policy.
    #[must_use]
    pub const fn identity(self) -> &'static str {
        match self {
            Self::Gmsh4152 => GMSH_IDENTITY,
            Self::PlanarCircularHoleReferenceV1 => REFERENCE_IDENTITY,
            Self::StructuredCartesianV1 => CARTESIAN_IDENTITY,
            Self::AffineTriangleRectangleV1 => AFFINE_TRIANGLE_IDENTITY,
        }
    }

    /// Exact provider implementation version.
    #[must_use]
    pub const fn version(self) -> &'static str {
        match self {
            Self::Gmsh4152 => GMSH_VERSION,
            Self::PlanarCircularHoleReferenceV1 => REFERENCE_VERSION,
            Self::StructuredCartesianV1 => CARTESIAN_VERSION,
            Self::AffineTriangleRectangleV1 => AFFINE_TRIANGLE_VERSION,
        }
    }
}

/// Exact effective policy of the structured Cartesian provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CartesianMeshCellsV1([usize; 2]);

impl CartesianMeshCellsV1 {
    /// Construct positive x/y cell counts.
    pub fn new(cells: [usize; 2]) -> Result<Self, Diagnostic> {
        if cells.contains(&0) {
            return Err(invalid_artifact("Cartesian cell counts must be positive"));
        }
        if cells.iter().any(|&count| count.checked_add(1).is_none()) {
            return Err(invalid_artifact(
                "Cartesian cell count overflows its axis vertex count",
            ));
        }
        Ok(Self(cells))
    }

    /// Exact x/y cell counts.
    #[must_use]
    pub const fn cells(self) -> [usize; 2] {
        self.0
    }
}

/// Exact effective policy of the fixed-diagonal affine-triangle provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AffineTriangleMeshCellsV1([usize; 2]);

impl AffineTriangleMeshCellsV1 {
    /// Construct positive x/y subdivision counts whose analytic sizes fit `usize`.
    pub fn new(cells: [usize; 2]) -> Result<Self, Diagnostic> {
        if cells.contains(&0) {
            return Err(invalid_artifact(
                "affine-triangle cell counts must be positive",
            ));
        }
        let [nx, ny] = cells;
        let vertices = nx
            .checked_add(1)
            .and_then(|x| ny.checked_add(1).and_then(|y| x.checked_mul(y)));
        let triangles = nx.checked_mul(ny).and_then(|cells| cells.checked_mul(2));
        let boundary_facets = nx.checked_add(ny).and_then(|sum| sum.checked_mul(2));
        if vertices.is_none() || triangles.is_none() || boundary_facets.is_none() {
            return Err(invalid_artifact(
                "affine-triangle cell counts overflow analytic mesh counts",
            ));
        }
        Ok(Self(cells))
    }

    /// Exact x/y structured-cell counts before fixed diagonal subdivision.
    #[must_use]
    pub const fn cells(self) -> [usize; 2] {
        self.0
    }

    /// Provider-owned diagonal convention; callers cannot select another one.
    #[must_use]
    pub const fn diagonal(self) -> &'static str {
        AFFINE_TRIANGLE_DIAGONAL
    }
}

/// Exact effective numerical policy shared by the two current planar providers.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlanarMeshQualityV1 {
    maximum_boundary_error_m: f64,
    minimum_mean_ratio: f64,
    maximum_boundary_facets: usize,
}

impl PlanarMeshQualityV1 {
    /// Construct one complete effective planar policy.
    ///
    /// # Errors
    /// Returns `EQ0901` for nonfinite/nonpositive error, invalid quality, or
    /// fewer than eight admitted circular-boundary facets.
    pub fn new(
        maximum_boundary_error_m: f64,
        minimum_mean_ratio: f64,
        maximum_boundary_facets: usize,
    ) -> Result<Self, Diagnostic> {
        if !maximum_boundary_error_m.is_finite() || maximum_boundary_error_m <= 0.0 {
            return Err(invalid_artifact(
                "maximum boundary error must be finite and positive",
            ));
        }
        MeshQualityGate::new(minimum_mean_ratio)
            .map_err(|diagnostic| invalid_artifact(diagnostic.message()))?;
        if maximum_boundary_facets < 8 {
            return Err(invalid_artifact(
                "maximum boundary facets must be at least eight",
            ));
        }
        Ok(Self {
            maximum_boundary_error_m,
            minimum_mean_ratio,
            maximum_boundary_facets,
        })
    }

    /// Effective maximum chordal boundary error in metres.
    #[must_use]
    pub const fn maximum_boundary_error_m(self) -> f64 {
        self.maximum_boundary_error_m
    }

    /// Effective affine-simplex quality gate.
    #[must_use]
    pub const fn minimum_mean_ratio(self) -> f64 {
        self.minimum_mean_ratio
    }

    /// Effective circular-boundary work limit.
    #[must_use]
    pub const fn maximum_boundary_facets(self) -> usize {
        self.maximum_boundary_facets
    }
}

/// Canonical provider occurrence that produced one exact accepted common Mesh.
///
/// This artifact is deliberately separate from provider-neutral Geometry,
/// Mesh, and correspondence identity. It binds those identities to the exact
/// provider release and every effective provider policy value.
#[derive(Debug, Clone, PartialEq)]
pub struct MeshProductionLineageEnvelopeV1 {
    wire: WireMeshProductionLineageV1,
}

impl MeshProductionLineageEnvelopeV1 {
    /// Bind one exact Gmsh 4.15.2 occurrence to accepted resources.
    ///
    /// # Errors
    /// Returns `EQ0901` if a resource digest cannot be constructed.
    pub fn from_gmsh_4152_resources(
        policy: PlanarMeshQualityV1,
        geometry: &CanonicalGeometryV1,
        mesh: &SimplicialMeshEnvelopeV1,
        correspondence: &GeometryMeshCorrespondenceEnvelopeV1,
    ) -> Result<Self, Diagnostic> {
        Self::from_resources(
            MeshProductionProvider::Gmsh4152,
            policy,
            geometry,
            mesh,
            correspondence,
        )
    }

    /// Bind one reference-producer v1 occurrence to accepted resources.
    ///
    /// # Errors
    /// Returns `EQ0901` if a resource digest cannot be constructed.
    pub fn from_planar_circular_hole_reference_v1_resources(
        policy: PlanarMeshQualityV1,
        geometry: &CanonicalGeometryV1,
        mesh: &SimplicialMeshEnvelopeV1,
        correspondence: &GeometryMeshCorrespondenceEnvelopeV1,
    ) -> Result<Self, Diagnostic> {
        Self::from_resources(
            MeshProductionProvider::PlanarCircularHoleReferenceV1,
            policy,
            geometry,
            mesh,
            correspondence,
        )
    }

    /// Bind one provider occurrence to accepted Geometry/Mesh resources.
    ///
    /// # Errors
    /// Returns `EQ0901` if a resource digest cannot be constructed.
    fn from_resources(
        provider: MeshProductionProvider,
        policy: PlanarMeshQualityV1,
        geometry: &CanonicalGeometryV1,
        mesh: &SimplicialMeshEnvelopeV1,
        correspondence: &GeometryMeshCorrespondenceEnvelopeV1,
    ) -> Result<Self, Diagnostic> {
        let lineage = Self {
            wire: WireMeshProductionLineageV1 {
                schema: SCHEMA.to_owned(),
                encoding: CANONICAL_ENCODING.to_owned(),
                provider: WireProviderV1 {
                    identity: provider.identity().to_owned(),
                    version: provider.version().to_owned(),
                },
                effective_policy: WireEffectivePolicyV1::PlanarMeshQuality(
                    WirePlanarMeshQualityV1::try_from(policy)?,
                ),
                geometry_sha256: ArtifactDigest::from_sha256(geometry.digest_bytes()).to_string(),
                mesh_sha256: mesh.digest()?.to_string(),
                correspondence_sha256: correspondence.digest()?.to_string(),
            },
        };
        lineage.validate_local()?;
        Ok(lineage)
    }

    /// Bind one structured Cartesian v1 occurrence to exact rectangle resources.
    pub fn from_structured_cartesian_v1_resources(
        policy: CartesianMeshCellsV1,
        geometry: &CanonicalGeometryV1,
        mesh: &CartesianMeshEnvelopeV1,
        correspondence: &GeometryMeshCorrespondenceEnvelopeV1,
    ) -> Result<Self, Diagnostic> {
        let lineage = Self {
            wire: WireMeshProductionLineageV1 {
                schema: SCHEMA.to_owned(),
                encoding: CANONICAL_ENCODING.to_owned(),
                provider: WireProviderV1 {
                    identity: CARTESIAN_IDENTITY.to_owned(),
                    version: CARTESIAN_VERSION.to_owned(),
                },
                effective_policy: WireEffectivePolicyV1::CartesianCells(
                    WireCartesianCellsV1::try_from(policy)?,
                ),
                geometry_sha256: ArtifactDigest::from_sha256(geometry.digest_bytes()).to_string(),
                mesh_sha256: mesh.digest()?.to_string(),
                correspondence_sha256: correspondence.digest()?.to_string(),
            },
        };
        lineage.validate_local()?;
        Ok(lineage)
    }

    /// Bind one fixed-diagonal affine-triangle v1 occurrence to exact rectangle resources.
    pub fn from_affine_triangle_rectangle_v1_resources(
        policy: AffineTriangleMeshCellsV1,
        geometry: &CanonicalGeometryV1,
        mesh: &SimplicialMeshEnvelopeV1,
        correspondence: &GeometryMeshCorrespondenceEnvelopeV1,
    ) -> Result<Self, Diagnostic> {
        let lineage = Self {
            wire: WireMeshProductionLineageV1 {
                schema: SCHEMA.to_owned(),
                encoding: CANONICAL_ENCODING.to_owned(),
                provider: WireProviderV1 {
                    identity: AFFINE_TRIANGLE_IDENTITY.to_owned(),
                    version: AFFINE_TRIANGLE_VERSION.to_owned(),
                },
                effective_policy: WireEffectivePolicyV1::AffineTriangleCells(
                    WireAffineTriangleCellsV1::try_from(policy)?,
                ),
                geometry_sha256: ArtifactDigest::from_sha256(geometry.digest_bytes()).to_string(),
                mesh_sha256: mesh.digest()?.to_string(),
                correspondence_sha256: correspondence.digest()?.to_string(),
            },
        };
        lineage.validate_local()?;
        Ok(lineage)
    }

    /// Decode exact canonical lineage bytes.
    ///
    /// # Errors
    /// Returns `EQ0901` for malformed, unknown, noncanonical, or invalid data.
    pub fn from_json(bytes: &[u8]) -> Result<Self, Diagnostic> {
        check_json_limits(bytes, JsonDecoderLimits::default())?;
        let wire: WireMeshProductionLineageV1 = serde_json::from_slice(bytes).map_err(|error| {
            invalid_artifact(format!("invalid mesh production lineage JSON: {error}"))
        })?;
        let lineage = Self { wire };
        lineage.validate_local()?;
        if lineage.canonical_json()? != bytes {
            return Err(invalid_artifact(
                "mesh production lineage is not canonical JSON",
            ));
        }
        Ok(lineage)
    }

    /// Rebuild and compare the complete production occurrence.
    ///
    /// # Errors
    /// Returns `EQ0901` if provider, policy, or any bound resource differs.
    fn validate_against_resources(
        &self,
        provider: MeshProductionProvider,
        policy: PlanarMeshQualityV1,
        geometry: &CanonicalGeometryV1,
        mesh: &SimplicialMeshEnvelopeV1,
        correspondence: &GeometryMeshCorrespondenceEnvelopeV1,
    ) -> Result<(), Diagnostic> {
        let expected = Self::from_resources(provider, policy, geometry, mesh, correspondence)?;
        if self != &expected {
            return Err(invalid_artifact(
                "mesh production lineage differs from provider occurrence or accepted resources",
            ));
        }
        Ok(())
    }

    /// Rebuild and compare one exact Gmsh 4.15.2 occurrence.
    ///
    /// # Errors
    /// Returns `EQ0901` if policy or any bound resource differs.
    pub fn validate_against_gmsh_4152_resources(
        &self,
        policy: PlanarMeshQualityV1,
        geometry: &CanonicalGeometryV1,
        mesh: &SimplicialMeshEnvelopeV1,
        correspondence: &GeometryMeshCorrespondenceEnvelopeV1,
    ) -> Result<(), Diagnostic> {
        self.validate_against_resources(
            MeshProductionProvider::Gmsh4152,
            policy,
            geometry,
            mesh,
            correspondence,
        )
    }

    /// Rebuild and compare one reference-producer v1 occurrence.
    ///
    /// # Errors
    /// Returns `EQ0901` if policy or any bound resource differs.
    pub fn validate_against_planar_circular_hole_reference_v1_resources(
        &self,
        policy: PlanarMeshQualityV1,
        geometry: &CanonicalGeometryV1,
        mesh: &SimplicialMeshEnvelopeV1,
        correspondence: &GeometryMeshCorrespondenceEnvelopeV1,
    ) -> Result<(), Diagnostic> {
        self.validate_against_resources(
            MeshProductionProvider::PlanarCircularHoleReferenceV1,
            policy,
            geometry,
            mesh,
            correspondence,
        )
    }

    /// Rebuild and compare one exact structured Cartesian v1 occurrence.
    pub fn validate_against_structured_cartesian_v1_resources(
        &self,
        policy: CartesianMeshCellsV1,
        geometry: &CanonicalGeometryV1,
        mesh: &CartesianMeshEnvelopeV1,
        correspondence: &GeometryMeshCorrespondenceEnvelopeV1,
    ) -> Result<(), Diagnostic> {
        let expected =
            Self::from_structured_cartesian_v1_resources(policy, geometry, mesh, correspondence)?;
        if self != &expected {
            return Err(invalid_artifact(
                "mesh production lineage differs from structured Cartesian occurrence or accepted resources",
            ));
        }
        Ok(())
    }

    /// Rebuild and compare one exact fixed-diagonal affine-triangle occurrence.
    pub fn validate_against_affine_triangle_rectangle_v1_resources(
        &self,
        policy: AffineTriangleMeshCellsV1,
        geometry: &CanonicalGeometryV1,
        mesh: &SimplicialMeshEnvelopeV1,
        correspondence: &GeometryMeshCorrespondenceEnvelopeV1,
    ) -> Result<(), Diagnostic> {
        let expected = Self::from_affine_triangle_rectangle_v1_resources(
            policy,
            geometry,
            mesh,
            correspondence,
        )?;
        if self != &expected {
            return Err(invalid_artifact(
                "mesh production lineage differs from affine-triangle rectangle occurrence or accepted resources",
            ));
        }
        Ok(())
    }

    /// Stable provider identity reconstructed from the canonical wire.
    #[must_use]
    pub fn provider_identity(&self) -> &str {
        &self.wire.provider.identity
    }

    /// Exact provider implementation version reconstructed from the wire.
    #[must_use]
    pub fn provider_version(&self) -> &str {
        &self.wire.provider.version
    }

    /// Exact effective numerical policy reconstructed from the canonical wire.
    #[must_use]
    pub fn planar_mesh_quality(&self) -> Option<PlanarMeshQualityV1> {
        match self.wire.effective_policy {
            WireEffectivePolicyV1::PlanarMeshQuality(policy) => policy.to_policy().ok(),
            WireEffectivePolicyV1::CartesianCells(_)
            | WireEffectivePolicyV1::AffineTriangleCells(_) => None,
        }
    }

    /// Exact Cartesian cell policy, when this is a Cartesian occurrence.
    #[must_use]
    pub fn cartesian_cells(&self) -> Option<CartesianMeshCellsV1> {
        match self.wire.effective_policy {
            WireEffectivePolicyV1::CartesianCells(policy) => policy.to_policy().ok(),
            WireEffectivePolicyV1::PlanarMeshQuality(_)
            | WireEffectivePolicyV1::AffineTriangleCells(_) => None,
        }
    }

    /// Exact affine-triangle subdivision policy, when this is that occurrence.
    #[must_use]
    pub fn affine_triangle_cells(&self) -> Option<AffineTriangleMeshCellsV1> {
        match self.wire.effective_policy {
            WireEffectivePolicyV1::AffineTriangleCells(policy) => policy.to_policy().ok(),
            WireEffectivePolicyV1::PlanarMeshQuality(_)
            | WireEffectivePolicyV1::CartesianCells(_) => None,
        }
    }

    /// Deterministic canonical JSON bytes.
    ///
    /// # Errors
    /// Returns `EQ0901` if serialization unexpectedly fails.
    pub fn canonical_json(&self) -> Result<Vec<u8>, Diagnostic> {
        serde_json::to_vec(&self.wire).map_err(|error| {
            invalid_artifact(format!("cannot serialize mesh production lineage: {error}"))
        })
    }

    /// Domain-separated identity of this provider occurrence.
    ///
    /// # Errors
    /// Returns `EQ0901` if canonical serialization unexpectedly fails.
    pub fn digest(&self) -> Result<ArtifactDigest, Diagnostic> {
        Ok(ArtifactDigest::compute(
            SCHEMA.as_bytes(),
            &self.canonical_json()?,
        ))
    }

    fn validate_local(&self) -> Result<(), Diagnostic> {
        if self.wire.schema != SCHEMA || self.wire.encoding != CANONICAL_ENCODING {
            return Err(invalid_artifact(
                "unsupported mesh production lineage schema or encoding",
            ));
        }
        let provider = provider_from_wire(&self.wire.provider)?;
        self.wire.effective_policy.validate()?;
        let compatible = matches!(
            (provider, self.wire.effective_policy),
            (
                MeshProductionProvider::Gmsh4152
                    | MeshProductionProvider::PlanarCircularHoleReferenceV1,
                WireEffectivePolicyV1::PlanarMeshQuality(_)
            ) | (
                MeshProductionProvider::StructuredCartesianV1,
                WireEffectivePolicyV1::CartesianCells(_)
            ) | (
                MeshProductionProvider::AffineTriangleRectangleV1,
                WireEffectivePolicyV1::AffineTriangleCells(_)
            )
        );
        if !compatible {
            return Err(invalid_artifact(
                "mesh production provider and effective policy kinds differ",
            ));
        }
        ArtifactDigest::from_hex(self.wire.geometry_sha256.clone())?;
        ArtifactDigest::from_hex(self.wire.mesh_sha256.clone())?;
        ArtifactDigest::from_hex(self.wire.correspondence_sha256.clone())?;
        Ok(())
    }
}

fn provider_from_wire(wire: &WireProviderV1) -> Result<MeshProductionProvider, Diagnostic> {
    match (wire.identity.as_str(), wire.version.as_str()) {
        (GMSH_IDENTITY, GMSH_VERSION) => Ok(MeshProductionProvider::Gmsh4152),
        (REFERENCE_IDENTITY, REFERENCE_VERSION) => {
            Ok(MeshProductionProvider::PlanarCircularHoleReferenceV1)
        }
        (CARTESIAN_IDENTITY, CARTESIAN_VERSION) => {
            Ok(MeshProductionProvider::StructuredCartesianV1)
        }
        (AFFINE_TRIANGLE_IDENTITY, AFFINE_TRIANGLE_VERSION) => {
            Ok(MeshProductionProvider::AffineTriangleRectangleV1)
        }
        _ => Err(invalid_artifact(
            "mesh production lineage names an unknown provider identity or version",
        )),
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireMeshProductionLineageV1 {
    schema: String,
    encoding: String,
    provider: WireProviderV1,
    effective_policy: WireEffectivePolicyV1,
    geometry_sha256: String,
    mesh_sha256: String,
    correspondence_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireProviderV1 {
    identity: String,
    version: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WirePlanarMeshQualityV1 {
    maximum_boundary_error_m: f64,
    minimum_mean_ratio: f64,
    maximum_boundary_facets: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
enum WireEffectivePolicyV1 {
    PlanarMeshQuality(WirePlanarMeshQualityV1),
    CartesianCells(WireCartesianCellsV1),
    AffineTriangleCells(WireAffineTriangleCellsV1),
}

impl WireEffectivePolicyV1 {
    fn validate(self) -> Result<(), Diagnostic> {
        match self {
            Self::PlanarMeshQuality(policy) => policy.to_policy().map(|_| ()),
            Self::CartesianCells(policy) => policy.to_policy().map(|_| ()),
            Self::AffineTriangleCells(policy) => policy.to_policy().map(|_| ()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireCartesianCellsV1 {
    cells: [u64; 2],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireAffineTriangleCellsV1 {
    cells: [u64; 2],
    diagonal: WireAffineTriangleDiagonalV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum WireAffineTriangleDiagonalV1 {
    LowerLeftToUpperRight,
}

impl TryFrom<AffineTriangleMeshCellsV1> for WireAffineTriangleCellsV1 {
    type Error = Diagnostic;

    fn try_from(policy: AffineTriangleMeshCellsV1) -> Result<Self, Self::Error> {
        Ok(Self {
            cells: [
                u64::try_from(policy.0[0])
                    .map_err(|_| invalid_artifact("x cell count exceeds portable u64"))?,
                u64::try_from(policy.0[1])
                    .map_err(|_| invalid_artifact("y cell count exceeds portable u64"))?,
            ],
            diagonal: WireAffineTriangleDiagonalV1::LowerLeftToUpperRight,
        })
    }
}

impl WireAffineTriangleCellsV1 {
    fn to_policy(self) -> Result<AffineTriangleMeshCellsV1, Diagnostic> {
        AffineTriangleMeshCellsV1::new([
            usize::try_from(self.cells[0])
                .map_err(|_| invalid_artifact("x cell count exceeds local usize"))?,
            usize::try_from(self.cells[1])
                .map_err(|_| invalid_artifact("y cell count exceeds local usize"))?,
        ])
    }
}

impl TryFrom<CartesianMeshCellsV1> for WireCartesianCellsV1 {
    type Error = Diagnostic;

    fn try_from(policy: CartesianMeshCellsV1) -> Result<Self, Self::Error> {
        Ok(Self {
            cells: [
                u64::try_from(policy.0[0])
                    .map_err(|_| invalid_artifact("x cell count exceeds portable u64"))?,
                u64::try_from(policy.0[1])
                    .map_err(|_| invalid_artifact("y cell count exceeds portable u64"))?,
            ],
        })
    }
}

impl WireCartesianCellsV1 {
    fn to_policy(self) -> Result<CartesianMeshCellsV1, Diagnostic> {
        CartesianMeshCellsV1::new([
            usize::try_from(self.cells[0])
                .map_err(|_| invalid_artifact("x cell count exceeds local usize"))?,
            usize::try_from(self.cells[1])
                .map_err(|_| invalid_artifact("y cell count exceeds local usize"))?,
        ])
    }
}

impl TryFrom<PlanarMeshQualityV1> for WirePlanarMeshQualityV1 {
    type Error = Diagnostic;

    fn try_from(policy: PlanarMeshQualityV1) -> Result<Self, Self::Error> {
        Ok(Self {
            maximum_boundary_error_m: policy.maximum_boundary_error_m,
            minimum_mean_ratio: policy.minimum_mean_ratio,
            maximum_boundary_facets: u64::try_from(policy.maximum_boundary_facets)
                .map_err(|_| invalid_artifact("maximum boundary facets exceeds portable u64"))?,
        })
    }
}

impl WirePlanarMeshQualityV1 {
    fn to_policy(self) -> Result<PlanarMeshQualityV1, Diagnostic> {
        PlanarMeshQualityV1::new(
            self.maximum_boundary_error_m,
            self.minimum_mean_ratio,
            usize::try_from(self.maximum_boundary_facets)
                .map_err(|_| invalid_artifact("maximum boundary facets exceeds local usize"))?,
        )
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use eqiora_geometry::{PlanarOperationGraph, PlanarTopologyHandle};

    use super::*;

    fn resources() -> (
        CanonicalGeometryV1,
        SimplicialMeshEnvelopeV1,
        GeometryMeshCorrespondenceEnvelopeV1,
    ) {
        let graph = PlanarOperationGraph::new();
        let rectangle = graph.rectangle([0.0, 2.2], [0.0, 0.41]).unwrap();
        let circle = graph.circle([0.2, 0.2], 0.05).unwrap();
        let fluid = graph.subtract(&rectangle, &circle).unwrap();
        let outer = rectangle.boundaries();
        let cut = circle.boundaries();
        let geometry = graph
            .build(
                &fluid,
                &BTreeMap::from([
                    (
                        "fluid".to_owned(),
                        vec![PlanarTopologyHandle::from(fluid.region())],
                    ),
                    ("inlet".to_owned(), vec![outer[0].into()]),
                    ("outlet".to_owned(), vec![outer[1].into()]),
                    ("walls".to_owned(), vec![outer[2].into(), outer[3].into()]),
                    ("cylinder".to_owned(), vec![cut[0].into()]),
                ]),
            )
            .unwrap();
        let policy = PlanarMeshQualityV1::new(1.0e-4, 1.0e-5, 50).unwrap();
        let (mesh, correspondence) =
            GeometryMeshCorrespondenceEnvelopeV1::from_planar_circular_hole_v2_reference(
                &geometry,
                policy.maximum_boundary_error_m(),
                policy.maximum_boundary_facets(),
                MeshQualityGate::new(policy.minimum_mean_ratio()).unwrap(),
            )
            .unwrap();
        (geometry, mesh, correspondence)
    }

    fn cartesian_resources() -> (
        CanonicalGeometryV1,
        CartesianMeshEnvelopeV1,
        GeometryMeshCorrespondenceEnvelopeV1,
    ) {
        let graph = PlanarOperationGraph::new();
        let rectangle = graph.rectangle([0.0, 2.0], [-1.0, 2.0]).unwrap();
        let edges = rectangle.boundaries();
        let geometry = graph
            .build(
                &rectangle,
                &BTreeMap::from([
                    ("region".to_owned(), vec![rectangle.region().into()]),
                    ("left".to_owned(), vec![edges[0].into()]),
                    ("right".to_owned(), vec![edges[1].into()]),
                    ("bottom".to_owned(), vec![edges[2].into()]),
                    ("top".to_owned(), vec![edges[3].into()]),
                ]),
            )
            .unwrap();
        let (mesh, correspondence) =
            GeometryMeshCorrespondenceEnvelopeV1::from_planar_rectangle_v2_cartesian(
                &geometry,
                [2, 3],
            )
            .unwrap();
        (geometry, mesh, correspondence)
    }

    fn affine_triangle_resources() -> (
        CanonicalGeometryV1,
        SimplicialMeshEnvelopeV1,
        GeometryMeshCorrespondenceEnvelopeV1,
    ) {
        let graph = PlanarOperationGraph::new();
        let rectangle = graph.rectangle([0.0, 2.0], [-1.0, 2.0]).unwrap();
        let edges = rectangle.boundaries();
        let geometry = graph
            .build(
                &rectangle,
                &BTreeMap::from([
                    ("region".to_owned(), vec![rectangle.region().into()]),
                    ("left".to_owned(), vec![edges[0].into()]),
                    ("right".to_owned(), vec![edges[1].into()]),
                    ("bottom".to_owned(), vec![edges[2].into()]),
                    ("top".to_owned(), vec![edges[3].into()]),
                ]),
            )
            .unwrap();
        let (mesh, correspondence) =
            GeometryMeshCorrespondenceEnvelopeV1::from_planar_rectangle_v2_affine_triangles(
                &geometry,
                [2, 3],
            )
            .unwrap();
        (geometry, mesh, correspondence)
    }

    #[test]
    fn registered_mesh_production_lineage_replays_and_rejects_mutations() {
        let (geometry, mesh, correspondence) = resources();
        let policy = PlanarMeshQualityV1::new(1.0e-4, 1.0e-5, 50).unwrap();
        let lineage =
            MeshProductionLineageEnvelopeV1::from_planar_circular_hole_reference_v1_resources(
                policy,
                &geometry,
                &mesh,
                &correspondence,
            )
            .unwrap();
        let bytes = lineage.canonical_json().unwrap();
        let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(value["effective_policy"]["kind"], "planar-mesh-quality");
        assert_eq!(
            MeshProductionLineageEnvelopeV1::from_json(&bytes).unwrap(),
            lineage
        );
        assert!(
            lineage
                .validate_against_resources(
                    MeshProductionProvider::Gmsh4152,
                    policy,
                    &geometry,
                    &mesh,
                    &correspondence,
                )
                .is_err()
        );
        for changed_policy in [
            PlanarMeshQualityV1::new(2.0e-4, 1.0e-5, 50).unwrap(),
            PlanarMeshQualityV1::new(1.0e-4, 2.0e-5, 50).unwrap(),
            PlanarMeshQualityV1::new(1.0e-4, 1.0e-5, 51).unwrap(),
        ] {
            assert!(
                lineage
                    .validate_against_resources(
                        MeshProductionProvider::PlanarCircularHoleReferenceV1,
                        changed_policy,
                        &geometry,
                        &mesh,
                        &correspondence,
                    )
                    .is_err()
            );
        }
        let mut mutated: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        mutated["provider"]["version"] = serde_json::Value::String("2".to_owned());
        assert!(
            MeshProductionLineageEnvelopeV1::from_json(&serde_json::to_vec(&mutated).unwrap())
                .is_err()
        );

        let mut unknown: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        unknown["unexpected"] = true.into();
        assert!(
            MeshProductionLineageEnvelopeV1::from_json(&serde_json::to_vec(&unknown).unwrap())
                .is_err()
        );
        assert!(MeshProductionLineageEnvelopeV1::from_json(b"not-json").is_err());
        let mut noncanonical = bytes.clone();
        noncanonical.push(b'\n');
        assert!(MeshProductionLineageEnvelopeV1::from_json(&noncanonical).is_err());

        let resource_digests = [
            ArtifactDigest::from_sha256(geometry.digest_bytes()).to_string(),
            mesh.digest().unwrap().to_string(),
            correspondence.digest().unwrap().to_string(),
        ];
        for digest in resource_digests {
            let resource_mutation = std::str::from_utf8(&bytes)
                .unwrap()
                .replacen(&digest, &"0".repeat(64), 1)
                .into_bytes();
            let mutated_lineage =
                MeshProductionLineageEnvelopeV1::from_json(&resource_mutation).unwrap();
            assert!(
                mutated_lineage
                    .validate_against_resources(
                        MeshProductionProvider::PlanarCircularHoleReferenceV1,
                        policy,
                        &geometry,
                        &mesh,
                        &correspondence,
                    )
                    .is_err()
            );
        }

        let (rectangle, cartesian_mesh, cartesian_correspondence) = cartesian_resources();
        let cells = CartesianMeshCellsV1::new([2, 3]).unwrap();
        let cartesian = MeshProductionLineageEnvelopeV1::from_structured_cartesian_v1_resources(
            cells,
            &rectangle,
            &cartesian_mesh,
            &cartesian_correspondence,
        )
        .unwrap();
        let cartesian_bytes = cartesian.canonical_json().unwrap();
        let cartesian_value: serde_json::Value = serde_json::from_slice(&cartesian_bytes).unwrap();
        assert_eq!(
            cartesian_value["effective_policy"],
            serde_json::json!({
                "kind": "cartesian-cells", "cells": [2, 3]
            })
        );
        cartesian
            .validate_against_structured_cartesian_v1_resources(
                cells,
                &rectangle,
                &cartesian_mesh,
                &cartesian_correspondence,
            )
            .unwrap();
        for changed_cells in [[3, 3], [2, 4]] {
            assert!(
                cartesian
                    .validate_against_structured_cartesian_v1_resources(
                        CartesianMeshCellsV1::new(changed_cells).unwrap(),
                        &rectangle,
                        &cartesian_mesh,
                        &cartesian_correspondence,
                    )
                    .is_err()
            );
        }
        for invalid_cells in [[0, 3], [2, 0], [usize::MAX, 3], [2, usize::MAX]] {
            assert!(CartesianMeshCellsV1::new(invalid_cells).is_err());
        }

        let (foreign_geometry, foreign_mesh, foreign_correspondence) = {
            let graph = PlanarOperationGraph::new();
            let rectangle = graph.rectangle([0.0, 3.0], [-1.0, 2.0]).unwrap();
            let edges = rectangle.boundaries();
            let geometry = graph
                .build(
                    &rectangle,
                    &BTreeMap::from([
                        ("region".to_owned(), vec![rectangle.region().into()]),
                        ("left".to_owned(), vec![edges[0].into()]),
                        ("right".to_owned(), vec![edges[1].into()]),
                        ("bottom".to_owned(), vec![edges[2].into()]),
                        ("top".to_owned(), vec![edges[3].into()]),
                    ]),
                )
                .unwrap();
            let (mesh, correspondence) =
                GeometryMeshCorrespondenceEnvelopeV1::from_planar_rectangle_v2_cartesian(
                    &geometry,
                    [2, 3],
                )
                .unwrap();
            (geometry, mesh, correspondence)
        };
        for resources in [
            (
                &foreign_geometry,
                &cartesian_mesh,
                &cartesian_correspondence,
            ),
            (&rectangle, &foreign_mesh, &cartesian_correspondence),
            (&rectangle, &cartesian_mesh, &foreign_correspondence),
        ] {
            assert!(
                cartesian
                    .validate_against_structured_cartesian_v1_resources(
                        cells,
                        resources.0,
                        resources.1,
                        resources.2,
                    )
                    .is_err()
            );
        }
        let cartesian_resource_digests = [
            ArtifactDigest::from_sha256(rectangle.digest_bytes()).to_string(),
            cartesian_mesh.digest().unwrap().to_string(),
            cartesian_correspondence.digest().unwrap().to_string(),
        ];
        for digest in cartesian_resource_digests {
            let resource_mutation = std::str::from_utf8(&cartesian_bytes)
                .unwrap()
                .replacen(&digest, &"0".repeat(64), 1)
                .into_bytes();
            let mutated_lineage =
                MeshProductionLineageEnvelopeV1::from_json(&resource_mutation).unwrap();
            assert!(
                mutated_lineage
                    .validate_against_structured_cartesian_v1_resources(
                        cells,
                        &rectangle,
                        &cartesian_mesh,
                        &cartesian_correspondence,
                    )
                    .is_err()
            );
        }
        let foreign_lineage =
            MeshProductionLineageEnvelopeV1::from_structured_cartesian_v1_resources(
                cells,
                &foreign_geometry,
                &foreign_mesh,
                &foreign_correspondence,
            )
            .unwrap();
        assert!(
            foreign_lineage
                .validate_against_structured_cartesian_v1_resources(
                    cells,
                    &rectangle,
                    &cartesian_mesh,
                    &cartesian_correspondence,
                )
                .is_err()
        );
        let mut provider_mismatch = cartesian_value.clone();
        provider_mismatch["provider"] = serde_json::json!({
            "identity": REFERENCE_IDENTITY, "version": REFERENCE_VERSION
        });
        assert!(
            MeshProductionLineageEnvelopeV1::from_json(
                &serde_json::to_vec(&provider_mismatch).unwrap()
            )
            .is_err()
        );
    }

    #[test]
    fn affine_triangle_lineage_replays_and_rejects_policy_provider_and_resource_mutation() {
        let (geometry, mesh, correspondence) = affine_triangle_resources();
        let policy = AffineTriangleMeshCellsV1::new([2, 3]).unwrap();
        let lineage = MeshProductionLineageEnvelopeV1::from_affine_triangle_rectangle_v1_resources(
            policy,
            &geometry,
            &mesh,
            &correspondence,
        )
        .unwrap();
        let bytes = lineage.canonical_json().unwrap();
        assert_eq!(
            MeshProductionLineageEnvelopeV1::from_json(&bytes).unwrap(),
            lineage
        );
        lineage
            .validate_against_affine_triangle_rectangle_v1_resources(
                policy,
                &geometry,
                &mesh,
                &correspondence,
            )
            .unwrap();

        let mut diagonal: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        diagonal["effective_policy"]["diagonal"] = "upper-left-to-lower-right".into();
        assert!(
            MeshProductionLineageEnvelopeV1::from_json(&serde_json::to_vec(&diagonal).unwrap())
                .is_err()
        );

        let mut provider: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        provider["provider"]["identity"] = CARTESIAN_IDENTITY.into();
        assert!(
            MeshProductionLineageEnvelopeV1::from_json(&serde_json::to_vec(&provider).unwrap())
                .is_err()
        );

        let counts_bytes = std::str::from_utf8(&bytes)
            .unwrap()
            .replace("\"cells\":[2,3]", "\"cells\":[3,2]")
            .into_bytes();
        let counts = MeshProductionLineageEnvelopeV1::from_json(&counts_bytes).unwrap();
        assert!(
            counts
                .validate_against_affine_triangle_rectangle_v1_resources(
                    policy,
                    &geometry,
                    &mesh,
                    &correspondence,
                )
                .is_err()
        );

        for digest in [
            ArtifactDigest::from_sha256(geometry.digest_bytes()).to_string(),
            mesh.digest().unwrap().to_string(),
            correspondence.digest().unwrap().to_string(),
        ] {
            let resource_bytes = std::str::from_utf8(&bytes)
                .unwrap()
                .replacen(&digest, &"0".repeat(64), 1)
                .into_bytes();
            let resource = MeshProductionLineageEnvelopeV1::from_json(&resource_bytes).unwrap();
            assert!(
                resource
                    .validate_against_affine_triangle_rectangle_v1_resources(
                        policy,
                        &geometry,
                        &mesh,
                        &correspondence,
                    )
                    .is_err()
            );
        }
    }
}
