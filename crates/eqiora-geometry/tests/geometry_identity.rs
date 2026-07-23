use eqiora_core::Id;
use eqiora_core::entity::kinds;
use eqiora_geometry::{
    BodyAssociationCandidate, CartesianBodyAssignment, CartesianBoundaryAssignment,
    GeometryCorrespondenceError, GeometryEntity, GeometryMeshCorrespondence,
    GeometryRevisionReference, GeometryRevisionTopology, ParentOutward,
    RetainedGeometryAssociation, RetentionRejection,
};
use eqiora_meshing::{MeshEntity, MeshQualityGate, MeshTopology, SimplicialMesh};
use eqiora_schema::kernel::BoundarySide;

type DomainId = Id<kinds::Domain>;

#[derive(Clone, Copy)]
struct BodyDomains {
    body: DomainId,
    boundaries: [[DomainId; 2]; 2],
}

fn domains() -> [BodyDomains; 2] {
    [0, 1].map(|_| BodyDomains {
        body: DomainId::new(),
        boundaries: [
            [DomainId::new(), DomainId::new()],
            [DomainId::new(), DomainId::new()],
        ],
    })
}

fn two_body_mesh() -> SimplicialMesh {
    SimplicialMesh::new(
        2,
        vec![
            vec![0.0, 0.0],
            vec![1.0, 0.0],
            vec![0.0, 1.0],
            vec![1.0, 1.0],
            vec![2.0, 0.0],
            vec![2.0, 1.0],
        ],
        vec![vec![0, 1, 3], vec![0, 3, 2], vec![1, 4, 5], vec![1, 5, 3]],
        MeshQualityGate::new(0.05).unwrap(),
    )
    .unwrap()
}

fn role_facets(
    mesh: &SimplicialMesh,
    body_cells: &[MeshEntity],
    x_lower: f64,
    x_upper: f64,
) -> [[Vec<MeshEntity>; 2]; 2] {
    let mut roles: [[Vec<MeshEntity>; 2]; 2] = Default::default();
    let facet_count = mesh.entity_count(1).unwrap();
    for index in 0..facet_count {
        let facet = MeshEntity::new(1, index);
        let adjacent_body_cells = mesh
            .incidence(facet, 2)
            .unwrap()
            .iter()
            .filter(|entry| body_cells.contains(&entry.entity))
            .count();
        if adjacent_body_cells != 1 {
            continue;
        }
        let vertices = mesh.entity_vertices(facet).unwrap();
        let coordinates = vertices
            .iter()
            .map(|vertex| &mesh.vertices()[vertex.index()])
            .collect::<Vec<_>>();
        let role = if coordinates.iter().all(|point| point[0] == x_lower) {
            (0, 0)
        } else if coordinates.iter().all(|point| point[0] == x_upper) {
            (0, 1)
        } else if coordinates.iter().all(|point| point[1] == 0.0) {
            (1, 0)
        } else if coordinates.iter().all(|point| point[1] == 1.0) {
            (1, 1)
        } else {
            panic!("exposed facet is not on a Cartesian side");
        };
        roles[role.0][role.1].push(facet);
    }
    roles
}

fn assignments(
    mesh: &SimplicialMesh,
    domains: [BodyDomains; 2],
) -> (
    Vec<CartesianBodyAssignment>,
    Vec<CartesianBoundaryAssignment>,
) {
    let body_cells = [
        vec![MeshEntity::new(2, 0), MeshEntity::new(2, 1)],
        vec![MeshEntity::new(2, 2), MeshEntity::new(2, 3)],
    ];
    let mut bodies = Vec::new();
    let mut boundaries = Vec::new();
    for body_index in 0..2 {
        bodies.push(CartesianBodyAssignment::new(
            domains[body_index].body,
            GeometryEntity::new(2, body_index),
            body_cells[body_index].clone(),
        ));
        let roles = role_facets(
            mesh,
            &body_cells[body_index],
            body_index as f64,
            body_index as f64 + 1.0,
        );
        for (axis, axis_roles) in roles.iter().enumerate() {
            for (side_index, role_facets) in axis_roles.iter().enumerate() {
                let side = if side_index == 0 {
                    BoundarySide::Lower
                } else {
                    BoundarySide::Upper
                };
                let geometry_index = match (body_index, axis, side_index) {
                    (0, 0, 0) => 0,
                    (0, 0, 1) | (1, 0, 0) => 1,
                    (0, 1, 0) => 2,
                    (0, 1, 1) => 3,
                    (1, 0, 1) => 4,
                    (1, 1, 0) => 5,
                    (1, 1, 1) => 6,
                    _ => unreachable!(),
                };
                boundaries.push(CartesianBoundaryAssignment::new(
                    domains[body_index].boundaries[axis][side_index],
                    domains[body_index].body,
                    axis,
                    side,
                    GeometryEntity::new(1, geometry_index),
                    role_facets.clone(),
                ));
            }
        }
    }
    (bodies, boundaries)
}

fn correspondence(
    mesh: &SimplicialMesh,
    domains: [BodyDomains; 2],
    revision_byte: u8,
) -> GeometryMeshCorrespondence {
    // Vertices are intentionally absent from the catalog. V1 proves only
    // body and codimension-one identity.
    let geometry = GeometryRevisionTopology::new(
        GeometryRevisionReference::from_digest_bytes([revision_byte; 32]),
        vec![0, 7, 2],
    )
    .unwrap();
    let (bodies, boundaries) = assignments(mesh, domains);
    GeometryMeshCorrespondence::validate(&geometry, mesh, bodies, boundaries).unwrap()
}

#[test]
fn closes_two_bodies_over_one_full_mesh_canonically() {
    let mesh = two_body_mesh();
    let domains = domains();
    let geometry = GeometryRevisionTopology::new(
        GeometryRevisionReference::from_digest_bytes([7; 32]),
        vec![0, 7, 2],
    )
    .unwrap();
    let (bodies, boundaries) = assignments(&mesh, domains);
    let accepted =
        GeometryMeshCorrespondence::validate(&geometry, &mesh, bodies.clone(), boundaries.clone())
            .unwrap();

    let mut reversed_bodies = bodies;
    reversed_bodies.reverse();
    for body in &mut reversed_bodies {
        let mut cells = body.cells().to_vec();
        cells.reverse();
        *body = CartesianBodyAssignment::new(body.domain(), body.geometry(), cells);
    }
    let mut reversed_boundaries = boundaries;
    reversed_boundaries.reverse();
    for boundary in &mut reversed_boundaries {
        let mut facets = boundary.facets().to_vec();
        facets.reverse();
        *boundary = CartesianBoundaryAssignment::new(
            boundary.domain(),
            boundary.parent(),
            boundary.axis(),
            boundary.side(),
            boundary.geometry(),
            facets,
        );
    }
    let reordered = GeometryMeshCorrespondence::validate(
        &geometry,
        &mesh,
        reversed_bodies,
        reversed_boundaries,
    )
    .unwrap();

    assert_eq!(accepted, reordered);
    assert_eq!(accepted.bodies().len(), 2);
    assert_eq!(accepted.boundaries().len(), 8);
    assert!(
        accepted
            .boundaries()
            .iter()
            .all(|boundary| boundary.orientation() == ParentOutward)
    );

    let left_interface = accepted
        .boundaries()
        .iter()
        .find(|boundary| {
            boundary.parent() == domains[0].body
                && boundary.axis() == 0
                && boundary.side() == BoundarySide::Upper
        })
        .unwrap();
    let right_interface = accepted
        .boundaries()
        .iter()
        .find(|boundary| {
            boundary.parent() == domains[1].body
                && boundary.axis() == 0
                && boundary.side() == BoundarySide::Lower
        })
        .unwrap();
    assert_eq!(left_interface.facets(), right_interface.facets());
    assert_ne!(left_interface.domain(), right_interface.domain());
}

#[test]
fn rejects_non_total_cell_and_boundary_partitions() {
    let mesh = two_body_mesh();
    let domains = domains();
    let geometry = GeometryRevisionTopology::new(
        GeometryRevisionReference::from_digest_bytes([8; 32]),
        vec![0, 7, 2],
    )
    .unwrap();
    let (mut bodies, boundaries) = assignments(&mesh, domains);
    bodies[1] = CartesianBodyAssignment::new(
        bodies[1].domain(),
        bodies[1].geometry(),
        vec![MeshEntity::new(2, 1), MeshEntity::new(2, 3)],
    );
    assert!(matches!(
        GeometryMeshCorrespondence::validate(&geometry, &mesh, bodies, boundaries),
        Err(GeometryCorrespondenceError::CellAssignedToMultipleBodies { .. })
    ));

    let (bodies, mut boundaries) = assignments(&mesh, domains);
    boundaries.retain(|boundary| {
        !(boundary.parent() == domains[0].body
            && boundary.axis() == 1
            && boundary.side() == BoundarySide::Upper)
    });
    assert!(matches!(
        GeometryMeshCorrespondence::validate(&geometry, &mesh, bodies, boundaries),
        Err(GeometryCorrespondenceError::UnassignedGeometryBoundary { .. })
    ));
}

#[test]
fn derives_boundary_retention_without_reusing_domain_ids() {
    let mesh = two_body_mesh();
    let source_domains = domains();
    let target_domains = domains();
    let source = correspondence(&mesh, source_domains, 1);
    let target = correspondence(&mesh, target_domains, 2);
    let candidates = vec![
        BodyAssociationCandidate::new(source_domains[1].body, target_domains[1].body),
        BodyAssociationCandidate::new(source_domains[0].body, target_domains[0].body),
    ];
    let retained = RetainedGeometryAssociation::validate(&source, &target, candidates).unwrap();

    assert_eq!(retained.bodies().len(), 2);
    assert_eq!(retained.boundaries().len(), 8);
    assert!(
        retained
            .bodies()
            .iter()
            .all(|pair| pair.source() != pair.target())
    );
    assert!(retained.boundaries().iter().all(|pair| {
        pair.source() != pair.target()
            && pair.source_parent() != pair.target_parent()
            && pair.axis() < 2
    }));
}

#[test]
fn rejects_missing_split_merge_and_ambiguous_associations() {
    let mesh = two_body_mesh();
    let source_domains = domains();
    let target_domains = domains();
    let source = correspondence(&mesh, source_domains, 3);
    let target = correspondence(&mesh, target_domains, 4);

    let missing = vec![BodyAssociationCandidate::new(
        source_domains[0].body,
        target_domains[0].body,
    )];
    assert!(matches!(
        RetainedGeometryAssociation::validate(&source, &target, missing),
        Err(RetentionRejection::Missing { .. })
    ));

    let split = vec![
        BodyAssociationCandidate::new(source_domains[0].body, target_domains[0].body),
        BodyAssociationCandidate::new(source_domains[0].body, target_domains[1].body),
    ];
    assert!(matches!(
        RetainedGeometryAssociation::validate(&source, &target, split),
        Err(RetentionRejection::Split { .. })
    ));

    let merged = vec![
        BodyAssociationCandidate::new(source_domains[0].body, target_domains[0].body),
        BodyAssociationCandidate::new(source_domains[1].body, target_domains[0].body),
    ];
    assert!(matches!(
        RetainedGeometryAssociation::validate(&source, &target, merged),
        Err(RetentionRejection::Merged { .. })
    ));

    let ambiguous = vec![
        BodyAssociationCandidate::new(source_domains[0].body, target_domains[0].body),
        BodyAssociationCandidate::new(source_domains[0].body, target_domains[1].body),
        BodyAssociationCandidate::new(source_domains[1].body, target_domains[0].body),
    ];
    assert!(matches!(
        RetainedGeometryAssociation::validate(&source, &target, ambiguous),
        Err(RetentionRejection::Ambiguous { .. })
    ));
}
