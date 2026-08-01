use std::collections::{BTreeMap, BTreeSet};

use eqiora::api::ModelDocument;
use eqiora::artifact::{
    GeometryIdentityEnvelopeV1, GeometryMeshCorrespondenceEnvelopeV1, ModelEnvelope,
    SimplicialMeshEnvelopeV1,
};
use eqiora::diagnostic::codes;
use eqiora::geometry::{
    AxisAlignedBox3, CadAuthoredFaceMesh, CadAuthoredGraph, CadAuthoredSweptMesh,
    ConstrainedRectangleV1,
};
use eqiora::meshing::{MeshEntity, MeshQualityGate, MeshTopology, SimplicialMesh};
use eqiora::{Id, kinds};
use num_rational::BigRational;

const MODEL: &str =
    include_str!("../../../verify/geometry/cad-authored-surface-sweep/models/box.eqi");
const GEOMETRY_TOLERANCE_M: f64 = 5.0e-10;

fn graph() -> CadAuthoredGraph {
    CadAuthoredGraph::new(
        ConstrainedRectangleV1::new((-2.0, 3.0), (-1.0, 2.0), 0.5).unwrap(),
        4.0,
        1.0e-9,
    )
    .unwrap()
}

fn quality(minimum: f64) -> MeshQualityGate {
    MeshQualityGate::new(minimum).unwrap()
}

fn surface(graph: &CadAuthoredGraph, provenance: &str) -> CadAuthoredFaceMesh {
    CadAuthoredFaceMesh::from_face(
        graph,
        &graph.face_handle(provenance).unwrap(),
        GEOMETRY_TOLERANCE_M,
        2.0,
        24,
        quality(0.95),
    )
    .unwrap()
}

fn sweep(
    graph: &CadAuthoredGraph,
    source: &CadAuthoredFaceMesh,
    minimum_quality: f64,
) -> CadAuthoredSweptMesh {
    CadAuthoredSweptMesh::through_body(source, graph, 2, 3.0, 144, quality(minimum_quality))
        .unwrap()
}

fn domain(document: &ModelDocument, name: &str) -> Id<kinds::Domain> {
    document.aliases()[name].downcast().unwrap()
}

fn model_geometry() -> (ModelDocument, ModelEnvelope, GeometryIdentityEnvelopeV1) {
    let document = ModelDocument::compile("box.eqi", MODEL).unwrap();
    let model = ModelEnvelope::from_program(document.program()).unwrap();
    let geometry =
        GeometryIdentityEnvelopeV1::new(&model, [domain(&document, "body")], GEOMETRY_TOLERANCE_M)
            .unwrap();
    (document, model, geometry)
}

fn assert_complete_correspondence(mesh: &SimplicialMesh, expected: [usize; 6]) {
    let (document, model, geometry) = model_geometry();
    let mesh = SimplicialMeshEnvelopeV1::from_mesh(mesh).unwrap();
    let correspondence = GeometryMeshCorrespondenceEnvelopeV1::new(&geometry, &model, &mesh)
        .expect("the sweep must close the existing generic correspondence");
    assert_eq!(
        correspondence
            .body_cells(domain(&document, "body"))
            .unwrap(),
        (0..mesh.mesh().cells().len()).collect::<Vec<_>>()
    );

    let names = [
        "x_lower", "x_upper", "y_lower", "y_upper", "z_lower", "z_upper",
    ];
    let boundary_sets = names
        .into_iter()
        .zip(expected)
        .map(|(name, count)| {
            let facets = correspondence
                .boundary_facets(domain(&document, name))
                .unwrap();
            assert_eq!(facets.len(), count, "boundary inventory for {name}");
            facets.into_iter().collect::<BTreeSet<_>>()
        })
        .collect::<Vec<_>>();
    for left in 0..boundary_sets.len() {
        for right in left + 1..boundary_sets.len() {
            assert!(boundary_sets[left].is_disjoint(&boundary_sets[right]));
        }
    }
    assert_eq!(
        boundary_sets
            .iter()
            .flat_map(BTreeSet::iter)
            .copied()
            .collect::<BTreeSet<_>>()
            .len(),
        expected.into_iter().sum::<usize>()
    );
}

fn boundary_facet_count(mesh: &SimplicialMesh) -> usize {
    (0..mesh.entity_count(2).unwrap())
        .filter(|&facet| mesh.is_boundary_entity(MeshEntity::new(2, facet)) == Some(true))
        .count()
}

fn exact_point(point: &[f64]) -> [BigRational; 3] {
    std::array::from_fn(|axis| {
        BigRational::from_float(point[axis])
            .expect("an accepted finite mesh coordinate has an exact dyadic image")
    })
}

fn exact_cell_determinant(mesh: &SimplicialMesh, cell: &[usize]) -> BigRational {
    let [i0, i1, i2, i3] = cell else {
        panic!("the accepted sweep contains only tetrahedra")
    };
    let [p0, p1, p2, p3] = [i0, i1, i2, i3].map(|&i| exact_point(&mesh.vertices()[i]));
    let a: [BigRational; 3] = std::array::from_fn(|axis| &p1[axis] - &p0[axis]);
    let b: [BigRational; 3] = std::array::from_fn(|axis| &p2[axis] - &p0[axis]);
    let c: [BigRational; 3] = std::array::from_fn(|axis| &p3[axis] - &p0[axis]);
    &a[0] * (&b[1] * &c[2] - &b[2] * &c[1]) - &a[1] * (&b[0] * &c[2] - &b[2] * &c[0])
        + &a[2] * (&b[0] * &c[1] - &b[1] * &c[0])
}

fn assert_exact_coverage(mesh: &SimplicialMesh, expected_minimum: &BigRational) {
    let zero = BigRational::from_integer(0.into());
    let determinants = mesh
        .cells()
        .iter()
        .map(|cell| exact_cell_determinant(mesh, cell))
        .collect::<Vec<_>>();
    assert!(determinants.iter().all(|determinant| determinant > &zero));
    assert_eq!(determinants.iter().min(), Some(expected_minimum));

    let sum = determinants
        .iter()
        .cloned()
        .fold(zero.clone(), |sum, determinant| sum + determinant);
    let reversed_sum = determinants
        .into_iter()
        .rev()
        .fold(zero, |sum, determinant| sum + determinant);
    assert_eq!(sum, BigRational::from_integer(360.into()));
    assert_eq!(reversed_sum, sum);
    assert_eq!(
        sum / BigRational::from_integer(6.into()),
        BigRational::from_integer(60.into())
    );
}

#[test]
fn dual_oracle_end_cap_freezes_layers_topology_quality_and_correspondence() {
    let graph = graph();
    let source = surface(&graph, "end-cap");
    let realization = sweep(&graph, &source, 0.23);

    assert_eq!(realization.source_surface(), &source);
    assert_eq!(realization.target_graph(), &graph);
    assert_eq!(
        realization.target_body_bounds(),
        AxisAlignedBox3::new([(-2.0, 3.0), (-1.0, 2.0), (0.5, 4.5)]).unwrap()
    );
    assert_eq!(realization.inward_direction(), [0.0, 0.0, -1.0]);
    assert_eq!(realization.normal_axis(), 2);
    assert_eq!(realization.sweep_distance_m(), 4.0);
    assert_eq!(realization.layers(), 2);
    assert_eq!(realization.growth_rate(), 3.0);
    assert_eq!(realization.layer_offsets_m(), &[0.0, 1.0, 4.0]);
    assert_eq!(realization.maximum_tetrahedra(), 144);

    let mesh = realization.mesh();
    let vertex_count = mesh.vertices().len();
    let edge_count = mesh.entity_count(1).expect("edges must be reported");
    let facet_count = mesh.entity_count(2).expect("facets must be reported");
    let cell_count = mesh.cells().len();
    assert_eq!(vertex_count, 60);
    assert_eq!(edge_count, 255);
    assert_eq!(facet_count, 340);
    assert_eq!(cell_count, 144);
    assert_eq!(boundary_facet_count(mesh), 104);
    assert_eq!(
        vertex_count as i64 - edge_count as i64 + facet_count as i64 - cell_count as i64,
        1
    );
    let minimum = BigRational::new(5.into(), 4.into());
    assert_exact_coverage(mesh, &minimum);
    let determinant_histogram = mesh
        .cells()
        .iter()
        .fold(BTreeMap::new(), |mut counts, cell| {
            *counts
                .entry(exact_cell_determinant(mesh, cell))
                .or_insert(0) += 1;
            counts
        });
    assert_eq!(
        determinant_histogram,
        BTreeMap::from([(minimum, 72), (BigRational::new(15.into(), 4.into()), 72),])
    );

    assert_eq!(
        &mesh.cells()[..6],
        &[
            vec![0, 6, 1, 26],
            vec![0, 1, 21, 26],
            vec![0, 21, 20, 26],
            vec![20, 26, 21, 46],
            vec![20, 21, 41, 46],
            vec![20, 41, 40, 46],
        ]
    );
    assert_eq!(
        &mesh.cells()[mesh.cells().len() - 6..],
        &[
            vec![13, 18, 19, 39],
            vec![13, 38, 18, 39],
            vec![13, 33, 38, 39],
            vec![33, 38, 39, 59],
            vec![33, 58, 38, 59],
            vec![33, 53, 58, 59],
        ]
    );
    assert_eq!(mesh.quality_report().minimum_signed_measure_scale(), 1.25);
    assert_eq!(
        mesh.quality_report().minimum_mean_ratio(),
        0.23264804448328424
    );
    assert_complete_correspondence(mesh, [12, 12, 16, 16, 24, 24]);

    let envelope = SimplicialMeshEnvelopeV1::from_mesh(mesh).unwrap();
    let bytes = envelope.canonical_json().unwrap();
    let replay = SimplicialMeshEnvelopeV1::from_json(&bytes, Default::default()).unwrap();
    assert_eq!(replay.canonical_json().unwrap(), bytes);
    assert_eq!(replay.mesh(), mesh);
}

#[test]
fn start_cap_reverses_inward_placement_without_changing_complete_body_counts() {
    let graph = graph();
    let end = sweep(&graph, &surface(&graph, "end-cap"), 0.23);
    let start_source = surface(&graph, "start-cap");
    let start = sweep(&graph, &start_source, 0.23);

    assert_eq!(start.inward_direction(), [0.0, 0.0, 1.0]);
    assert_eq!(start.normal_axis(), 2);
    assert_eq!(start.layer_offsets_m(), &[0.0, 1.0, 4.0]);
    assert_eq!(start.mesh().vertices()[20][2], 1.5);
    assert_eq!(start.mesh().vertices()[40][2], 4.5);
    assert_eq!(end.mesh().vertices()[20][2], 3.5);
    assert_ne!(
        start.source_surface().source_face(),
        end.source_surface().source_face()
    );
    assert_ne!(start.mesh().vertices(), end.mesh().vertices());
    assert_ne!(start.mesh().cells(), end.mesh().cells());

    assert_eq!(start.mesh().vertices().len(), 60);
    assert_eq!(start.mesh().entity_count(1), Some(255));
    assert_eq!(start.mesh().entity_count(2), Some(340));
    assert_eq!(start.mesh().cells().len(), 144);
    assert_eq!(boundary_facet_count(start.mesh()), 104);
    assert_exact_coverage(start.mesh(), &BigRational::new(5.into(), 4.into()));
    assert_eq!(start.mesh().quality_report(), end.mesh().quality_report());
    assert_complete_correspondence(start.mesh(), [12, 12, 16, 16, 24, 24]);

    let start_digest = SimplicialMeshEnvelopeV1::from_mesh(start.mesh())
        .unwrap()
        .digest()
        .unwrap();
    let end_digest = SimplicialMeshEnvelopeV1::from_mesh(end.mesh())
        .unwrap()
        .digest()
        .unwrap();
    assert_ne!(start_digest, end_digest);
}

#[test]
fn all_six_full_faces_use_one_common_sweep_and_generic_correspondence() {
    let graph = graph();
    let mut mesh_digests = BTreeSet::new();
    for provenance in [
        "start-cap",
        "end-cap",
        "profile-x-lower",
        "profile-x-upper",
        "profile-y-lower",
        "profile-y-upper",
    ] {
        let source = surface(&graph, provenance);
        let realization = sweep(&graph, &source, 0.18);
        let (
            normal_axis,
            surface_vertices,
            surface_triangles,
            offsets,
            counts,
            boundary,
            exact_minimum,
        ) = match provenance {
            "profile-x-lower" | "profile-x-upper" => (
                0,
                16,
                18,
                [0.0, 1.25, 5.0],
                [48, 197, 258, 108],
                [18, 18, 12, 12, 12, 12],
                BigRational::new(
                    30_023_997_515_803_305_i64.into(),
                    18_014_398_509_481_984_i64.into(),
                ),
            ),
            "profile-y-lower" | "profile-y-upper" => (
                1,
                20,
                24,
                [0.0, 0.75, 3.0],
                [60, 255, 340, 144],
                [12, 12, 24, 24, 16, 16],
                BigRational::new(
                    90_071_992_547_409_915_i64.into(),
                    72_057_594_037_927_936_i64.into(),
                ),
            ),
            "start-cap" | "end-cap" => (
                2,
                20,
                24,
                [0.0, 1.0, 4.0],
                [60, 255, 340, 144],
                [12, 12, 16, 16, 24, 24],
                BigRational::new(5.into(), 4.into()),
            ),
            _ => unreachable!(),
        };
        assert_eq!(source.mesh().vertices().len(), surface_vertices);
        assert_eq!(source.mesh().cells().len(), surface_triangles);
        assert_eq!(realization.normal_axis(), normal_axis);
        assert_eq!(realization.layer_offsets_m(), offsets);
        assert_eq!(realization.mesh().vertices().len(), counts[0]);
        assert_eq!(realization.mesh().entity_count(1), Some(counts[1]));
        assert_eq!(realization.mesh().entity_count(2), Some(counts[2]));
        assert_eq!(realization.mesh().cells().len(), counts[3]);
        assert_eq!(
            boundary_facet_count(realization.mesh()),
            boundary.into_iter().sum::<usize>()
        );
        assert_exact_coverage(realization.mesh(), &exact_minimum);
        assert!(realization.mesh().quality_report().minimum_mean_ratio() > 0.18);
        assert_complete_correspondence(realization.mesh(), boundary);
        mesh_digests.insert(
            SimplicialMeshEnvelopeV1::from_mesh(realization.mesh())
                .unwrap()
                .digest()
                .unwrap()
                .sha256_bytes(),
        );
    }
    assert_eq!(mesh_digests.len(), 6);
}

#[test]
fn source_request_quality_and_resource_falsifiers_fail_closed() {
    let graph = graph();
    let source = surface(&graph, "end-cap");
    let changed_graph = CadAuthoredGraph::new(graph.sketch(), 4.0, 2.0e-9).unwrap();
    assert_eq!(
        CadAuthoredSweptMesh::through_body(&source, &changed_graph, 2, 3.0, 144, quality(0.23))
            .unwrap_err()
            .code(),
        codes::INVALID_ARTIFACT
    );

    let cut = graph
        .circular_through_cut([0.0, 0.0], 0.25, 1.0e-9)
        .unwrap();
    let cut_surface = surface(&cut, "profile-x-lower");
    assert_eq!(
        CadAuthoredSweptMesh::through_body(&cut_surface, &cut, 2, 3.0, 144, quality(0.18))
            .unwrap_err()
            .code(),
        codes::INVALID_ARTIFACT
    );

    for growth in [0.0, 0.5, f64::NAN, f64::INFINITY] {
        assert_eq!(
            CadAuthoredSweptMesh::through_body(&source, &graph, 2, growth, 144, quality(0.23))
                .unwrap_err()
                .code(),
            codes::INVALID_ARTIFACT
        );
    }
    for (layers, maximum) in [(0, 144), (2, 2), (2, 1_000_001), (2, 143)] {
        assert_eq!(
            CadAuthoredSweptMesh::through_body(
                &source,
                &graph,
                layers,
                3.0,
                maximum,
                quality(0.23),
            )
            .unwrap_err()
            .code(),
            codes::INVALID_ARTIFACT
        );
    }
    assert_eq!(
        CadAuthoredSweptMesh::through_body(
            &source,
            &graph,
            usize::MAX,
            3.0,
            1_000_000,
            quality(0.23),
        )
        .unwrap_err()
        .code(),
        codes::INVALID_ARTIFACT
    );
    assert_eq!(
        CadAuthoredSweptMesh::through_body(&source, &graph, 2_000, 2.0, 1_000_000, quality(0.23),)
            .unwrap_err()
            .code(),
        codes::INVALID_ARTIFACT
    );
    assert_eq!(
        CadAuthoredSweptMesh::through_body(&source, &graph, 2, 3.0, 144, quality(0.24))
            .unwrap_err()
            .code(),
        codes::INVALID_MESH
    );
}

#[test]
fn incomplete_half_depth_and_cavity_meshes_fail_generic_correspondence() {
    let graph = graph();
    let accepted = sweep(&graph, &surface(&graph, "end-cap"), 0.23);
    let mesh = accepted.mesh();
    let (_document, model, geometry) = model_geometry();

    let retained_cells = mesh
        .cells()
        .iter()
        .filter(|cell| cell.iter().all(|&vertex| mesh.vertices()[vertex][2] >= 3.5))
        .cloned()
        .collect::<Vec<_>>();
    let retained_vertices = retained_cells
        .iter()
        .flatten()
        .copied()
        .collect::<BTreeSet<_>>();
    let remap = retained_vertices
        .iter()
        .copied()
        .enumerate()
        .map(|(new, old)| (old, new))
        .collect::<BTreeMap<_, _>>();
    let half_depth = SimplicialMesh::new(
        3,
        retained_vertices
            .iter()
            .map(|&vertex| mesh.vertices()[vertex].clone())
            .collect(),
        retained_cells
            .iter()
            .map(|cell| cell.iter().map(|vertex| remap[vertex]).collect())
            .collect(),
        quality(0.23),
    )
    .unwrap();
    assert_eq!(half_depth.cells().len(), 72);
    let half_artifact = SimplicialMeshEnvelopeV1::from_mesh(&half_depth).unwrap();
    assert!(GeometryMeshCorrespondenceEnvelopeV1::new(&geometry, &model, &half_artifact).is_err());

    let mut cavity_cells = mesh.cells().to_vec();
    cavity_cells.remove(62);
    let cavity =
        SimplicialMesh::new(3, mesh.vertices().to_vec(), cavity_cells, quality(0.23)).unwrap();
    assert_eq!(cavity.cells().len(), 143);
    let cavity_artifact = SimplicialMeshEnvelopeV1::from_mesh(&cavity).unwrap();
    assert!(
        GeometryMeshCorrespondenceEnvelopeV1::new(&geometry, &model, &cavity_artifact).is_err()
    );
}
