use std::collections::BTreeSet;
use std::f64::consts::PI;
use std::path::Path;
use std::process::Command;

use eqiora::artifact::{
    GeometryDefinitionV1, GeometryMeshCorrespondenceEnvelopeV1, SimplicialMeshEnvelopeV1,
};
use eqiora::diagnostic::codes;
use eqiora::geometry::{
    CanonicalGeometryV1, CircularHoleChordalMeshV1, EDGE_DIMENSION, FACE_DIMENSION, NamedEntitySet,
    VERTEX_DIMENSION,
};
use eqiora::meshing::{MeshEntity, MeshQualityGate, MeshTopology};
use sha2::{Digest, Sha256};

const BOUNDS: [[f64; 2]; 2] = [[0.0, 2.2], [0.0, 0.41]];
const CENTER: [f64; 2] = [0.2, 0.2];
const RADIUS_M: f64 = 0.05;
const SOURCE_TOLERANCE_M: f64 = 1.0e-12;
const MAX_BOUNDARY_ERROR_M: f64 = 1.0e-4;
const MAX_SEGMENTS: usize = 50;
const MINIMUM_QUALITY: f64 = 1.0e-5;
const ORACLE_SHA256: &str = "0bdbbec6f9ff9c532ba5f30c856d1cd3b25e64949e4b11abf5fa3823e6a25742";

fn dfg_source() -> CanonicalGeometryV1 {
    CanonicalGeometryV1::from_circular_hole(
        BOUNDS,
        CENTER,
        RADIUS_M,
        vec![
            NamedEntitySet::new("inlet", EDGE_DIMENSION, vec![0]),
            NamedEntitySet::new("outlet", EDGE_DIMENSION, vec![1]),
            NamedEntitySet::new("walls", EDGE_DIMENSION, vec![2, 3]),
            NamedEntitySet::new("cylinder", EDGE_DIMENSION, vec![4]),
            NamedEntitySet::new("fluid", FACE_DIMENSION, vec![0]),
        ],
        SOURCE_TOLERANCE_M,
    )
    .unwrap()
}

fn entity_mapping_source() -> CanonicalGeometryV1 {
    CanonicalGeometryV1::from_circular_hole(
        BOUNDS,
        CENTER,
        RADIUS_M,
        vec![
            NamedEntitySet::new("corners", VERTEX_DIMENSION, vec![0, 1, 2, 3]),
            NamedEntitySet::new("x-low", EDGE_DIMENSION, vec![0]),
            NamedEntitySet::new("x-high", EDGE_DIMENSION, vec![1]),
            NamedEntitySet::new("y-low", EDGE_DIMENSION, vec![2]),
            NamedEntitySet::new("y-high", EDGE_DIMENSION, vec![3]),
            NamedEntitySet::new("circle", EDGE_DIMENSION, vec![4]),
            NamedEntitySet::new("face", FACE_DIMENSION, vec![0]),
        ],
        SOURCE_TOLERANCE_M,
    )
    .unwrap()
}

fn quality_gate() -> MeshQualityGate {
    MeshQualityGate::new(MINIMUM_QUALITY).unwrap()
}

fn realize(source: &CanonicalGeometryV1) -> CircularHoleChordalMeshV1 {
    CircularHoleChordalMeshV1::from_exact(
        source,
        MAX_BOUNDARY_ERROR_M,
        MAX_SEGMENTS,
        quality_gate(),
    )
    .unwrap()
}

#[test]
fn straight_geometry_is_not_an_exact_circle_source() {
    let exact = dfg_source();
    let chordal = realize(&exact);
    let straight = CanonicalGeometryV1::from_region(chordal.region()).unwrap();
    let error = CircularHoleChordalMeshV1::from_exact(
        &straight,
        MAX_BOUNDARY_ERROR_M,
        MAX_SEGMENTS,
        quality_gate(),
    )
    .expect_err("straight geometry cannot enter the circular realization");
    assert_eq!(error.code(), codes::INVALID_ARTIFACT);
}

fn oracle_path() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../verify/geometry/circular-hole-chordal-reference-mesh/oracle.py")
}

fn hex_digest(bytes: impl AsRef<[u8]>) -> String {
    bytes
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn assert_close(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "{actual:.17e} differs from {expected:.17e} by more than {tolerance:.17e}",
    );
}

#[test]
fn independent_oracle_and_dfg_reference_path_agree() {
    let oracle_bytes = std::fs::read(oracle_path()).unwrap();
    assert_eq!(hex_digest(Sha256::digest(&oracle_bytes)), ORACLE_SHA256);
    let oracle = Command::new("python3")
        .arg(oracle_path())
        .output()
        .expect("independent chordal-circle oracle executes");
    assert!(
        oracle.status.success(),
        "oracle failed: {}",
        String::from_utf8_lossy(&oracle.stderr)
    );
    let output = String::from_utf8(oracle.stdout).unwrap();
    for expected in [
        "select.accepted_n=50",
        "topology.vertices=104",
        "topology.triangles=104",
        "check.falsifier_naive_acos_cancels_to_unity=pass",
        "check.falsifier_stable_asin_survives=pass",
        "oracle.checks_total=99",
        "oracle.checks_failed=0",
        "oracle.result=pass",
    ] {
        assert!(output.contains(expected), "oracle omitted {expected}");
    }

    let source = dfg_source();
    let realization = realize(&source);
    assert_eq!(
        realization.source().digest_bytes(),
        source.digest_bytes(),
        "the realization binds the exact circular source"
    );
    assert_eq!(realization.requested_max_boundary_error_m(), 1.0e-4);
    assert_eq!(realization.circle_segments(), 50);
    assert_eq!(realization.region().vertices().len(), 104);
    assert_eq!(realization.region().faces().len(), 1);
    assert_eq!(realization.region().faces()[0].outer().len(), 54);
    assert_eq!(realization.region().faces()[0].holes()[0].len(), 50);
    assert_eq!(realization.region().edge_count(), 104);
    assert_eq!(realization.mesh().vertices().len(), 104);
    assert_eq!(realization.mesh().cells().len(), 104);
    assert_eq!(realization.mesh().entity_count(1), Some(208));
    assert!(realization.mesh().quality_report().minimum_mean_ratio() >= MINIMUM_QUALITY);

    let allowance_m: f64 = 6.252_776_074_688_882e-14;
    let area_allowance_m2: f64 = 1.964_367_538_078_461_7e-14;
    assert_eq!(
        realization.boundary_evaluation_allowance_m().to_bits(),
        allowance_m.to_bits()
    );
    assert_close(
        realization.boundary_error_bound_m() - allowance_m,
        9.866_357_858_642_19e-5,
        allowance_m,
    );
    assert!(realization.boundary_error_bound_m() <= MAX_BOUNDARY_ERROR_M);
    assert_close(
        realization.circle_area_deficit_m2(),
        2.065_453_620_546_776e-5,
        area_allowance_m2,
    );
    assert_close(
        realization.circle_perimeter_deficit_m(),
        2.066_677_124_124_434_7e-4,
        allowance_m,
    );

    assert_eq!(
        realization
            .region()
            .entity_set("inlet")
            .unwrap()
            .members()
            .len(),
        14
    );
    assert_eq!(
        realization
            .region()
            .entity_set("outlet")
            .unwrap()
            .members()
            .len(),
        2
    );
    assert_eq!(
        realization
            .region()
            .entity_set("walls")
            .unwrap()
            .members()
            .len(),
        38
    );
    assert_eq!(
        realization
            .region()
            .entity_set("cylinder")
            .unwrap()
            .members()
            .len(),
        50
    );

    let geometry = GeometryDefinitionV1::from_region(realization.region());
    let mesh = SimplicialMeshEnvelopeV1::from_mesh(realization.mesh()).unwrap();
    let correspondence =
        GeometryMeshCorrespondenceEnvelopeV1::from_region(&geometry, &mesh).unwrap();
    let mut owned_boundary = BTreeSet::<MeshEntity>::new();
    for (name, count) in [
        ("inlet", 14),
        ("outlet", 2),
        ("walls", 38),
        ("cylinder", 50),
    ] {
        let members = correspondence
            .region_entity_set_entities(&geometry, name)
            .unwrap();
        assert_eq!(members.len(), count);
        for member in members {
            assert_eq!(member.dimension(), 1);
            assert_eq!(realization.mesh().is_boundary_entity(member), Some(true));
            assert!(
                owned_boundary.insert(member),
                "boundary facet {member:?} has more than one physical owner"
            );
        }
    }
    assert_eq!(owned_boundary.len(), 104);
    let boundary_count = (0..realization.mesh().entity_count(1).unwrap())
        .filter(|&index| {
            realization
                .mesh()
                .is_boundary_entity(MeshEntity::new(1, index))
                == Some(true)
        })
        .count();
    assert_eq!(boundary_count, owned_boundary.len());
    assert_eq!(
        correspondence
            .region_entity_set_entities(&geometry, "fluid")
            .unwrap()
            .len(),
        104
    );
}

#[test]
fn entity_propagation_source_binding_and_failure_paths_are_closed() {
    let dfg = dfg_source();
    let dfg_realization = realize(&dfg);
    let mapped = entity_mapping_source();
    let mapped_realization = realize(&mapped);
    assert_ne!(dfg.digest_bytes(), mapped.digest_bytes());
    assert_ne!(dfg_realization.source(), mapped_realization.source());

    for (name, count) in [
        ("corners", 4),
        ("x-low", 14),
        ("x-high", 2),
        ("y-low", 19),
        ("y-high", 19),
        ("circle", 50),
        ("face", 1),
    ] {
        assert_eq!(
            mapped_realization
                .region()
                .entity_set(name)
                .unwrap()
                .members()
                .len(),
            count,
            "source set {name} changed meaning while expanding to chords"
        );
    }
    assert_eq!(
        mapped_realization
            .region()
            .entity_set("corners")
            .unwrap()
            .dimension(),
        VERTEX_DIMENSION
    );

    let allowance = dfg_realization.boundary_evaluation_allowance_m();
    let below_allowance = f64::from_bits(allowance.to_bits() - 1);
    for invalid_error in [
        f64::NAN,
        f64::INFINITY,
        0.0,
        -1.0,
        allowance,
        below_allowance,
    ] {
        let error = CircularHoleChordalMeshV1::from_exact(
            &dfg,
            invalid_error,
            MAX_SEGMENTS,
            quality_gate(),
        )
        .unwrap_err();
        assert_eq!(error.code(), codes::INVALID_ARTIFACT);
    }
    for max_segments in [7, 49, 100_001] {
        let error = CircularHoleChordalMeshV1::from_exact(
            &dfg,
            MAX_BOUNDARY_ERROR_M,
            max_segments,
            quality_gate(),
        )
        .unwrap_err();
        assert_eq!(error.code(), codes::INVALID_ARTIFACT);
    }

    let too_strict = MeshQualityGate::new(
        0.5 * (1.0 + dfg_realization.mesh().quality_report().minimum_mean_ratio()),
    )
    .unwrap();
    let error =
        CircularHoleChordalMeshV1::from_exact(&dfg, MAX_BOUNDARY_ERROR_M, MAX_SEGMENTS, too_strict)
            .unwrap_err();
    assert_eq!(error.code(), codes::INVALID_MESH);

    let coarse =
        CircularHoleChordalMeshV1::from_exact(&dfg, 0.2, 8, MeshQualityGate::new(1.0e-8).unwrap())
            .unwrap();
    assert_eq!(coarse.circle_segments(), 8);
}

fn ideal_sagitta_m(segments: usize) -> f64 {
    let sine = (PI / (2.0 * segments as f64)).sin();
    2.0 * RADIUS_M * sine * sine
}

fn request_selecting(segments: usize, allowance_m: f64) -> f64 {
    if segments == 8 {
        0.2
    } else {
        0.5 * (ideal_sagitta_m(segments - 1) + ideal_sagitta_m(segments)) + allowance_m
    }
}

fn observed_order(coarse: f64, fine: f64) -> f64 {
    (coarse / fine).log2()
}

#[test]
fn boundary_area_and_perimeter_converge_independently_at_second_order() {
    let source = dfg_source();
    let allowance_m = realize(&source).boundary_evaluation_allowance_m();
    let mut boundary = Vec::new();
    let mut area = Vec::new();
    let mut perimeter = Vec::new();
    for segments in [8, 16, 32, 64] {
        let realization = CircularHoleChordalMeshV1::from_exact(
            &source,
            request_selecting(segments, allowance_m),
            segments,
            MeshQualityGate::new(1.0e-8).unwrap(),
        )
        .unwrap();
        assert_eq!(realization.circle_segments(), segments);
        assert_eq!(
            realization.source().digest_bytes(),
            source.digest_bytes(),
            "mesh refinement must not change the exact circle source identity"
        );
        boundary.push(realization.boundary_error_bound_m() - allowance_m);
        area.push(realization.circle_area_deficit_m2());
        perimeter.push(realization.circle_perimeter_deficit_m());
    }

    let boundary_orders = [
        observed_order(boundary[0], boundary[1]),
        observed_order(boundary[1], boundary[2]),
        observed_order(boundary[2], boundary[3]),
    ];
    let area_orders = [
        observed_order(area[0], area[1]),
        observed_order(area[1], area[2]),
        observed_order(area[2], area[3]),
    ];
    let perimeter_orders = [
        observed_order(perimeter[0], perimeter[1]),
        observed_order(perimeter[1], perimeter[2]),
        observed_order(perimeter[2], perimeter[3]),
    ];
    for (actual, expected) in
        boundary_orders
            .into_iter()
            .zip([1.986_072_498_760, 1.996_522_326_355, 1.999_130_843_560])
    {
        assert_close(actual, expected, 1.0e-9);
    }
    for (actual, expected) in
        area_orders
            .into_iter()
            .zip([1.966_597_557_231, 1.991_655_028_264, 1.997_914_114_431])
    {
        assert_close(actual, expected, 1.0e-9);
    }
    for (actual, expected) in
        perimeter_orders
            .into_iter()
            .zip([1.991_655_028_264, 1.997_914_114_431, 1.999_478_551_019])
    {
        assert_close(actual, expected, 1.0e-9);
    }
}
