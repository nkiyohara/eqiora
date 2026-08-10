//! Precommitted E1 profile/topology evidence for Stokes dissipation design.
//!
//! Evidence authority:
//! `/data/nk523/.tmp/issue407-stokes-e1-profile-topology-association-evidence-v1.md`,
//! SHA-256 `c680c63616b37c04b51a2baddb1f8430f2af3c18e7b57ad915c063b37e8628c2`.
//!
//! The production writer must return the real private binding. These tests do
//! not build a substitute Geometry, Model, Mesh, correspondence, or evidence
//! DTO. Mutant replay is a `cfg(test)` revalidation seam over that admitted
//! binding; it is not a second production constructor or public API.

use std::collections::{BTreeMap, BTreeSet};

use sha2::{Digest, Sha256};

use super::{
    E1ProfileTopologyEvidenceMutation2d, E1ProfileTopologyRejection2d,
    StokesDissipationGeometryModelBinding2d, StokesDissipationTopologyRole2d,
    e1_stokes_dissipation_sealed_inputs_v1,
};

const SEALED_INPUT_SHA256: [u8; 32] = [
    218, 50, 35, 213, 28, 175, 17, 246, 230, 39, 84, 15, 146, 132, 194, 187, 48, 117, 24, 247, 216,
    122, 98, 226, 137, 122, 159, 55, 50, 187, 246, 32,
];
const REFERENCE_IDENTITY: &str = "stokes-square-ring-reference-n32-m4-v1";
const REFINED_IDENTITY: &str = "stokes-square-ring-refined-n64-m8-v1";
const PROFILE_COORDINATE_ATOL_M: f64 = 2.0e-12;
const ANALYTIC_AREA_ATOL_M2: f64 = 5.0e-13;
const ANALYTIC_AREA_RTOL: f64 = 5.0e-13;
const HARMONIC_RESIDUAL_UPPER_BOUND: f64 = 2.0e-11;
const MINIMUM_SIGNED_AREA_M2: f64 = 1.0e-8;
const MINIMUM_SCALED_JACOBIAN: f64 = 1.5e-2;

#[test]
fn sealed_positive_binds_the_same_nonzero_profile_to_distinct_topologies() {
    let reference = admitted_binding(StokesDissipationTopologyRole2d::Reference);
    let refined = admitted_binding(StokesDissipationTopologyRole2d::Refined);

    assert_eq!(reference.profile_identity(), refined.profile_identity());
    assert_eq!(reference.profile().coefficients(), [2.0e-3, 0.0]);
    assert_eq!(refined.profile().coefficients(), [2.0e-3, 0.0]);
    assert_ne!(reference.profile().coefficients(), [0.0, 0.0]);

    assert_eq!(reference.topology_content_identity(), REFERENCE_IDENTITY);
    assert_eq!(refined.topology_content_identity(), REFINED_IDENTITY);
    assert_ne!(
        reference.topology_content_identity(),
        refined.topology_content_identity()
    );
    assert_ne!(
        reference.chordal_geometry_digest(),
        refined.chordal_geometry_digest()
    );
    assert_ne!(
        reference.mesh_artifact_digest(),
        refined.mesh_artifact_digest()
    );
    assert_ne!(
        reference.model_artifact_digest(),
        refined.model_artifact_digest()
    );

    assert_complete_association(&reference);
    assert_complete_association(&refined);
    assert_topology_and_harmonic_state(&reference);
    assert_topology_and_harmonic_state(&refined);
}

#[test]
fn exact_area_is_owned_by_the_profile_not_the_chordal_polygon() {
    for role in roles() {
        let binding = admitted_binding(role);
        let profile = binding.profile();
        let expected = std::f64::consts::PI * profile.area_radius_m().powi(2);
        assert_mixed_close(
            profile.analytic_area_m2(),
            expected,
            ANALYTIC_AREA_ATOL_M2,
            ANALYTIC_AREA_RTOL,
        );

        let body = binding.body_vertex_ids();
        let coordinates = binding.fixed_topology_state().coordinates();
        let polygon = signed_polygon_area(body, coordinates);
        let analytic_floor = ANALYTIC_AREA_ATOL_M2
            + ANALYTIC_AREA_RTOL * expected.abs().max(profile.analytic_area_m2().abs());
        assert!(
            expected - polygon > analytic_floor,
            "finite chordal area must not alias exact analytic area"
        );
    }
}

#[test]
fn circle_and_exact_conjugate_design_aliases_are_rejected_after_positive() {
    // `CircleAlias` replaces plus-profile coordinates/Geometry while retaining
    // the plus identity. `DesignSwap` substitutes the exact sealed minus
    // branch at the same first coordinate-a2 step while retaining plus Model
    // Parameters. Both mutations happen only after ordinary admission.
    for role in roles() {
        assert_mutant_rejected(
            role,
            E1ProfileTopologyEvidenceMutation2d::CircleAlias,
            E1ProfileTopologyRejection2d::ProfileIdentity,
        );
        assert_mutant_rejected(
            role,
            E1ProfileTopologyEvidenceMutation2d::DesignSwap,
            E1ProfileTopologyRejection2d::DesignAssociation,
        );
    }
}

#[test]
fn reference_cannot_be_relabelled_or_reused_as_refined() {
    // `FakeRefinement` reuses the complete reference association under the
    // refined role. Counts, filenames, and objective values are not consulted.
    for role in roles() {
        assert_mutant_rejected(
            role,
            E1ProfileTopologyEvidenceMutation2d::FakeRefinement,
            E1ProfileTopologyRejection2d::TopologyRole,
        );
    }
}

#[test]
fn area_geometry_model_mesh_and_correspondence_swaps_are_rejected() {
    // These variants change exactly one already-admitted association member:
    // analytic-area owner, Model GeometryRegion, or Mesh/correspondence/state.
    for role in roles() {
        for (mutation, rejection) in [
            (
                E1ProfileTopologyEvidenceMutation2d::PolygonAreaAuthority,
                E1ProfileTopologyRejection2d::AnalyticAreaAuthority,
            ),
            (
                E1ProfileTopologyEvidenceMutation2d::GeometryRegionSwap,
                E1ProfileTopologyRejection2d::GeometryModelAssociation,
            ),
            (
                E1ProfileTopologyEvidenceMutation2d::MeshCorrespondenceSwap,
                E1ProfileTopologyRejection2d::MeshCorrespondenceAssociation,
            ),
        ] {
            assert_mutant_rejected(role, mutation, rejection);
        }
    }
}

#[test]
fn facet_role_and_endpoint_mutants_are_rejected() {
    for role in roles() {
        for mutation in [
            E1ProfileTopologyEvidenceMutation2d::FacetRoleSwap,
            E1ProfileTopologyEvidenceMutation2d::FacetLabelMissing,
            E1ProfileTopologyEvidenceMutation2d::FacetLabelDuplicated,
            E1ProfileTopologyEvidenceMutation2d::FacetLabelAdded,
            E1ProfileTopologyEvidenceMutation2d::FacetEndpointOutOfRange,
        ] {
            assert_mutant_rejected(
                role,
                mutation,
                if mutation == E1ProfileTopologyEvidenceMutation2d::FacetRoleSwap
                    || mutation == E1ProfileTopologyEvidenceMutation2d::FacetLabelMissing
                    || mutation == E1ProfileTopologyEvidenceMutation2d::FacetLabelDuplicated
                    || mutation == E1ProfileTopologyEvidenceMutation2d::FacetLabelAdded
                {
                    E1ProfileTopologyRejection2d::FacetRole
                } else {
                    E1ProfileTopologyRejection2d::TopologyIndex
                },
            );
        }
    }
}

#[test]
fn correspondence_angle_and_cell_index_mutants_are_rejected() {
    // Every index mutation forks after the exact topology passed. It changes
    // one existing index/order entry (or uses vertex_count as the first invalid
    // endpoint) while retaining the original content-identity label.
    for role in roles() {
        for mutation in [
            E1ProfileTopologyEvidenceMutation2d::CorrespondenceIndexSwap,
            E1ProfileTopologyEvidenceMutation2d::AngleOrderSwap,
            E1ProfileTopologyEvidenceMutation2d::CellConnectivityDuplicate,
            E1ProfileTopologyEvidenceMutation2d::CellEndpointOutOfRange,
        ] {
            assert_mutant_rejected(role, mutation, E1ProfileTopologyRejection2d::TopologyIndex);
        }
    }
}

fn roles() -> [StokesDissipationTopologyRole2d; 2] {
    [
        StokesDissipationTopologyRole2d::Reference,
        StokesDissipationTopologyRole2d::Refined,
    ]
}

fn admitted_binding(
    role: StokesDissipationTopologyRole2d,
) -> StokesDissipationGeometryModelBinding2d {
    let bytes = e1_stokes_dissipation_sealed_inputs_v1();
    let digest: [u8; 32] = Sha256::digest(bytes).into();
    assert_eq!(digest, SEALED_INPUT_SHA256);

    let binding = StokesDissipationGeometryModelBinding2d::from_e1_sealed_inputs_v1(bytes, role)
        .expect("the exact noncircular E1 positive must be admitted");
    assert_eq!(binding.sealed_input_sha256(), SEALED_INPUT_SHA256);
    binding
        .revalidate_e1_profile_topology()
        .expect("the ordinary positive must reach complete binding admission");
    binding
}

fn assert_complete_association(binding: &StokesDissipationGeometryModelBinding2d) {
    let profile = binding.profile();
    assert_eq!(profile.area_radius_m().to_bits(), 1.0_f64.to_bits());
    assert_eq!(profile.coefficients()[0].to_bits(), 2.0e-3_f64.to_bits());
    assert_eq!(profile.coefficients()[1].to_bits(), 0.0_f64.to_bits());
    assert_eq!(
        binding.model_profile_values().map(f64::to_bits),
        [
            profile.area_radius_m().to_bits(),
            profile.coefficients()[0].to_bits(),
            profile.coefficients()[1].to_bits(),
        ]
    );
    assert_eq!(binding.profile_identity(), binding.model_profile_identity());
    assert_eq!(
        binding.chordal_geometry_digest(),
        binding.model_geometry_region_digest()
    );

    let entity_sets = binding
        .entity_set_names()
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    assert_eq!(binding.entity_set_names().len(), 6);
    assert_eq!(
        entity_sets,
        BTreeSet::from([
            "body",
            "fluid",
            "outer_x_lower",
            "outer_x_upper",
            "outer_y_lower",
            "outer_y_upper",
        ])
    );
}

fn assert_topology_and_harmonic_state(binding: &StokesDissipationGeometryModelBinding2d) {
    let (sector_count, radial_interval_count, expected_identity) = match binding.topology_role() {
        StokesDissipationTopologyRole2d::Reference => (32, 4, REFERENCE_IDENTITY),
        StokesDissipationTopologyRole2d::Refined => (64, 8, REFINED_IDENTITY),
    };
    assert_eq!(binding.topology_content_identity(), expected_identity);

    let reference = binding.reference_topology();
    let current = binding.fixed_topology_state().coordinates();
    assert_eq!(
        reference.vertices().len(),
        sector_count * (radial_interval_count + 1)
    );
    assert_eq!(
        reference.cells().len(),
        2 * sector_count * radial_interval_count
    );
    assert_eq!(current.len(), reference.vertices().len());
    binding
        .fixed_topology_state()
        .reconstruct_mesh(reference)
        .expect("the exact state must replay on unchanged connectivity");

    let expected_body = (0..sector_count).collect::<Vec<_>>();
    let expected_outer = (radial_interval_count * sector_count
        ..(radial_interval_count + 1) * sector_count)
        .collect::<Vec<_>>();
    assert_eq!(binding.body_vertex_ids(), expected_body);
    assert_eq!(binding.outer_vertex_ids(), expected_outer);
    assert_eq!(binding.ordered_body_angle_turns().len(), sector_count);

    let profile = binding.profile();
    for (angle_index, (&vertex, turns)) in binding
        .body_vertex_ids()
        .iter()
        .zip(binding.ordered_body_angle_turns())
        .enumerate()
    {
        assert_eq!(parse_turn(turns), (angle_index, sector_count));
        let theta = 2.0 * std::f64::consts::PI * angle_index as f64 / sector_count as f64;
        let radius = independent_radius(profile.area_radius_m(), profile.coefficients(), theta);
        let expected = [radius * theta.cos(), radius * theta.sin()];
        assert_coordinate_close(&current[vertex], expected);
        assert_mixed_close(
            profile.radial_coordinate_m(theta),
            radius,
            PROFILE_COORDINATE_ATOL_M,
            0.0,
        );
    }

    for &vertex in binding.outer_vertex_ids() {
        assert_eq!(current[vertex].len(), 2);
        assert_eq!(reference.vertices()[vertex].len(), 2);
        assert_eq!(
            current[vertex][0].to_bits(),
            reference.vertices()[vertex][0].to_bits()
        );
        assert_eq!(
            current[vertex][1].to_bits(),
            reference.vertices()[vertex][1].to_bits()
        );
    }

    assert_indexed_cells_and_facets(binding, sector_count, radial_interval_count);
    assert_harmonic_residual(binding);
    assert_mesh_predicates(binding);
}

fn assert_indexed_cells_and_facets(
    binding: &StokesDissipationGeometryModelBinding2d,
    sector_count: usize,
    radial_interval_count: usize,
) {
    let reference = binding.reference_topology();
    let vertex_count = reference.vertices().len();
    let mut directed_edges = BTreeMap::<(usize, usize), Vec<(usize, usize)>>::new();

    for cell in reference.cells() {
        let [a, b, c] = cell.as_slice() else {
            panic!("every sealed cell must be triangular");
        };
        assert!(a < &vertex_count && b < &vertex_count && c < &vertex_count);
        assert!(a != b && b != c && c != a);
        assert!(
            signed_triangle_area(
                &reference.vertices()[*a],
                &reference.vertices()[*b],
                &reference.vertices()[*c]
            ) > 0.0
        );
        for (first, second) in [(*a, *b), (*b, *c), (*c, *a)] {
            directed_edges
                .entry(ordered_edge(first, second))
                .or_default()
                .push((first, second));
        }
    }
    assert!(
        directed_edges
            .values()
            .all(|incidence| matches!(incidence.len(), 1 | 2))
    );
    assert_connected_to_dirichlet_boundary(
        reference.vertices().len(),
        reference.cells(),
        binding.body_vertex_ids(),
        binding.outer_vertex_ids(),
    );

    let facets = binding.boundary_facets();
    assert_eq!(facets.len(), 2 * sector_count);
    let expected_boundary_edges = directed_edges
        .iter()
        .filter_map(|(edge, incidence)| (incidence.len() == 1).then_some(*edge))
        .collect::<BTreeSet<_>>();
    let mut observed_edges = BTreeSet::new();
    let mut labels = BTreeSet::new();
    let outer_start = radial_interval_count * sector_count;

    for (expected_id, facet) in facets.iter().enumerate() {
        assert_eq!(facet.id(), expected_id);
        let [first, second] = facet.vertices();
        assert!(first < vertex_count && second < vertex_count && first != second);
        let edge = ordered_edge(first, second);
        assert!(observed_edges.insert(edge));
        assert_eq!(directed_edges[&edge], vec![(first, second)]);
        labels.insert(facet.source_label());

        if first < sector_count && second < sector_count {
            assert_eq!(facet.kind_name(), "body");
            assert_eq!(facet.source_label(), "body_no_slip");
            assert_eq!(facet.orientation_name(), "fluid_domain_boundary_clockwise");
        } else {
            assert!(first >= outer_start && second >= outer_start);
            assert_eq!(facet.kind_name(), "outer");
            assert_eq!(
                facet.orientation_name(),
                "fluid_domain_boundary_counterclockwise"
            );
            let midpoint = [
                0.5 * (reference.vertices()[first][0] + reference.vertices()[second][0]),
                0.5 * (reference.vertices()[first][1] + reference.vertices()[second][1]),
            ];
            assert_eq!(
                facet.source_label(),
                expected_outer_label(midpoint, binding.profile().area_radius_m())
            );
        }
    }
    assert_eq!(observed_edges, expected_boundary_edges);
    assert_eq!(
        labels,
        BTreeSet::from([
            "body_no_slip",
            "outer_x_minus",
            "outer_x_plus",
            "outer_y_minus",
            "outer_y_plus",
        ])
    );

    let correspondence = binding.correspondence();
    assert_eq!(correspondence.len(), sector_count);
    for (angle_index, entry) in correspondence.iter().enumerate() {
        assert_eq!(entry.angle_index(), angle_index);
        assert_eq!(parse_turn(entry.angle_turns()), (angle_index, sector_count));
        assert_eq!(
            entry.body_vertex_id(),
            binding.body_vertex_ids()[angle_index]
        );
        assert_eq!(entry.body_facet_id(), angle_index);
        assert_eq!(facets[entry.body_facet_id()].source_label(), "body_no_slip");
    }
}

fn assert_harmonic_residual(binding: &StokesDissipationGeometryModelBinding2d) {
    let reference = binding.reference_topology();
    let current = binding.fixed_topology_state().coordinates();
    let vertex_count = reference.vertices().len();
    let mut stiffness = vec![0.0; vertex_count * vertex_count];

    for cell in reference.cells() {
        let [a, b, c] = cell.as_slice() else {
            unreachable!("triangular topology was checked before harmonic replay");
        };
        let ids = [*a, *b, *c];
        let points = [
            &reference.vertices()[ids[0]],
            &reference.vertices()[ids[1]],
            &reference.vertices()[ids[2]],
        ];
        let twice_area = cross(sub(points[1], points[0]), sub(points[2], points[0]));
        assert!(twice_area > 0.0);
        let gradient_numerators = [
            [points[1][1] - points[2][1], points[2][0] - points[1][0]],
            [points[2][1] - points[0][1], points[0][0] - points[2][0]],
            [points[0][1] - points[1][1], points[1][0] - points[0][0]],
        ];
        for row in 0..3 {
            for column in 0..3 {
                let local =
                    dot(gradient_numerators[row], gradient_numerators[column]) / (2.0 * twice_area);
                stiffness[ids[row] * vertex_count + ids[column]] += local;
            }
        }
    }

    let boundary = binding
        .body_vertex_ids()
        .iter()
        .chain(binding.outer_vertex_ids())
        .copied()
        .collect::<BTreeSet<_>>();
    let mut displacement = vec![[0.0; 2]; vertex_count];
    let mut boundary_scale = 0.0_f64;
    for vertex in 0..vertex_count {
        for component in 0..2 {
            displacement[vertex][component] =
                current[vertex][component] - reference.vertices()[vertex][component];
            if boundary.contains(&vertex) {
                boundary_scale = boundary_scale.max(displacement[vertex][component].abs());
            }
        }
    }

    let mut residual = 0.0_f64;
    for vertex in 0..vertex_count {
        if boundary.contains(&vertex) {
            continue;
        }
        // The inner sum varies the outer vertex index while this loop selects
        // one component; iterating `displacement` would swap those axes.
        #[allow(clippy::needless_range_loop)]
        for component in 0..2 {
            let value = (0..vertex_count)
                .map(|column| {
                    stiffness[vertex * vertex_count + column] * displacement[column][component]
                })
                .sum::<f64>();
            residual = residual.max(value.abs());
        }
    }
    let normalized = residual / boundary_scale.max(1.0);
    assert!(normalized <= HARMONIC_RESIDUAL_UPPER_BOUND);
}

fn assert_connected_to_dirichlet_boundary(
    vertex_count: usize,
    cells: &[Vec<usize>],
    body: &[usize],
    outer: &[usize],
) {
    let mut adjacency = vec![Vec::new(); vertex_count];
    for cell in cells {
        let [first, second, third] = cell.as_slice() else {
            unreachable!("triangular topology was checked before connectivity replay");
        };
        for (left, right) in [(*first, *second), (*second, *third), (*third, *first)] {
            adjacency[left].push(right);
            adjacency[right].push(left);
        }
    }

    let mut reachable = vec![false; vertex_count];
    let mut stack = body.iter().chain(outer).copied().collect::<Vec<_>>();
    while let Some(vertex) = stack.pop() {
        if reachable[vertex] {
            continue;
        }
        reachable[vertex] = true;
        stack.extend(
            adjacency[vertex]
                .iter()
                .copied()
                .filter(|&neighbor| !reachable[neighbor]),
        );
    }
    assert!(reachable.into_iter().all(|value| value));
}

fn assert_mesh_predicates(binding: &StokesDissipationGeometryModelBinding2d) {
    let current = binding.fixed_topology_state().coordinates();
    for cell in binding.reference_topology().cells() {
        let [a, b, c] = cell.as_slice() else {
            unreachable!("triangular topology was checked before mesh predicates");
        };
        let area = signed_triangle_area(&current[*a], &current[*b], &current[*c]);
        assert!(area > MINIMUM_SIGNED_AREA_M2);
        let denominator = squared_distance(&current[*a], &current[*b])
            + squared_distance(&current[*b], &current[*c])
            + squared_distance(&current[*c], &current[*a]);
        let scaled_jacobian = 4.0 * 3.0_f64.sqrt() * area / denominator;
        assert!(scaled_jacobian > MINIMUM_SCALED_JACOBIAN);
    }

    let body = binding
        .body_vertex_ids()
        .iter()
        .map(|&vertex| [current[vertex][0], current[vertex][1]])
        .collect::<Vec<_>>();
    assert!(polygon_is_simple(&body));
    let half_width = 10.0 * binding.profile().area_radius_m();
    assert!(body.iter().all(|point| {
        half_width - point[0].abs().max(point[1].abs()) > 5.0 * binding.profile().area_radius_m()
    }));
}

fn assert_mutant_rejected(
    role: StokesDissipationTopologyRole2d,
    mutation: E1ProfileTopologyEvidenceMutation2d,
    expected: E1ProfileTopologyRejection2d,
) {
    let binding = admitted_binding(role);
    let rejection = binding
        .revalidate_e1_evidence_mutant(mutation)
        .expect_err("a mutant forked from the admitted positive must be rejected");
    assert_eq!(rejection, expected);
}

fn independent_radius(area_radius_m: f64, coefficients: [f64; 2], theta: f64) -> f64 {
    let [a_2, a_4] = coefficients;
    let numerator = 1.0 + a_2 * (2.0 * theta).cos() + a_4 * (4.0 * theta).cos();
    let denominator = (1.0 + 0.5 * (a_2 * a_2 + a_4 * a_4)).sqrt();
    area_radius_m * numerator / denominator
}

fn expected_outer_label(midpoint: [f64; 2], area_radius_m: f64) -> &'static str {
    let half_width = 10.0 * area_radius_m;
    for (coordinate, value, label) in [
        (midpoint[0], -half_width, "outer_x_minus"),
        (midpoint[0], half_width, "outer_x_plus"),
        (midpoint[1], -half_width, "outer_y_minus"),
        (midpoint[1], half_width, "outer_y_plus"),
    ] {
        if (coordinate - value).abs() <= PROFILE_COORDINATE_ATOL_M {
            return label;
        }
    }
    panic!("sealed outer facet is not on one exact square side");
}

fn parse_turn(value: &str) -> (usize, usize) {
    let (numerator, denominator) = value
        .split_once('/')
        .expect("a sealed turn is one exact rational string");
    (
        numerator
            .parse()
            .expect("sealed turn numerator is an index"),
        denominator
            .parse()
            .expect("sealed turn denominator is a sector count"),
    )
}

fn assert_coordinate_close(actual: &[f64], expected: [f64; 2]) {
    assert_eq!(actual.len(), 2);
    for component in 0..2 {
        assert!((actual[component] - expected[component]).abs() <= PROFILE_COORDINATE_ATOL_M);
    }
}

fn assert_mixed_close(actual: f64, expected: f64, absolute: f64, relative: f64) {
    assert!((actual - expected).abs() <= absolute + relative * actual.abs().max(expected.abs()));
}

fn signed_polygon_area(vertices: &[usize], coordinates: &[Vec<f64>]) -> f64 {
    0.5 * vertices
        .iter()
        .zip(vertices.iter().cycle().skip(1))
        .take(vertices.len())
        .map(|(&first, &second)| {
            cross(
                [coordinates[first][0], coordinates[first][1]],
                [coordinates[second][0], coordinates[second][1]],
            )
        })
        .sum::<f64>()
}

fn signed_triangle_area(first: &[f64], second: &[f64], third: &[f64]) -> f64 {
    0.5 * cross(sub(second, first), sub(third, first))
}

fn ordered_edge(first: usize, second: usize) -> (usize, usize) {
    if first < second {
        (first, second)
    } else {
        (second, first)
    }
}

fn sub(left: &[f64], right: &[f64]) -> [f64; 2] {
    [left[0] - right[0], left[1] - right[1]]
}

fn dot(left: [f64; 2], right: [f64; 2]) -> f64 {
    left[0] * right[0] + left[1] * right[1]
}

fn cross(left: [f64; 2], right: [f64; 2]) -> f64 {
    left[0] * right[1] - left[1] * right[0]
}

fn squared_distance(first: &[f64], second: &[f64]) -> f64 {
    (first[0] - second[0]).powi(2) + (first[1] - second[1]).powi(2)
}

fn polygon_is_simple(vertices: &[[f64; 2]]) -> bool {
    for first in 0..vertices.len() {
        let first_next = (first + 1) % vertices.len();
        for second in first + 1..vertices.len() {
            let second_next = (second + 1) % vertices.len();
            if first == second
                || first == second_next
                || first_next == second
                || first_next == second_next
            {
                continue;
            }
            if proper_segments_intersect(
                vertices[first],
                vertices[first_next],
                vertices[second],
                vertices[second_next],
            ) {
                return false;
            }
        }
    }
    true
}

fn proper_segments_intersect(
    first: [f64; 2],
    second: [f64; 2],
    third: [f64; 2],
    fourth: [f64; 2],
) -> bool {
    let orient = |a: [f64; 2], b: [f64; 2], c: [f64; 2]| {
        cross([b[0] - a[0], b[1] - a[1]], [c[0] - a[0], c[1] - a[1]])
    };
    let first_side = orient(first, second, third);
    let second_side = orient(first, second, fourth);
    let third_side = orient(third, fourth, first);
    let fourth_side = orient(third, fourth, second);
    first_side * second_side < 0.0 && third_side * fourth_side < 0.0
}
