//! Direct source correspondence for planar rectangle Geometry v2 on a Cartesian Mesh.

use std::collections::BTreeSet;

use eqiora_core::Diagnostic;
use eqiora_geometry::{CanonicalGeometryV1, EDGE_DIMENSION, FACE_DIMENSION};
use eqiora_meshing::{CartesianMesh, MeshEntity, MeshTopology, OrientationCode};
use serde::{Deserialize, Serialize};

use super::{CORRESPONDENCE_SCHEMA, GeometryMeshCorrespondenceEnvelopeV1, WireCorrespondenceV1};
use crate::{
    ArtifactDigest, CANONICAL_ENCODING, CartesianMeshEnvelopeV1, GeometryDecoderLimits,
    invalid_artifact,
};

const SOURCE: &str = "planar-rectangle-v2-cartesian-v1";
const SOURCE_EDGE_COUNT: usize = 4;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct WirePlanarRectangleV2CorrespondenceV1 {
    schema: String,
    encoding: String,
    source: String,
    pub(super) geometry_sha256: String,
    pub(super) mesh_sha256: String,
    dimension: u64,
    face: WireFaceAssignment,
    frontiers: Vec<WireFrontierAssignment>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireFaceAssignment {
    geometry_face: u64,
    cell_indices: Vec<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireFrontierAssignment {
    parent_face: u64,
    geometry_edge: u64,
    facet_indices: Vec<u64>,
}

impl GeometryMeshCorrespondenceEnvelopeV1 {
    /// Generate one exact uniform Cartesian Mesh and its direct source correspondence.
    ///
    /// Source edge order is the rectangle-v2 topology order: x-lower,
    /// x-upper, y-lower, y-upper. Facet membership is derived from canonical
    /// topological multi-indices and exact incidence, never from coordinates.
    pub fn from_planar_rectangle_v2_cartesian(
        geometry: &CanonicalGeometryV1,
        cells: [usize; 2],
    ) -> Result<(CartesianMeshEnvelopeV1, Self), Diagnostic> {
        let bounds = require_planar_rectangle_v2(geometry)?;
        if cells.contains(&0) {
            return Err(invalid_artifact(
                "Cartesian rectangle cell counts must be positive",
            ));
        }
        let native = CartesianMesh::uniform(bounds, &cells)?;
        let mesh = CartesianMeshEnvelopeV1::from_mesh(&native)?;
        let wire = wire_from_resources(geometry, &mesh)?;
        let correspondence = Self {
            wire: WireCorrespondenceV1::PlanarRectangleV2(wire),
        };
        correspondence.validate_local(GeometryDecoderLimits::default())?;
        Ok((mesh, correspondence))
    }

    /// Replay the complete rectangle-v2 Cartesian production relation.
    pub fn validate_against_planar_rectangle_v2_cartesian(
        &self,
        geometry: &CanonicalGeometryV1,
        mesh: &CartesianMeshEnvelopeV1,
        cells: [usize; 2],
    ) -> Result<(), Diagnostic> {
        let (expected_mesh, expected) = Self::from_planar_rectangle_v2_cartesian(geometry, cells)?;
        if mesh != &expected_mesh || self != &expected {
            return Err(invalid_artifact(
                "planar rectangle v2 Cartesian resources differ from exact replay",
            ));
        }
        Ok(())
    }

    /// Mesh entities realizing one exact rectangle-v2 entity set.
    pub fn planar_rectangle_v2_entity_set_entities(
        &self,
        geometry: &CanonicalGeometryV1,
        name: &str,
    ) -> Result<Vec<MeshEntity>, Diagnostic> {
        let WireCorrespondenceV1::PlanarRectangleV2(wire) = &self.wire else {
            return Err(invalid_artifact(
                "correspondence has no planar rectangle v2 entity sets",
            ));
        };
        require_planar_rectangle_v2(geometry)?;
        if wire.geometry_sha256 != ArtifactDigest::from_sha256(geometry.digest_bytes()).to_string()
        {
            return Err(invalid_artifact(
                "correspondence geometry digest differs from planar rectangle Geometry v2",
            ));
        }
        let set = geometry.entity_set(name).ok_or_else(|| {
            invalid_artifact(format!(
                "planar rectangle Geometry v2 has no entity set named '{name}'"
            ))
        })?;
        let mut entities = BTreeSet::new();
        match set.dimension() {
            EDGE_DIMENSION => {
                for &member in set.members() {
                    let frontier = wire
                        .frontiers
                        .get(member)
                        .filter(|entry| usize::try_from(entry.geometry_edge) == Ok(member))
                        .ok_or_else(|| {
                            invalid_artifact(
                                "planar rectangle source edge is absent from correspondence",
                            )
                        })?;
                    for &facet in &frontier.facet_indices {
                        entities
                            .insert(MeshEntity::new(EDGE_DIMENSION, local(facet, "mesh facet")?));
                    }
                }
            }
            FACE_DIMENSION => {
                if set.members() != [0] {
                    return Err(invalid_artifact(
                        "planar rectangle face set differs from its exact source face",
                    ));
                }
                for &cell in &wire.face.cell_indices {
                    entities.insert(MeshEntity::new(FACE_DIMENSION, local(cell, "mesh cell")?));
                }
            }
            _ => {
                return Err(invalid_artifact(
                    "planar rectangle entity set dimension is unsupported",
                ));
            }
        }
        Ok(entities.into_iter().collect())
    }
}

fn wire_from_resources(
    geometry: &CanonicalGeometryV1,
    mesh: &CartesianMeshEnvelopeV1,
) -> Result<WirePlanarRectangleV2CorrespondenceV1, Diagnostic> {
    require_planar_rectangle_v2(geometry)?;
    let native = mesh.mesh();
    if native.topological_dimension() != 2 || mesh.dimension() != 2 {
        return Err(invalid_artifact(
            "planar rectangle correspondence requires a two-dimensional Cartesian Mesh",
        ));
    }
    let (cell_indices, frontiers) = topology_assignments(native)?;
    Ok(WirePlanarRectangleV2CorrespondenceV1 {
        schema: CORRESPONDENCE_SCHEMA.to_owned(),
        encoding: CANONICAL_ENCODING.to_owned(),
        source: SOURCE.to_owned(),
        geometry_sha256: ArtifactDigest::from_sha256(geometry.digest_bytes()).to_string(),
        mesh_sha256: mesh.digest()?.to_string(),
        dimension: 2,
        face: WireFaceAssignment {
            geometry_face: 0,
            cell_indices,
        },
        frontiers,
    })
}

trait RectangleCartesianTopology {
    fn entity_count(&self, dimension: usize) -> Option<usize>;
    fn axis_cell_count(&self, axis: usize) -> Option<usize>;
    fn incidence(
        &self,
        entity: MeshEntity,
        target_dimension: usize,
    ) -> Option<Vec<eqiora_meshing::EntityIncidence>>;
    fn entity_vertices(&self, entity: MeshEntity) -> Option<Vec<MeshEntity>>;
    fn vertex_multi_index(&self, vertex: MeshEntity) -> Option<&[usize]>;
}

impl RectangleCartesianTopology for CartesianMesh {
    fn entity_count(&self, dimension: usize) -> Option<usize> {
        MeshTopology::entity_count(self, dimension)
    }

    fn axis_cell_count(&self, axis: usize) -> Option<usize> {
        CartesianMesh::axis_cell_count(self, axis)
    }

    fn incidence(
        &self,
        entity: MeshEntity,
        target_dimension: usize,
    ) -> Option<Vec<eqiora_meshing::EntityIncidence>> {
        MeshTopology::incidence(self, entity, target_dimension)
    }

    fn entity_vertices(&self, entity: MeshEntity) -> Option<Vec<MeshEntity>> {
        CartesianMesh::entity_vertices(self, entity)
    }

    fn vertex_multi_index(&self, vertex: MeshEntity) -> Option<&[usize]> {
        CartesianMesh::vertex_multi_index(self, vertex)
    }
}

fn topology_assignments(
    native: &impl RectangleCartesianTopology,
) -> Result<(Vec<u64>, Vec<WireFrontierAssignment>), Diagnostic> {
    let cell_count = native
        .entity_count(FACE_DIMENSION)
        .ok_or_else(|| invalid_artifact("Cartesian Mesh has no face stratum"))?;
    let facet_count = native
        .entity_count(EDGE_DIMENSION)
        .ok_or_else(|| invalid_artifact("Cartesian Mesh has no edge stratum"))?;
    let cells = [
        native
            .axis_cell_count(0)
            .ok_or_else(|| invalid_artifact("Cartesian Mesh omitted x-axis cells"))?,
        native
            .axis_cell_count(1)
            .ok_or_else(|| invalid_artifact("Cartesian Mesh omitted y-axis cells"))?,
    ];
    let mut source_facets: [Vec<usize>; SOURCE_EDGE_COUNT] = std::array::from_fn(|_| Vec::new());
    let mut exposed = BTreeSet::new();
    for facet_index in 0..facet_count {
        let facet = MeshEntity::new(EDGE_DIMENSION, facet_index);
        let adjacent = native
            .incidence(facet, FACE_DIMENSION)
            .ok_or_else(|| invalid_artifact("Cartesian frontier incidence is unavailable"))?;
        if adjacent.len() == 2 {
            continue;
        }
        let [parent] = adjacent.as_slice() else {
            return Err(invalid_artifact(
                "Cartesian frontier requires exactly one parent-cell incidence",
            ));
        };
        if parent.orientation != OrientationCode::identity() || parent.local_ordinal >= 4 {
            return Err(invalid_artifact(
                "Cartesian frontier has noncanonical local-side orientation",
            ));
        }
        let vertices = native
            .entity_vertices(facet)
            .ok_or_else(|| invalid_artifact("Cartesian frontier has no vertex closure"))?;
        let source_edge = structural_source_edge(native, &vertices, cells)?;
        let expected_local_side = [2, 3, 0, 1][source_edge];
        if parent.local_ordinal != expected_local_side {
            return Err(invalid_artifact(
                "Cartesian frontier local side differs from its source rectangle edge",
            ));
        }
        source_facets[source_edge].push(facet_index);
        exposed.insert(facet_index);
    }
    let assigned = source_facets
        .iter()
        .flatten()
        .copied()
        .collect::<BTreeSet<_>>();
    if source_facets.iter().any(Vec::is_empty) || assigned != exposed {
        return Err(invalid_artifact(
            "Cartesian rectangle frontiers must completely and exclusively realize four source edges",
        ));
    }
    let frontiers = source_facets
        .into_iter()
        .enumerate()
        .map(|(geometry_edge, facets)| {
            Ok(WireFrontierAssignment {
                parent_face: 0,
                geometry_edge: portable(geometry_edge, "geometry edge")?,
                facet_indices: facets
                    .into_iter()
                    .map(|facet| portable(facet, "mesh facet"))
                    .collect::<Result<Vec<_>, _>>()?,
            })
        })
        .collect::<Result<Vec<_>, Diagnostic>>()?;
    let cell_indices = (0..cell_count)
        .map(|cell| portable(cell, "mesh cell"))
        .collect::<Result<Vec<_>, _>>()?;
    Ok((cell_indices, frontiers))
}

fn structural_source_edge(
    mesh: &impl RectangleCartesianTopology,
    vertices: &[MeshEntity],
    cells: [usize; 2],
) -> Result<usize, Diagnostic> {
    if vertices.len() != 2 {
        return Err(invalid_artifact(
            "Cartesian rectangle frontier must be a segment",
        ));
    }
    let indices = vertices
        .iter()
        .map(|&vertex| {
            mesh.vertex_multi_index(vertex)
                .map(<[usize]>::to_vec)
                .ok_or_else(|| invalid_artifact("Cartesian frontier vertex index is unavailable"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    for axis in 0..2 {
        if indices.iter().all(|index| index[axis] == indices[0][axis]) {
            return match (axis, indices[0][axis]) {
                (0, 0) => Ok(0),
                (0, value) if value == cells[0] => Ok(1),
                (1, 0) => Ok(2),
                (1, value) if value == cells[1] => Ok(3),
                _ => Err(invalid_artifact(
                    "Cartesian exposed facet is not on a canonical rectangle side",
                )),
            };
        }
    }
    Err(invalid_artifact(
        "Cartesian rectangle frontier has no fixed structural axis",
    ))
}

fn require_planar_rectangle_v2(
    geometry: &CanonicalGeometryV1,
) -> Result<&[[f64; 2]; 2], Diagnostic> {
    let replayed = CanonicalGeometryV1::decode_planar_rectangle_v2_canonical(
        geometry.canonical_bytes(),
        Default::default(),
    )
    .map_err(|_| invalid_artifact("CartesianMesher requires planar rectangle Geometry v2"))?;
    if replayed != *geometry {
        return Err(invalid_artifact(
            "planar rectangle Geometry v2 differs from canonical replay",
        ));
    }
    geometry
        .planar_rectangle_bounds()
        .ok_or_else(|| invalid_artifact("CartesianMesher requires planar rectangle Geometry v2"))
}

impl WirePlanarRectangleV2CorrespondenceV1 {
    pub(super) fn validate_local(&self, limits: GeometryDecoderLimits) -> Result<(), Diagnostic> {
        if self.schema != CORRESPONDENCE_SCHEMA
            || self.encoding != CANONICAL_ENCODING
            || self.source != SOURCE
            || self.dimension != 2
        {
            return Err(invalid_artifact(
                "unsupported planar rectangle v2 correspondence schema, encoding, source, or dimension",
            ));
        }
        ArtifactDigest::from_hex(self.geometry_sha256.clone())?;
        ArtifactDigest::from_hex(self.mesh_sha256.clone())?;
        if self.face.geometry_face != 0
            || self.face.cell_indices.is_empty()
            || !strictly_sorted(&self.face.cell_indices)
            || self.frontiers.len() != SOURCE_EDGE_COUNT
        {
            return Err(invalid_artifact(
                "planar rectangle correspondence requires one complete face and four source frontiers",
            ));
        }
        let canonical = self.frontiers.iter().enumerate().all(|(edge, frontier)| {
            frontier.parent_face == 0
                && usize::try_from(frontier.geometry_edge) == Ok(edge)
                && !frontier.facet_indices.is_empty()
                && strictly_sorted(&frontier.facet_indices)
        });
        let unique = self
            .frontiers
            .iter()
            .flat_map(|frontier| frontier.facet_indices.iter().copied())
            .collect::<BTreeSet<_>>();
        let frontier_memberships = self
            .frontiers
            .iter()
            .map(|frontier| frontier.facet_indices.len())
            .sum::<usize>();
        let memberships = self
            .face
            .cell_indices
            .len()
            .checked_add(frontier_memberships)
            .ok_or_else(|| invalid_artifact("rectangle membership count overflows usize"))?;
        if !canonical
            || unique.len() != frontier_memberships
            || SOURCE_EDGE_COUNT + 1 > limits.max_geometry_entities
            || memberships > limits.max_geometry_mesh_memberships
        {
            return Err(invalid_artifact(
                "planar rectangle assignments must be canonical, disjoint, and within limits",
            ));
        }
        Ok(())
    }
}

fn strictly_sorted(values: &[u64]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn portable(value: usize, label: &str) -> Result<u64, Diagnostic> {
    u64::try_from(value)
        .map_err(|_| invalid_artifact(format!("{label} index exceeds portable u64")))
}

fn local(value: u64, label: &str) -> Result<usize, Diagnostic> {
    usize::try_from(value)
        .map_err(|_| invalid_artifact(format!("{label} index exceeds local usize")))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use eqiora_geometry::{PlanarOperationGraph, PlanarTopologyHandle};
    use eqiora_meshing::EntityIncidence;

    use super::*;
    use crate::{CartesianMeshCellsV1, MeshProductionLineageEnvelopeV1};

    fn rectangle() -> CanonicalGeometryV1 {
        let graph = PlanarOperationGraph::new();
        let rectangle = graph.rectangle([0.0, 2.0], [-1.0, 2.0]).unwrap();
        let edges = rectangle.boundaries();
        graph
            .build(
                &rectangle,
                &BTreeMap::from([
                    (
                        "region".to_owned(),
                        vec![PlanarTopologyHandle::from(rectangle.region())],
                    ),
                    ("left".to_owned(), vec![edges[0].into()]),
                    ("right".to_owned(), vec![edges[1].into()]),
                    ("bottom".to_owned(), vec![edges[2].into()]),
                    ("top".to_owned(), vec![edges[3].into()]),
                ]),
            )
            .unwrap()
    }

    fn entity_indices(entities: Vec<MeshEntity>) -> Vec<usize> {
        entities.into_iter().map(MeshEntity::index).collect()
    }

    fn non_rectangle() -> CanonicalGeometryV1 {
        let graph = PlanarOperationGraph::new();
        let rectangle = graph.rectangle([0.0, 2.0], [-1.0, 2.0]).unwrap();
        let circle = graph.circle([1.0, 0.5], 0.25).unwrap();
        let cut = graph.subtract(&rectangle, &circle).unwrap();
        let outer = rectangle.boundaries();
        let hole = circle.boundaries();
        graph
            .build(
                &cut,
                &BTreeMap::from([
                    ("region".to_owned(), vec![cut.region().into()]),
                    ("left".to_owned(), vec![outer[0].into()]),
                    ("right".to_owned(), vec![outer[1].into()]),
                    ("walls".to_owned(), vec![outer[2].into(), outer[3].into()]),
                    ("hole".to_owned(), vec![hole[0].into()]),
                ]),
            )
            .unwrap()
    }

    #[derive(Clone, Copy)]
    enum TopologyMutation {
        Connectivity,
        LocalOrdinal,
        Orientation,
    }

    struct MutatedTopology<'a> {
        native: &'a CartesianMesh,
        mutation: TopologyMutation,
    }

    impl RectangleCartesianTopology for MutatedTopology<'_> {
        fn entity_count(&self, dimension: usize) -> Option<usize> {
            MeshTopology::entity_count(self.native, dimension)
        }

        fn axis_cell_count(&self, axis: usize) -> Option<usize> {
            self.native.axis_cell_count(axis)
        }

        fn incidence(
            &self,
            entity: MeshEntity,
            target_dimension: usize,
        ) -> Option<Vec<EntityIncidence>> {
            let mut incidence = MeshTopology::incidence(self.native, entity, target_dimension)?;
            if entity == MeshEntity::new(1, 8) && target_dimension == 2 {
                match self.mutation {
                    TopologyMutation::LocalOrdinal => incidence[0].local_ordinal = 0,
                    TopologyMutation::Orientation => {
                        incidence[0].orientation = OrientationCode::new(1)
                    }
                    TopologyMutation::Connectivity => {}
                }
            }
            Some(incidence)
        }

        fn entity_vertices(&self, entity: MeshEntity) -> Option<Vec<MeshEntity>> {
            let source = if matches!(self.mutation, TopologyMutation::Connectivity)
                && entity == MeshEntity::new(1, 8)
            {
                MeshEntity::new(1, 11)
            } else {
                entity
            };
            self.native.entity_vertices(source)
        }

        fn vertex_multi_index(&self, vertex: MeshEntity) -> Option<&[usize]> {
            self.native.vertex_multi_index(vertex)
        }
    }

    fn mutate_wire(
        correspondence: &GeometryMeshCorrespondenceEnvelopeV1,
        mutation: impl FnOnce(&mut WirePlanarRectangleV2CorrespondenceV1),
    ) -> GeometryMeshCorrespondenceEnvelopeV1 {
        let mut mutated = correspondence.clone();
        let WireCorrespondenceV1::PlanarRectangleV2(wire) = &mut mutated.wire else {
            panic!("expected planar rectangle correspondence")
        };
        mutation(wire);
        mutated
    }

    #[test]
    fn rectangle_cartesian_resources_have_analytic_counts_and_direct_membership() {
        let geometry = rectangle();
        let (mesh, correspondence) =
            GeometryMeshCorrespondenceEnvelopeV1::from_planar_rectangle_v2_cartesian(
                &geometry,
                [2, 3],
            )
            .unwrap();
        let native = mesh.mesh();
        assert_eq!(native.axis_coordinates(0), Some(&[0.0, 1.0, 2.0][..]));
        assert_eq!(native.axis_coordinates(1), Some(&[-1.0, 0.0, 1.0, 2.0][..]));
        let expected_coordinates = [
            [0.0, -1.0],
            [0.0, 0.0],
            [0.0, 1.0],
            [0.0, 2.0],
            [1.0, -1.0],
            [1.0, 0.0],
            [1.0, 1.0],
            [1.0, 2.0],
            [2.0, -1.0],
            [2.0, 0.0],
            [2.0, 1.0],
            [2.0, 2.0],
        ];
        for (vertex, expected) in expected_coordinates.into_iter().enumerate() {
            assert_eq!(
                native.vertex_coordinates(MeshEntity::new(0, vertex)),
                Some(expected.to_vec())
            );
        }
        let expected_cells = [
            [0, 4, 1, 5],
            [1, 5, 2, 6],
            [2, 6, 3, 7],
            [4, 8, 5, 9],
            [5, 9, 6, 10],
            [6, 10, 7, 11],
        ];
        for (cell, expected) in expected_cells.into_iter().enumerate() {
            assert_eq!(
                entity_indices(native.entity_vertices(MeshEntity::new(2, cell)).unwrap()),
                expected
            );
        }
        let expected_facets = [
            [0, 4],
            [1, 5],
            [2, 6],
            [3, 7],
            [4, 8],
            [5, 9],
            [6, 10],
            [7, 11],
            [0, 1],
            [1, 2],
            [2, 3],
            [4, 5],
            [5, 6],
            [6, 7],
            [8, 9],
            [9, 10],
            [10, 11],
        ];
        for (facet, expected) in expected_facets.into_iter().enumerate() {
            assert_eq!(
                entity_indices(native.entity_vertices(MeshEntity::new(1, facet)).unwrap()),
                expected
            );
        }
        assert_eq!(
            ["left", "right", "bottom", "top", "region"].map(|name| entity_indices(
                correspondence
                    .planar_rectangle_v2_entity_set_entities(&geometry, name)
                    .unwrap()
            )),
            [
                vec![8, 9, 10],
                vec![14, 15, 16],
                vec![0, 4],
                vec![3, 7],
                vec![0, 1, 2, 3, 4, 5],
            ]
        );
        let expected_boundary_incidence = [
            (0, 0, 0),
            (4, 3, 0),
            (3, 2, 1),
            (7, 5, 1),
            (8, 0, 2),
            (9, 1, 2),
            (10, 2, 2),
            (14, 3, 3),
            (15, 4, 3),
            (16, 5, 3),
        ];
        for (facet, cell, local_ordinal) in expected_boundary_incidence {
            assert_eq!(
                MeshTopology::incidence(native, MeshEntity::new(1, facet), 2),
                Some(vec![EntityIncidence {
                    entity: MeshEntity::new(2, cell),
                    local_ordinal,
                    orientation: OrientationCode::identity(),
                }])
            );
        }
        let expected_boundary = BTreeSet::from([0, 3, 4, 7, 8, 9, 10, 14, 15, 16]);
        let expected_interior = BTreeSet::from([1, 2, 5, 6, 11, 12, 13]);
        let actual_boundary = (0..17)
            .filter(|&facet| {
                MeshTopology::incidence(native, MeshEntity::new(1, facet), 2)
                    .is_some_and(|parents| parents.len() == 1)
            })
            .collect::<BTreeSet<_>>();
        let actual_interior = (0..17)
            .filter(|&facet| {
                MeshTopology::incidence(native, MeshEntity::new(1, facet), 2)
                    .is_some_and(|parents| parents.len() == 2)
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(actual_boundary, expected_boundary);
        assert_eq!(actual_interior, expected_interior);
        assert!(actual_boundary.is_disjoint(&actual_interior));
        assert_eq!(actual_boundary.len() + actual_interior.len(), 17);

        let WireCorrespondenceV1::PlanarRectangleV2(wire) = &correspondence.wire else {
            panic!("expected planar rectangle correspondence")
        };
        assert_eq!(wire.face.cell_indices, vec![0, 1, 2, 3, 4, 5]);
        assert_eq!(
            wire.frontiers
                .iter()
                .map(|frontier| frontier.facet_indices.clone())
                .collect::<Vec<_>>(),
            [vec![8, 9, 10], vec![14, 15, 16], vec![0, 4], vec![3, 7]]
        );
        correspondence
            .validate_against_planar_rectangle_v2_cartesian(&geometry, &mesh, [2, 3])
            .unwrap();
        assert!(
            correspondence
                .validate_against_planar_rectangle_v2_cartesian(&geometry, &mesh, [3, 2])
                .is_err()
        );
        let (replayed_mesh, replayed_correspondence) =
            GeometryMeshCorrespondenceEnvelopeV1::from_planar_rectangle_v2_cartesian(
                &geometry,
                [2, 3],
            )
            .unwrap();
        assert_eq!(replayed_mesh, mesh);
        assert_eq!(replayed_correspondence, correspondence);
    }

    #[test]
    fn rectangle_correspondence_rejects_wire_and_resource_mutations() {
        let geometry = rectangle();
        let (mesh, correspondence) =
            GeometryMeshCorrespondenceEnvelopeV1::from_planar_rectangle_v2_cartesian(
                &geometry,
                [2, 3],
            )
            .unwrap();
        let bytes = correspondence.canonical_json().unwrap();
        assert_eq!(
            GeometryMeshCorrespondenceEnvelopeV1::from_json(&bytes, Default::default()).unwrap(),
            correspondence
        );
        for mutation in [
            |value: &mut serde_json::Value| value["source"] = "other".into(),
            |value: &mut serde_json::Value| value["dimension"] = 3.into(),
            |value: &mut serde_json::Value| {
                value["frontiers"][0]["facet_indices"] = serde_json::json!([])
            },
            |value: &mut serde_json::Value| value["frontiers"][1]["geometry_edge"] = 0.into(),
        ] {
            let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            mutation(&mut value);
            assert!(
                GeometryMeshCorrespondenceEnvelopeV1::from_json(
                    &serde_json::to_vec(&value).unwrap(),
                    Default::default(),
                )
                .is_err()
            );
        }
        for swapped in [
            mutate_wire(&correspondence, |wire| {
                let left = wire.frontiers[0].facet_indices.clone();
                wire.frontiers[0].facet_indices = wire.frontiers[1].facet_indices.clone();
                wire.frontiers[1].facet_indices = left;
            }),
            mutate_wire(&correspondence, |wire| {
                let bottom = wire.frontiers[2].facet_indices.clone();
                wire.frontiers[2].facet_indices = wire.frontiers[3].facet_indices.clone();
                wire.frontiers[3].facet_indices = bottom;
            }),
            mutate_wire(&correspondence, |wire| {
                wire.frontiers[0].facet_indices = vec![8, 9, 11];
            }),
        ] {
            swapped.validate_local(Default::default()).unwrap();
            assert!(
                swapped
                    .validate_against_planar_rectangle_v2_cartesian(&geometry, &mesh, [2, 3])
                    .is_err()
            );
        }
        for mutation in [
            TopologyMutation::Connectivity,
            TopologyMutation::LocalOrdinal,
            TopologyMutation::Orientation,
        ] {
            assert!(
                topology_assignments(&MutatedTopology {
                    native: mesh.mesh(),
                    mutation,
                })
                .is_err()
            );
        }
        let alternate_geometry = {
            let graph = PlanarOperationGraph::new();
            let rectangle = graph.rectangle([0.0, 3.0], [-1.0, 2.0]).unwrap();
            let edges = rectangle.boundaries();
            graph
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
                .unwrap()
        };
        assert!(
            correspondence
                .validate_against_planar_rectangle_v2_cartesian(&alternate_geometry, &mesh, [2, 3])
                .is_err()
        );
        assert!(
            GeometryMeshCorrespondenceEnvelopeV1::from_planar_rectangle_v2_cartesian(
                &non_rectangle(),
                [2, 3],
            )
            .is_err()
        );
        for invalid_cells in [[0, 3], [2, 0], [usize::MAX, 3], [2, usize::MAX]] {
            assert!(
                GeometryMeshCorrespondenceEnvelopeV1::from_planar_rectangle_v2_cartesian(
                    &geometry,
                    invalid_cells,
                )
                .is_err()
            );
        }
        let (foreign_mesh, foreign_correspondence) =
            GeometryMeshCorrespondenceEnvelopeV1::from_planar_rectangle_v2_cartesian(
                &geometry,
                [3, 2],
            )
            .unwrap();
        assert!(
            correspondence
                .validate_against_planar_rectangle_v2_cartesian(&geometry, &foreign_mesh, [2, 3])
                .is_err()
        );
        assert!(
            foreign_correspondence
                .validate_against_planar_rectangle_v2_cartesian(&geometry, &mesh, [2, 3])
                .is_err()
        );
        let cells = CartesianMeshCellsV1::new([2, 3]).unwrap();
        let lineage = MeshProductionLineageEnvelopeV1::from_structured_cartesian_v1_resources(
            cells,
            &geometry,
            &mesh,
            &correspondence,
        )
        .unwrap();
        let foreign_lineage =
            MeshProductionLineageEnvelopeV1::from_structured_cartesian_v1_resources(
                CartesianMeshCellsV1::new([3, 2]).unwrap(),
                &geometry,
                &foreign_mesh,
                &foreign_correspondence,
            )
            .unwrap();
        lineage
            .validate_against_structured_cartesian_v1_resources(
                cells,
                &geometry,
                &mesh,
                &correspondence,
            )
            .unwrap();
        assert!(
            foreign_lineage
                .validate_against_structured_cartesian_v1_resources(
                    cells,
                    &geometry,
                    &mesh,
                    &correspondence,
                )
                .is_err()
        );
    }

    #[test]
    fn registered_rectangle_cartesian_common_mesh_evidence() {
        rectangle_cartesian_resources_have_analytic_counts_and_direct_membership();
        rectangle_correspondence_rejects_wire_and_resource_mutations();
    }
}
