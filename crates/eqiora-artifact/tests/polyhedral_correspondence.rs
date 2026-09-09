//! Exact source outer bodies and independently specified tetrahedral refinements.

use eqiora_artifact::{
    GeometryDefinitionV1, GeometryMeshCorrespondenceEnvelopeV1, SimplicialMeshEnvelopeV1,
};
use eqiora_geometry::{CanonicalGeometryV1, NamedEntitySet};
use eqiora_meshing::{MeshEntity, MeshQualityGate, SimplicialMesh};

fn geometry(tetra: bool) -> GeometryDefinitionV1 {
    let (vertices, shells, interface, left_exterior, right_exterior) = if tetra {
        (
            vec![
                [-1., 0., 0.],
                [0., 0., 0.],
                [0., 0., 1.],
                [0., 1., 0.],
                [1., 0., 0.],
            ],
            vec![
                vec![vec![1, 3, 2], vec![0, 3, 1], vec![0, 2, 3], vec![0, 1, 2]],
                vec![vec![1, 2, 3], vec![4, 1, 3], vec![4, 3, 2], vec![4, 2, 1]],
            ],
            3,
            vec![0, 1, 2],
            vec![4, 5, 6],
        )
    } else {
        (
            vec![
                [-1., 0., 0.],
                [0., -1., 0.],
                [0., 0., -1.],
                [0., 0., 1.],
                [0., 1., 0.],
                [1., 0., 0.],
            ],
            vec![
                vec![
                    vec![1, 2, 4, 3],
                    vec![0, 2, 1],
                    vec![0, 4, 2],
                    vec![0, 3, 4],
                    vec![0, 1, 3],
                ],
                vec![
                    vec![1, 3, 4, 2],
                    vec![5, 1, 2],
                    vec![5, 2, 4],
                    vec![5, 4, 3],
                    vec![5, 3, 1],
                ],
            ],
            4,
            vec![0, 1, 2, 3],
            vec![5, 6, 7, 8],
        )
    };
    let geometry = CanonicalGeometryV1::from_convex_polyhedra(
        vertices,
        shells,
        vec![
            NamedEntitySet::new("left", 3, vec![0]),
            NamedEntitySet::new("right", 3, vec![1]),
            NamedEntitySet::new("both", 3, vec![0, 1]),
            NamedEntitySet::new("interface", 2, vec![interface]),
            NamedEntitySet::new("left_exterior", 2, left_exterior),
            NamedEntitySet::new("right_exterior", 2, right_exterior),
            NamedEntitySet::new("vertex", 0, vec![0]),
            NamedEntitySet::new("edge", 1, vec![0]),
        ],
        1e-12,
    )
    .unwrap();
    GeometryDefinitionV1::from_canonical(&geometry).unwrap()
}
fn mesh(vertices: Vec<Vec<f64>>, mut cells: Vec<Vec<usize>>) -> SimplicialMeshEnvelopeV1 {
    for cell in &mut cells {
        let p = cell.iter().map(|&i| &vertices[i]).collect::<Vec<_>>();
        let v = |i: usize, j: usize| p[i][j] - p[0][j];
        let determinant = v(1, 0) * (v(2, 1) * v(3, 2) - v(2, 2) * v(3, 1))
            - v(1, 1) * (v(2, 0) * v(3, 2) - v(2, 2) * v(3, 0))
            + v(1, 2) * (v(2, 0) * v(3, 1) - v(2, 1) * v(3, 0));
        if determinant < 0. {
            cell.swap(1, 2);
        }
    }
    SimplicialMeshEnvelopeV1::from_mesh(
        &SimplicialMesh::new(3, vertices, cells, MeshQualityGate::new(1e-9).unwrap()).unwrap(),
    )
    .unwrap()
}
fn bipyramid_mesh() -> SimplicialMeshEnvelopeV1 {
    let vertices = vec![
        vec![-1., 0., 0.],
        vec![1., 0., 0.],
        vec![0., 0., 0.],
        vec![0., 1., 0.],
        vec![0., 0., 1.],
        vec![0., -1., 0.],
        vec![0., 0., -1.],
    ];
    let ring = [3, 4, 5, 6];
    let cells = (0..2)
        .flat_map(|apex| (0..4).map(move |i| vec![apex, 2, ring[i], ring[(i + 1) % 4]]))
        .collect();
    mesh(vertices, cells)
}
fn tetrahedral_mesh(refined: bool) -> SimplicialMeshEnvelopeV1 {
    if !refined {
        return mesh(
            vec![
                vec![0., 0., 0.],
                vec![0., 1., 0.],
                vec![0., 0., 1.],
                vec![-1., 0., 0.],
                vec![1., 0., 0.],
            ],
            vec![vec![3, 0, 1, 2], vec![4, 0, 1, 2]],
        );
    }
    mesh(
        vec![
            vec![0., 0., 0.],
            vec![0., 1., 0.],
            vec![0., 0., 1.],
            vec![-1., 0., 0.],
            vec![-0.25, 0.25, 0.25],
            vec![0., 1. / 3., 1. / 3.],
            vec![1., 0., 0.],
        ],
        vec![
            vec![4, 5, 0, 1],
            vec![4, 5, 1, 2],
            vec![4, 5, 2, 0],
            vec![4, 3, 1, 2],
            vec![4, 3, 2, 0],
            vec![4, 3, 0, 1],
            vec![6, 5, 0, 1],
            vec![6, 5, 1, 2],
            vec![6, 5, 2, 0],
        ],
    )
}
fn entities(
    c: &GeometryMeshCorrespondenceEnvelopeV1,
    g: &GeometryDefinitionV1,
    name: &str,
) -> Vec<usize> {
    c.polyhedral_entity_set_entities(g, name)
        .unwrap()
        .into_iter()
        .map(|e| e.index())
        .collect()
}
fn closures(mesh: &SimplicialMeshEnvelopeV1, dimension: usize, ids: &[usize]) -> Vec<Vec<usize>> {
    let mut result = ids
        .iter()
        .map(|&id| {
            mesh.mesh()
                .entity_vertices(MeshEntity::new(dimension, id))
                .unwrap()
                .into_iter()
                .map(|v| v.index())
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    for vertices in &mut result {
        vertices.sort_unstable();
    }
    result.sort();
    result
}

#[test]
fn bipyramid_and_refined_tetrahedra_keep_exact_outer_entities_and_shared_incidence() {
    for (tetra, mesh, left, right, triangles) in [
        (
            false,
            bipyramid_mesh(),
            vec![0, 1, 2, 3],
            vec![4, 5, 6, 7],
            vec![vec![2, 3, 4], vec![2, 3, 6], vec![2, 4, 5], vec![2, 5, 6]],
        ),
        (
            true,
            tetrahedral_mesh(true),
            vec![0, 1, 2, 3, 4, 5],
            vec![6, 7, 8],
            vec![vec![0, 1, 5], vec![0, 2, 5], vec![1, 2, 5]],
        ),
    ] {
        let geometry = geometry(tetra);
        let c = GeometryMeshCorrespondenceEnvelopeV1::from_polyhedra(&geometry, &mesh).unwrap();
        assert_eq!(entities(&c, &geometry, "left"), left);
        assert_eq!(entities(&c, &geometry, "right"), right);
        assert_eq!(
            closures(&mesh, 2, &entities(&c, &geometry, "interface")),
            triangles
        );
        assert_eq!(
            entities(&c, &geometry, "both").len(),
            mesh.mesh().cells().len()
        );
        assert_eq!(
            geometry.canonical().polyhedral_vertices().unwrap().len(),
            if tetra { 5 } else { 6 }
        );
        assert_eq!(
            entities(&c, &geometry, "vertex"),
            if tetra { vec![3] } else { vec![0] }
        );
        assert_eq!(
            closures(&mesh, 1, &entities(&c, &geometry, "edge")),
            if tetra {
                vec![vec![0, 3]]
            } else {
                vec![vec![0, 5]]
            }
        );
        let wire: serde_json::Value = serde_json::from_slice(&c.canonical_json().unwrap()).unwrap();
        assert_eq!(wire["source"], "convex-polyhedra-tetrahedra-v1");
        let frontier = if tetra { 3 } else { 4 };
        let rows = wire["frontiers"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|r| r["geometry_facet"] == frontier)
            .collect::<Vec<_>>();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0]["facet_indices"], rows[1]["facet_indices"]);
        for (a, b) in rows[0]["parent_outward"]
            .as_array()
            .unwrap()
            .iter()
            .zip(rows[1]["parent_outward"].as_array().unwrap())
        {
            assert_ne!(a, b);
        }
        let replay_g = GeometryDefinitionV1::from_json(
            &geometry.canonical_json().unwrap(),
            Default::default(),
        )
        .unwrap();
        let replay_m = SimplicialMeshEnvelopeV1::from_json(
            &mesh.canonical_json().unwrap(),
            Default::default(),
        )
        .unwrap();
        let replay_c = GeometryMeshCorrespondenceEnvelopeV1::from_json(
            &c.canonical_json().unwrap(),
            Default::default(),
        )
        .unwrap();
        replay_c
            .validate_against_polyhedra(&replay_g, &replay_m)
            .unwrap();
        assert_eq!(replay_c.digest().unwrap(), c.digest().unwrap());
        assert_eq!(replay_g.digest().unwrap(), geometry.digest().unwrap());
        assert!(replay_g.region().is_err());
    }
}

#[test]
fn interior_and_interface_refinement_changes_mesh_not_geometry_identity() {
    let geometry = geometry(true);
    let coarse = tetrahedral_mesh(false);
    let fine = tetrahedral_mesh(true);
    let a = GeometryMeshCorrespondenceEnvelopeV1::from_polyhedra(&geometry, &coarse).unwrap();
    let b = GeometryMeshCorrespondenceEnvelopeV1::from_polyhedra(&geometry, &fine).unwrap();
    assert_eq!(a.geometry_artifact(), b.geometry_artifact());
    assert_ne!(a.mesh_artifact(), b.mesh_artifact());
    assert_ne!(a.digest().unwrap(), b.digest().unwrap());
    assert_eq!(entities(&a, &geometry, "interface").len(), 1);
    assert_eq!(entities(&b, &geometry, "interface").len(), 3);
    assert!(
        a.validate_against_polyhedra(&geometry, &fine)
            .unwrap_err()
            .message()
            .contains("digests")
    );
    assert!(
        b.validate_against_polyhedra(&geometry, &coarse)
            .unwrap_err()
            .message()
            .contains("digests")
    );
}

#[test]
fn hostile_correspondence_frontier_orientation_membership_and_digest_fail_closed() {
    let geometry = geometry(false);
    let mesh = bipyramid_mesh();
    let c = GeometryMeshCorrespondenceEnvelopeV1::from_polyhedra(&geometry, &mesh).unwrap();
    let original: serde_json::Value = serde_json::from_slice(&c.canonical_json().unwrap()).unwrap();
    for mutation in 0..6 {
        let mut wire = original.clone();
        match mutation {
            0 => {
                wire["frontiers"].as_array_mut().unwrap().remove(0);
            }
            1 => {
                let row = wire["frontiers"][0].clone();
                wire["frontiers"].as_array_mut().unwrap().insert(0, row);
            }
            2 => {
                let orientation = wire["frontiers"][0]["parent_outward"][0].as_str().unwrap();
                wire["frontiers"][0]["parent_outward"][0] =
                    serde_json::json!(if orientation == "along-canonical-facet-normal" {
                        "against-canonical-facet-normal"
                    } else {
                        "along-canonical-facet-normal"
                    });
            }
            3 => wire["geometry_sha256"] = serde_json::json!("00".repeat(32)),
            4 => wire["mesh_sha256"] = serde_json::json!("11".repeat(32)),
            _ => wire["volumes"][0]["mesh_entities"][0] = serde_json::json!(999),
        }
        let error = match GeometryMeshCorrespondenceEnvelopeV1::from_json(
            &serde_json::to_vec(&wire).unwrap(),
            Default::default(),
        ) {
            Err(error) => error,
            Ok(decoded) => decoded
                .validate_against_polyhedra(&geometry, &mesh)
                .unwrap_err(),
        };
        assert!(
            error.message().contains("polyhedral"),
            "{mutation}: {error:?}"
        );
    }
    let foreign = self::geometry(true);
    assert!(
        c.validate_against_polyhedra(&foreign, &mesh)
            .unwrap_err()
            .message()
            .contains("digests")
    );
}

#[test]
fn a_missing_tetrahedron_and_nonconforming_shared_mesh_interface_reject() {
    let geometry = geometry(true);
    let complete = tetrahedral_mesh(true);
    let mut cells = complete.mesh().cells().to_vec();
    cells.remove(0);
    let missing = mesh(complete.mesh().vertices().to_vec(), cells);
    let error =
        GeometryMeshCorrespondenceEnvelopeV1::from_polyhedra(&geometry, &missing).unwrap_err();
    assert!(
        error.message().contains("complete exact polyhedral volume"),
        "{error:?}"
    );
    // Two geometrically coincident interface triangulations use different mesh
    // vertex identities. Native incidence does not connect them.
    let disconnected = mesh(
        vec![
            vec![0., 0., 0.],
            vec![0., 1., 0.],
            vec![0., 0., 1.],
            vec![-1., 0., 0.],
            vec![1., 0., 0.],
            vec![0., 0., 0.],
            vec![0., 1., 0.],
            vec![0., 0., 1.],
        ],
        vec![vec![3, 0, 1, 2], vec![4, 5, 6, 7]],
    );
    let error =
        GeometryMeshCorrespondenceEnvelopeV1::from_polyhedra(&geometry, &disconnected).unwrap_err();
    assert!(
        error.message().contains("one unique exact mesh vertex"),
        "{error:?}"
    );
}

#[test]
fn overlapping_cells_and_subprecision_cross_parent_cells_reject_at_exact_assignment() {
    let geometry = geometry(false);
    let original = bipyramid_mesh();
    let mut vertices = original.mesh().vertices().to_vec();
    let mut cells = original.mesh().cells().to_vec();
    let start = vertices.len();
    vertices.extend([
        vec![-0.5, 0., 0.],
        vec![-0.5, 0.125, 0.],
        vec![-0.5, 0., 0.125],
        vec![-0.375, 0., 0.],
    ]);
    cells.push((start..start + 4).collect());
    let overlapping = mesh(vertices, cells);
    let error =
        GeometryMeshCorrespondenceEnvelopeV1::from_polyhedra(&geometry, &overlapping).unwrap_err();
    assert!(
        error.message().contains("tetrahedron interiors overlap"),
        "{error:?}"
    );
    let mut crossed = original.mesh().vertices().to_vec();
    crossed[2][0] = 2f64.powi(-45);
    let crossed = mesh(crossed, original.mesh().cells().to_vec());
    let error =
        GeometryMeshCorrespondenceEnvelopeV1::from_polyhedra(&geometry, &crossed).unwrap_err();
    assert!(error.message().contains("belong uniquely"), "{error:?}");
}

#[test]
fn selected_geometry_model_and_artifact_replay_retain_exact_polyhedral_parents() {
    use eqiora_artifact::AcceptedModelArtifact;
    use eqiora_compiler::{CompiledModel, StaticBindingValue};
    use eqiora_graph::{GraphStore, InMemoryGraphStore};
    use eqiora_sem::KernelProgram;
    const SOURCE: &str = r#"
public connector MechanicalBoundary {
  trace displacement: m;
  flux traction: kg * m ^ -1 * s ^ -2;
  shape spatial_vector;
  frame spatial;
  pairing euclidean_boundary_duality;
  orientation parent_outward;
}
public component Terminal(
  support body: volume(ambient_dimension = 3),
  support face: boundary(parent = body),
  port mechanical: MechanicalBoundary over face,
) {
  relation law on face {
    mechanical.displacement - mechanical.displacement = 0;
    mechanical.traction - mechanical.traction = 0;
  }
}
model Main(
  support left: volume(ambient_dimension = 3),
  support right: volume(ambient_dimension = 3),
  support left_face: boundary(parent = left),
  support right_face: boundary(parent = right),
) {
  instance a: Terminal(body = left, face = left_face);
  instance b: Terminal(body = right, face = right_face);
  connect a.mechanical, b.mechanical;
}
"#;
    for tetra in [false, true] {
        let definition = geometry(tetra);
        let geometry = definition.canonical();
        let bindings = [
            ("left", "left", None),
            ("right", "right", None),
            ("left_face", "interface", Some("left")),
            ("right_face", "interface", Some("right")),
        ]
        .map(|(slot, name, parent)| {
            (
                slot,
                StaticBindingValue::GeometrySupport {
                    geometry,
                    selection: geometry.entity_set(name).unwrap(),
                    parent: parent.map(|p| geometry.entity_set(p).unwrap()),
                },
            )
        });
        let compiled =
            CompiledModel::compile_selected("polyhedral.eqi", SOURCE, "Main", &bindings).unwrap();
        let (transaction, model, _) = compiled.into_parts();
        let mut store = InMemoryGraphStore::new();
        store.commit(transaction).unwrap();
        assert!(KernelProgram::from_snapshot(&store.snapshot(), model).is_err());
        let program =
            KernelProgram::from_snapshot_with_geometry(&store.snapshot(), model, &[geometry])
                .unwrap();
        let artifact = AcceptedModelArtifact::from_program(&program).unwrap();
        let decoded = AcceptedModelArtifact::from_json(
            &artifact.canonical_json().unwrap(),
            Default::default(),
        )
        .unwrap();
        let replay_geometry = GeometryDefinitionV1::from_json(
            &definition.canonical_json().unwrap(),
            Default::default(),
        )
        .unwrap();
        let (transaction, model) = decoded.to_transaction().unwrap();
        let mut restored = InMemoryGraphStore::new();
        restored.commit(transaction).unwrap();
        let replay = KernelProgram::from_snapshot_with_geometry(
            &restored.snapshot(),
            model,
            &[replay_geometry.canonical()],
        )
        .unwrap();
        assert_eq!(replay, program);
        let other = self::geometry(!tetra);
        assert!(
            KernelProgram::from_snapshot_with_geometry(
                &restored.snapshot(),
                model,
                &[other.canonical()]
            )
            .is_err()
        );
    }
}
