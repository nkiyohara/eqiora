use eqiora_artifact::{
    GeometryDefinitionV1, GeometryMeshCorrespondenceEnvelopeV1, SimplicialMeshEnvelopeV1,
};
use eqiora_geometry::{
    EDGE_DIMENSION, FACE_DIMENSION, NamedEntitySet, PlanarFace, PlanarRegion, VERTEX_DIMENSION,
};
use eqiora_meshing::{MeshQualityGate, SimplicialMesh};
use serde_json::Value;

fn square_with_hole(extra_sets: Vec<NamedEntitySet>) -> GeometryDefinitionV1 {
    let mut sets = vec![
        NamedEntitySet::new("corners", VERTEX_DIMENSION, (0..8).collect()),
        NamedEntitySet::new("exterior", EDGE_DIMENSION, vec![0, 1, 2, 3]),
        NamedEntitySet::new("hole", EDGE_DIMENSION, vec![4, 5, 6, 7]),
        NamedEntitySet::new("fluid", FACE_DIMENSION, vec![0]),
    ];
    sets.extend(extra_sets);
    let region = PlanarRegion::new(
        vec![
            [0.0, 0.0],
            [1.0, 0.0],
            [1.0, 1.0],
            [0.0, 1.0],
            [0.25, 0.25],
            [0.75, 0.25],
            [0.75, 0.75],
            [0.25, 0.75],
        ],
        vec![PlanarFace::new(vec![0, 1, 2, 3], vec![vec![4, 5, 6, 7]])],
        sets,
        1.0e-9,
    )
    .expect("square-with-hole geometry is valid");
    GeometryDefinitionV1::from_region(&region)
}

fn grid_vertices() -> Vec<Vec<f64>> {
    vec![
        vec![0.0, 0.0],
        vec![0.25, 0.0],
        vec![0.5, 0.0],
        vec![0.75, 0.0],
        vec![1.0, 0.0],
        vec![0.0, 0.25],
        vec![0.25, 0.25],
        vec![0.5, 0.25],
        vec![0.75, 0.25],
        vec![1.0, 0.25],
        vec![0.0, 0.5],
        vec![0.25, 0.5],
        vec![0.75, 0.5],
        vec![1.0, 0.5],
        vec![0.0, 0.75],
        vec![0.25, 0.75],
        vec![0.5, 0.75],
        vec![0.75, 0.75],
        vec![1.0, 0.75],
        vec![0.0, 1.0],
        vec![0.25, 1.0],
        vec![0.5, 1.0],
        vec![0.75, 1.0],
        vec![1.0, 1.0],
    ]
}

fn forward_diagonal_cells() -> Vec<Vec<usize>> {
    vec![
        vec![0, 1, 6],
        vec![0, 6, 5],
        vec![1, 2, 7],
        vec![1, 7, 6],
        vec![2, 3, 8],
        vec![2, 8, 7],
        vec![3, 4, 9],
        vec![3, 9, 8],
        vec![5, 6, 11],
        vec![5, 11, 10],
        vec![8, 9, 13],
        vec![8, 13, 12],
        vec![10, 11, 15],
        vec![10, 15, 14],
        vec![12, 13, 18],
        vec![12, 18, 17],
        vec![14, 15, 20],
        vec![14, 20, 19],
        vec![15, 16, 21],
        vec![15, 21, 20],
        vec![16, 17, 22],
        vec![16, 22, 21],
        vec![17, 18, 23],
        vec![17, 23, 22],
    ]
}

fn reverse_diagonal_cells() -> Vec<Vec<usize>> {
    vec![
        vec![0, 1, 5],
        vec![1, 6, 5],
        vec![1, 2, 6],
        vec![2, 7, 6],
        vec![2, 3, 7],
        vec![3, 8, 7],
        vec![3, 4, 8],
        vec![4, 9, 8],
        vec![5, 6, 10],
        vec![6, 11, 10],
        vec![8, 9, 12],
        vec![9, 13, 12],
        vec![10, 11, 14],
        vec![11, 15, 14],
        vec![12, 13, 17],
        vec![13, 18, 17],
        vec![14, 15, 19],
        vec![15, 20, 19],
        vec![15, 16, 20],
        vec![16, 21, 20],
        vec![16, 17, 21],
        vec![17, 22, 21],
        vec![17, 18, 22],
        vec![18, 23, 22],
    ]
}

fn mesh_artifact(vertices: Vec<Vec<f64>>, cells: Vec<Vec<usize>>) -> SimplicialMeshEnvelopeV1 {
    let mesh = SimplicialMesh::new(2, vertices, cells, MeshQualityGate::new(0.1).unwrap())
        .expect("fixture is an accepted affine triangulation");
    SimplicialMeshEnvelopeV1::from_mesh(&mesh).unwrap()
}

fn forward_mesh() -> SimplicialMeshEnvelopeV1 {
    mesh_artifact(grid_vertices(), forward_diagonal_cells())
}

fn reverse_mesh() -> SimplicialMeshEnvelopeV1 {
    mesh_artifact(grid_vertices(), reverse_diagonal_cells())
}

fn generated_value() -> (GeometryDefinitionV1, SimplicialMeshEnvelopeV1, Value) {
    let geometry = square_with_hole(Vec::new());
    let mesh = forward_mesh();
    let correspondence =
        GeometryMeshCorrespondenceEnvelopeV1::from_region(&geometry, &mesh).unwrap();
    let value = serde_json::from_slice(&correspondence.canonical_json().unwrap()).unwrap();
    (geometry, mesh, value)
}

fn decode(value: &Value) -> GeometryMeshCorrespondenceEnvelopeV1 {
    GeometryMeshCorrespondenceEnvelopeV1::from_json(
        &serde_json::to_vec(value).unwrap(),
        Default::default(),
    )
    .expect("mutated fixture remains locally canonical")
}

fn frontiers(value: &mut Value) -> &mut Vec<Value> {
    value["frontiers"].as_array_mut().unwrap()
}

fn sort_frontiers(value: &mut Value) {
    frontiers(value).sort_by_key(|entry| {
        (
            entry["parent_face"].as_u64().unwrap(),
            entry["geometry_edge"].as_u64().unwrap(),
        )
    });
}

#[test]
fn generated_correspondence_is_total_and_exact_for_every_entity_set() {
    let geometry = square_with_hole(Vec::new());
    let mesh = forward_mesh();
    let correspondence =
        GeometryMeshCorrespondenceEnvelopeV1::from_region(&geometry, &mesh).unwrap();
    correspondence
        .validate_against_region(&geometry, &mesh)
        .unwrap();

    assert_eq!(
        correspondence
            .region_entity_set_entities(&geometry, "fluid")
            .unwrap()
            .len(),
        24
    );
    assert_eq!(
        correspondence
            .region_entity_set_entities(&geometry, "exterior")
            .unwrap()
            .len(),
        16
    );
    assert_eq!(
        correspondence
            .region_entity_set_entities(&geometry, "hole")
            .unwrap()
            .len(),
        8
    );
    assert_eq!(
        correspondence
            .region_entity_set_entities(&geometry, "corners")
            .unwrap()
            .len(),
        8
    );
}

#[test]
fn shared_facet_is_mapped_once_for_each_parent_face() {
    let region = PlanarRegion::new(
        vec![
            [0.0, 0.0],
            [1.0, 0.0],
            [1.0, 1.0],
            [0.0, 1.0],
            [2.0, 0.0],
            [2.0, 1.0],
        ],
        vec![
            PlanarFace::new(vec![0, 1, 2, 3], Vec::new()),
            PlanarFace::new(vec![1, 4, 5, 2], Vec::new()),
        ],
        vec![
            NamedEntitySet::new("all-vertices", VERTEX_DIMENSION, (0..6).collect()),
            NamedEntitySet::new("all-edges", EDGE_DIMENSION, (0..8).collect()),
            NamedEntitySet::new("all-faces", FACE_DIMENSION, vec![0, 1]),
        ],
        1.0e-9,
    )
    .unwrap();
    let geometry = GeometryDefinitionV1::from_region(&region);
    let mesh = mesh_artifact(
        vec![
            vec![0.0, 0.0],
            vec![1.0, 0.0],
            vec![1.0, 1.0],
            vec![0.0, 1.0],
            vec![2.0, 0.0],
            vec![2.0, 1.0],
        ],
        vec![vec![0, 1, 2], vec![0, 2, 3], vec![1, 4, 5], vec![1, 5, 2]],
    );
    let correspondence =
        GeometryMeshCorrespondenceEnvelopeV1::from_region(&geometry, &mesh).unwrap();
    let value: Value = serde_json::from_slice(&correspondence.canonical_json().unwrap()).unwrap();
    let mut uses = std::collections::BTreeMap::new();
    for frontier in value["frontiers"].as_array().unwrap() {
        for facet in frontier["facet_indices"].as_array().unwrap() {
            *uses.entry(facet.as_u64().unwrap()).or_insert(0) += 1;
        }
    }

    assert_eq!(uses.values().filter(|&&count| count == 2).count(), 1);
    assert_eq!(
        correspondence
            .region_entity_set_entities(&geometry, "all-edges")
            .unwrap()
            .len(),
        7
    );
}

#[test]
fn duplicated_coincident_interface_facets_are_not_conforming() {
    let region = PlanarRegion::new(
        vec![
            [0.0, 0.0],
            [1.0, 0.0],
            [1.0, 1.0],
            [0.0, 1.0],
            [2.0, 0.0],
            [2.0, 1.0],
        ],
        vec![
            PlanarFace::new(vec![0, 1, 2, 3], Vec::new()),
            PlanarFace::new(vec![1, 4, 5, 2], Vec::new()),
        ],
        vec![NamedEntitySet::new("all-faces", FACE_DIMENSION, vec![0, 1])],
        1.0e-9,
    )
    .unwrap();
    let geometry = GeometryDefinitionV1::from_region(&region);
    let mesh = mesh_artifact(
        vec![
            vec![0.0, 0.0],
            vec![1.0, 0.0],
            vec![1.0, 1.0],
            vec![0.0, 1.0],
            vec![2.0, 0.0],
            vec![2.0, 1.0],
            vec![1.0, 0.5],
            vec![1.0, 0.5],
        ],
        vec![
            vec![0, 1, 6],
            vec![0, 6, 2],
            vec![0, 2, 3],
            vec![1, 4, 7],
            vec![4, 5, 7],
            vec![5, 2, 7],
        ],
    );

    let error = GeometryMeshCorrespondenceEnvelopeV1::from_region(&geometry, &mesh)
        .unwrap_err()
        .message()
        .to_owned();
    assert!(error.contains("identical shared mesh-facet incidence"));
}

#[test]
fn removed_frontier_facet_is_rejected() {
    let (geometry, mesh, mut value) = generated_value();
    let frontier = &mut frontiers(&mut value)[0];
    frontier["facet_indices"].as_array_mut().unwrap().pop();
    frontier["parent_outward"].as_array_mut().unwrap().pop();
    let correspondence = decode(&value);

    assert!(
        correspondence
            .validate_against_region(&geometry, &mesh)
            .is_err()
    );
}

#[test]
fn facet_relabelled_to_a_different_geometry_edge_is_rejected() {
    let (geometry, mesh, mut value) = generated_value();
    let assignments = frontiers(&mut value);
    let facet = assignments[0]["facet_indices"]
        .as_array_mut()
        .unwrap()
        .pop()
        .unwrap();
    let orientation = assignments[0]["parent_outward"]
        .as_array_mut()
        .unwrap()
        .pop()
        .unwrap();
    let target_facets = assignments[1]["facet_indices"].as_array_mut().unwrap();
    let insertion = target_facets.partition_point(|candidate| candidate.as_u64() < facet.as_u64());
    target_facets.insert(insertion, facet);
    assignments[1]["parent_outward"]
        .as_array_mut()
        .unwrap()
        .insert(insertion, orientation);
    let correspondence = decode(&value);

    assert!(
        correspondence
            .validate_against_region(&geometry, &mesh)
            .is_err()
    );
}

#[test]
fn complete_exterior_and_hole_identity_swap_is_rejected() {
    let (geometry, mesh, mut value) = generated_value();
    for pair in 0..4 {
        let assignments = frontiers(&mut value);
        let outer = assignments
            .iter()
            .position(|entry| entry["geometry_edge"].as_u64() == Some(pair))
            .unwrap();
        let hole = assignments
            .iter()
            .position(|entry| entry["geometry_edge"].as_u64() == Some(pair + 4))
            .unwrap();
        assignments[outer]["geometry_edge"] = Value::from(pair + 4);
        assignments[hole]["geometry_edge"] = Value::from(pair);
    }
    sort_frontiers(&mut value);
    let correspondence = decode(&value);

    assert!(
        correspondence
            .validate_against_region(&geometry, &mesh)
            .is_err()
    );
}

#[test]
fn mesh_whose_boundary_fills_the_authored_hole_is_rejected() {
    let geometry = square_with_hole(Vec::new());
    let mesh = mesh_artifact(
        vec![
            vec![0.0, 0.0],
            vec![0.25, 0.0],
            vec![0.5, 0.0],
            vec![0.75, 0.0],
            vec![1.0, 0.0],
            vec![0.0, 0.25],
            vec![0.25, 0.25],
            vec![0.5, 0.25],
            vec![0.75, 0.25],
            vec![1.0, 0.25],
            vec![0.0, 0.5],
            vec![0.25, 0.5],
            vec![0.5, 0.5],
            vec![0.75, 0.5],
            vec![1.0, 0.5],
            vec![0.0, 0.75],
            vec![0.25, 0.75],
            vec![0.5, 0.75],
            vec![0.75, 0.75],
            vec![1.0, 0.75],
            vec![0.0, 1.0],
            vec![0.25, 1.0],
            vec![0.5, 1.0],
            vec![0.75, 1.0],
            vec![1.0, 1.0],
        ],
        vec![
            vec![0, 1, 6],
            vec![0, 6, 5],
            vec![1, 2, 7],
            vec![1, 7, 6],
            vec![2, 3, 8],
            vec![2, 8, 7],
            vec![3, 4, 9],
            vec![3, 9, 8],
            vec![5, 6, 11],
            vec![5, 11, 10],
            vec![6, 7, 12],
            vec![6, 12, 11],
            vec![7, 8, 13],
            vec![7, 13, 12],
            vec![8, 9, 14],
            vec![8, 14, 13],
            vec![10, 11, 16],
            vec![10, 16, 15],
            vec![11, 12, 17],
            vec![11, 17, 16],
            vec![12, 13, 18],
            vec![12, 18, 17],
            vec![13, 14, 19],
            vec![13, 19, 18],
            vec![15, 16, 21],
            vec![15, 21, 20],
            vec![16, 17, 22],
            vec![16, 22, 21],
            vec![17, 18, 23],
            vec![17, 23, 22],
            vec![18, 19, 24],
            vec![18, 24, 23],
        ],
    );

    let error = GeometryMeshCorrespondenceEnvelopeV1::from_region(&geometry, &mesh)
        .unwrap_err()
        .message()
        .to_owned();
    assert!(error.contains("must be owned by exactly one region face"));
    assert!(error.contains("found 0"));
}

#[test]
fn cell_not_owned_by_any_region_face_is_rejected() {
    let geometry = square_with_hole(Vec::new());
    let mut vertices = grid_vertices();
    vertices.extend([vec![1.5, 0.0], vec![2.0, 0.0], vec![1.5, 0.5]]);
    let mut cells = forward_diagonal_cells();
    cells.push(vec![24, 25, 26]);
    let mesh = mesh_artifact(vertices, cells);

    let error = GeometryMeshCorrespondenceEnvelopeV1::from_region(&geometry, &mesh)
        .unwrap_err()
        .message()
        .to_owned();
    assert!(error.contains("cell 24"));
    assert!(error.contains("found 0"));
}

#[test]
fn reversed_parent_outward_facet_orientation_is_rejected() {
    let (geometry, mesh, mut value) = generated_value();
    let orientation = &mut frontiers(&mut value)[0]["parent_outward"]
        .as_array_mut()
        .unwrap()[0];
    *orientation = match orientation.as_str().unwrap() {
        "left-of-canonical-facet" => Value::from("right-of-canonical-facet"),
        "right-of-canonical-facet" => Value::from("left-of-canonical-facet"),
        other => panic!("unexpected orientation {other}"),
    };
    let correspondence = decode(&value);

    assert!(
        correspondence
            .validate_against_region(&geometry, &mesh)
            .is_err()
    );
}

#[test]
fn two_triangulations_change_mesh_and_correspondence_but_not_region_identity() {
    let geometry = square_with_hole(Vec::new());
    let first_mesh = forward_mesh();
    let second_mesh = reverse_mesh();
    let first = GeometryMeshCorrespondenceEnvelopeV1::from_region(&geometry, &first_mesh).unwrap();
    let second =
        GeometryMeshCorrespondenceEnvelopeV1::from_region(&geometry, &second_mesh).unwrap();

    assert_eq!(
        first.geometry_artifact(),
        GeometryDefinitionV1::from_region(&geometry.region().unwrap())
            .digest()
            .unwrap()
    );
    assert_eq!(first.geometry_artifact(), second.geometry_artifact());
    assert_ne!(first_mesh.digest().unwrap(), second_mesh.digest().unwrap());
    assert_ne!(first.digest().unwrap(), second.digest().unwrap());
}

#[test]
fn facet_ambiguity_names_both_region_entity_sets() {
    let geometry = square_with_hole(vec![NamedEntitySet::new(
        "outer-copy",
        EDGE_DIMENSION,
        vec![0],
    )]);
    let error = GeometryMeshCorrespondenceEnvelopeV1::from_region(&geometry, &forward_mesh())
        .unwrap_err()
        .message()
        .to_owned();

    assert!(error.contains("exterior"));
    assert!(error.contains("outer-copy"));
}
