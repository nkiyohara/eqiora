//! Application projection for one exact unstructured P1 scalar Field.
//!
//! Semantic and artifact validation remains below this boundary. The
//! projection owns a bounded, renderer-ready copy so presentation adapters do
//! not reinterpret a Field snapshot or retain borrowed artifact state.

use eqiora_artifact::{
    ArtifactDigest, DiscreteFieldEnvelopeV1, FieldSnapshotEnvelopeV1, RunManifestV2,
    ValidatedFixedSpatialContextV1,
};
use eqiora_core::entity::kinds;
use eqiora_core::{Diagnostic, DimExponents, Id};
use eqiora_meshing::{DiscreteFieldAssociation, DiscreteFieldShape};

const MAX_STUDIO_P1_VERTICES: usize = 250_000;
const MAX_STUDIO_P1_TRIANGLES: usize = 500_000;

/// Complete bounded projection of one coherent-SI scalar P1 Field on affine triangles.
///
/// This is an application value, not a durable artifact or general
/// visualization schema. Every identity is derived from and checked against
/// the supplied fixed-spatial lineage before coordinates, connectivity, or
/// values are copied.
#[derive(Debug, Clone, PartialEq)]
pub struct UnstructuredP1ScalarFieldProjection2d {
    model_artifact: ArtifactDigest,
    semantic_revision: u64,
    realization_artifact: ArtifactDigest,
    run_artifact: ArtifactDigest,
    snapshot_artifact: ArtifactDigest,
    mesh_artifact: ArtifactDigest,
    field: Id<kinds::Field>,
    support_domain: Id<kinds::Domain>,
    value_dimension: DimExponents,
    bounds_m: [[f64; 2]; 2],
    vertices_m: Vec<[f64; 2]>,
    triangles: Vec<[u32; 3]>,
    values: Vec<f64>,
    minimum: f64,
    maximum: f64,
}

impl UnstructuredP1ScalarFieldProjection2d {
    /// Validate and materialize one exact two-dimensional P1 scalar snapshot.
    ///
    /// Model revision, Realization plan, Run, Field, Domain, mesh, coefficient
    /// association, component shape, counts, and finite data are checked
    /// before a complete projection is returned.
    ///
    /// # Errors
    /// Returns `EQ0901` for foreign or stale lineage, a non-P1-scalar Field,
    /// unsupported dimension, resource excess, or inconsistent mesh/values.
    pub fn from_fixed_snapshot(
        context: &ValidatedFixedSpatialContextV1<'_>,
        run: &RunManifestV2,
        snapshot: &FieldSnapshotEnvelopeV1,
        block: &DiscreteFieldEnvelopeV1,
    ) -> Result<Self, Diagnostic> {
        run.validate_against(context.realization())?;
        snapshot.validate_against(context, std::slice::from_ref(block))?;
        let snapshot_artifact = snapshot.digest()?;
        if !run.outputs().contains(&snapshot_artifact) {
            return Err(invalid_projection(
                "Studio P1 projection snapshot is not an output of the exact Run",
            ));
        }

        let mesh_artifact = context.mesh().digest()?;
        if context.mesh().dimension() != 2 {
            return Err(invalid_projection(
                "Studio P1 projection requires one two-dimensional affine-triangle mesh",
            ));
        }
        if !snapshot.value_shape().is_scalar()
            || block.association() != DiscreteFieldAssociation::Vertex
            || block.component_shape() != DiscreteFieldShape::Scalar
        {
            return Err(invalid_projection(
                "Studio P1 projection requires one scalar vertex coefficient block",
            ));
        }
        if block.mesh_artifact() != mesh_artifact {
            return Err(invalid_projection(
                "Studio P1 projection Field references a foreign mesh artifact",
            ));
        }

        let mesh = context.mesh().mesh();
        let vertex_count = mesh.vertices().len();
        let triangle_count = mesh.cells().len();
        if vertex_count == 0
            || vertex_count > MAX_STUDIO_P1_VERTICES
            || triangle_count == 0
            || triangle_count > MAX_STUDIO_P1_TRIANGLES
        {
            return Err(invalid_projection(format!(
                "Studio P1 projection admits at most {MAX_STUDIO_P1_VERTICES} vertices and \
                 {MAX_STUDIO_P1_TRIANGLES} triangles",
            )));
        }
        if block.entity_count()? != vertex_count || block.values().len() != vertex_count {
            return Err(invalid_projection(
                "Studio P1 projection requires exactly one scalar per mesh vertex",
            ));
        }

        let mut vertices_m = Vec::new();
        vertices_m.try_reserve_exact(vertex_count).map_err(|_| {
            invalid_projection("Studio P1 coordinate allocation exceeds available capacity")
        })?;
        let mut bounds_m = [[f64::INFINITY, f64::NEG_INFINITY]; 2];
        for vertex in mesh.vertices() {
            let [x, y] = vertex.as_slice() else {
                return Err(invalid_projection(
                    "Studio P1 mesh vertex does not have exactly two coordinates",
                ));
            };
            if !x.is_finite() || !y.is_finite() {
                return Err(invalid_projection(
                    "Studio P1 mesh contains a non-finite coordinate",
                ));
            }
            vertices_m.push([*x, *y]);
            bounds_m[0][0] = bounds_m[0][0].min(*x);
            bounds_m[0][1] = bounds_m[0][1].max(*x);
            bounds_m[1][0] = bounds_m[1][0].min(*y);
            bounds_m[1][1] = bounds_m[1][1].max(*y);
        }
        if bounds_m.iter().any(|[lower, upper]| upper <= lower) {
            return Err(invalid_projection(
                "Studio P1 coordinate bounds must have positive extent",
            ));
        }

        let mut triangles = Vec::new();
        triangles.try_reserve_exact(triangle_count).map_err(|_| {
            invalid_projection("Studio P1 connectivity allocation exceeds available capacity")
        })?;
        for cell in mesh.cells() {
            let [a, b, c] = cell.as_slice() else {
                return Err(invalid_projection(
                    "Studio P1 mesh cell is not an affine triangle",
                ));
            };
            let portable = |vertex| {
                u32::try_from(vertex)
                    .map_err(|_| invalid_projection("Studio P1 vertex index exceeds portable u32"))
            };
            triangles.push([portable(*a)?, portable(*b)?, portable(*c)?]);
        }

        let mut values = Vec::new();
        values.try_reserve_exact(vertex_count).map_err(|_| {
            invalid_projection("Studio P1 value allocation exceeds available capacity")
        })?;
        let mut minimum = f64::INFINITY;
        let mut maximum = f64::NEG_INFINITY;
        for &value in block.values() {
            if !value.is_finite() {
                return Err(invalid_projection(
                    "Studio P1 Field contains a non-finite scalar",
                ));
            }
            values.push(value);
            minimum = minimum.min(value);
            maximum = maximum.max(value);
        }

        let model = context.model_reference();
        Ok(Self {
            model_artifact: model.artifact().clone(),
            semantic_revision: model.semantic_revision().get(),
            realization_artifact: context.realization().digest()?,
            run_artifact: run.digest()?,
            snapshot_artifact,
            mesh_artifact,
            field: snapshot.field(),
            support_domain: snapshot.support_domain(),
            value_dimension: snapshot.dimension(),
            bounds_m,
            vertices_m,
            triangles,
            values,
            minimum,
            maximum,
        })
    }

    /// Exact canonical Model artifact.
    #[must_use]
    pub const fn model_artifact(&self) -> &ArtifactDigest {
        &self.model_artifact
    }

    /// Exact semantic graph revision carried by the Model artifact.
    #[must_use]
    pub const fn semantic_revision(&self) -> u64 {
        self.semantic_revision
    }

    /// Exact Realization artifact which owns the accepted plan.
    #[must_use]
    pub const fn realization_artifact(&self) -> &ArtifactDigest {
        &self.realization_artifact
    }

    /// Exact Run artifact.
    #[must_use]
    pub const fn run_artifact(&self) -> &ArtifactDigest {
        &self.run_artifact
    }

    /// Exact logical Field snapshot artifact.
    #[must_use]
    pub const fn snapshot_artifact(&self) -> &ArtifactDigest {
        &self.snapshot_artifact
    }

    /// Exact affine-triangle mesh artifact.
    #[must_use]
    pub const fn mesh_artifact(&self) -> &ArtifactDigest {
        &self.mesh_artifact
    }

    /// Exact Semantic Field identity.
    #[must_use]
    pub const fn field(&self) -> Id<kinds::Field> {
        self.field
    }

    /// Exact Semantic Domain supporting the Field.
    #[must_use]
    pub const fn support_domain(&self) -> Id<kinds::Domain> {
        self.support_domain
    }

    /// Physical value dimension in coherent SI.
    #[must_use]
    pub const fn value_dimension(&self) -> DimExponents {
        self.value_dimension
    }

    /// Coordinate bounds in coherent-SI metres.
    #[must_use]
    pub const fn bounds_m(&self) -> &[[f64; 2]; 2] {
        &self.bounds_m
    }

    /// Complete coordinates in canonical mesh-vertex order.
    #[must_use]
    pub fn vertices_m(&self) -> &[[f64; 2]] {
        &self.vertices_m
    }

    /// Complete connectivity in canonical mesh-cell order.
    ///
    /// Positive orientation and index validity are inherited from the
    /// admitted [`eqiora_meshing::SimplicialMesh`] held by the fixed context.
    #[must_use]
    pub fn triangles(&self) -> &[[u32; 3]] {
        &self.triangles
    }

    /// Complete scalar coefficients in canonical mesh-vertex order.
    #[must_use]
    pub fn values(&self) -> &[f64] {
        &self.values
    }

    /// Minimum accepted scalar value.
    #[must_use]
    pub const fn minimum(&self) -> f64 {
        self.minimum
    }

    /// Maximum accepted scalar value.
    #[must_use]
    pub const fn maximum(&self) -> f64 {
        self.maximum
    }
}

fn invalid_projection(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(eqiora_core::diagnostic::codes::INVALID_ARTIFACT, message)
}
