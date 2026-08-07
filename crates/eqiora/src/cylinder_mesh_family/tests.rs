//! Independent pre-implementation evidence for private cylinder MESH0.
//!
//! The production module did not exist when this file and its fixtures were
//! frozen. Every negative helper first admits and replays the complete ordinary
//! three-plus-one positive, then mutates a fresh candidate. No force, pressure,
//! Strouhal, scientific time step, tolerance, solver, or solve appears here.

use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

use eqiora_artifact::ArtifactDigest;
use eqiora_geometry::{CanonicalGeometryV1, EDGE_DIMENSION, FACE_DIMENSION, NamedEntitySet};

use super::*;

const SOURCE_SHA256: &str = "b00123472a596e8289820cabaee20d52cdf81b5572fa9ce58ff17cdaa00046d9";
const EXECUTABLE_SHA256: &str = "0a923f7069d3ab91d142ed7afcc9e933144c88034e2119067146d2dd87cb4cac";
const PRIMARY_RECIPE_SHA256: &str =
    "e53ec57c6c30f29a4441899bd50d9d530cde0bd33bac4cd45fce5f1e013d9b43";
const BIAS_RECIPE_SHA256: &str = "4fb346431e1703e79ba3b1c16d4d22b3751f62ffb65c81d533ac960509872c66";
const METHOD_SHA256: &str = "0d24c2b0719cab96a5443a383b5fb0b6a8f33226b23f5b1a119eacc306de0883";
const FIXTURE_MINIMUM_MEAN_RATIO: f64 = 1.0e-8;
const FIXTURE_MINIMUM_MEAN_RATIO_BITS: u64 = 0x3e45_798e_e230_8c3a;
const REQUESTS: [f64; 3] = [4.0e-3, 1.0e-3, 2.5e-4];
const SEEDS: [u64; 3] = [1008, 1016, 1032];

const PRIMARY_L0: &[u8] = include_bytes!(
    "../../../../verify/fluid/flow-past-cylinder-mesh-family-private/references/primary-l0.msh"
);
const PRIMARY_L1: &[u8] = include_bytes!(
    "../../../../verify/fluid/flow-past-cylinder-mesh-family-private/references/primary-l1.msh"
);
const PRIMARY_L2: &[u8] = include_bytes!(
    "../../../../verify/fluid/flow-past-cylinder-mesh-family-private/references/primary-l2.msh"
);
const BIAS_FINE: &[u8] = include_bytes!(
    "../../../../verify/fluid/flow-past-cylinder-mesh-family-private/references/bias-fine.msh"
);

#[derive(Clone, Copy)]
struct ExpectedMember {
    geometry: &'static str,
    mesh: &'static str,
    correspondence: &'static str,
    binding: &'static str,
    requested_bits: u64,
    accepted_bits: u64,
    segments: usize,
    chord_bits: u64,
    diameter_bits: u64,
}

const EXPECTED_PRIMARY: [ExpectedMember; 3] = [
    ExpectedMember {
        geometry: "da3217192cad68fbc9dd82aa35f77d6b63c5d7a69fe5678ead1ee0cc61316911",
        mesh: "262d6b83902c25b7d068d58c7f081d2238b3cdbb75147af8f65593519427bcbb",
        correspondence: "bf122bd5b049ec5aa79738e6327722914b6642dedcbd1b54cf087a236d0c1dc0",
        binding: "89b32e77c3066e459155372ecf6bbb6fc519752a2e76e1fac65ca0d3844de408",
        requested_bits: 0x3f70_624d_d2f1_a9fc,
        accepted_bits: 0x3f6f_2dcf_3d7b_22f3,
        segments: 8,
        chord_bits: 0x3fa3_97e8_8558_8783,
        diameter_bits: 0x3fc4_2271_e397_0b9d,
    },
    ExpectedMember {
        geometry: "029a57673a7264ec7d803eb28b49acb6e71dc4ca78b60cf0576aae749d63f78d",
        mesh: "1304faf5f703b35e27977385d8e054ebadc35a52853750d14fcd87d0e8197d8f",
        correspondence: "5808bb7678748430a46f53ba15ce03c2831a780ddab2fdb00d05b2ca8af5ae32",
        binding: "0dad71b5b57556e0f26f4e1537101a518d97b36ebc9426c33672b78ab02ff406",
        requested_bits: 0x3f50_624d_d2f1_a9fc,
        accepted_bits: 0x3f4f_7b3c_ce8f_dd4d,
        segments: 16,
        chord_bits: 0x3f93_fa2c_fd21_51a2,
        diameter_bits: 0x3fb7_fa5d_b221_d52c,
    },
    ExpectedMember {
        geometry: "cf573e2c14d103012fbc3b5ae4ec41de45d6f7bfff48be19be686c5255cfdef7",
        mesh: "b4ee2087da4c19158de51288eb1cdacffe403ab0ff21fcdabbb2c524a32c95af",
        correspondence: "450c489ad382a506187bccdea33885668a1517d5cc62e038f06dd2ca8601f0f6",
        binding: "ca3f50beaaa06426af6a931999cc97ab0d359c017d8b5153b4f771fdab954668",
        requested_bits: 0x3f30_624d_d2f1_a9fc,
        accepted_bits: 0x3f2f_8eb0_259f_1733,
        segments: 32,
        chord_bits: 0x3f84_12eb_c9ba_f68a,
        diameter_bits: 0x3fab_7b99_9f2c_2899,
    },
];

const EXPECTED_BIAS: ExpectedMember = ExpectedMember {
    geometry: "cf573e2c14d103012fbc3b5ae4ec41de45d6f7bfff48be19be686c5255cfdef7",
    mesh: "5be87ef2efa29948620991a7ab3517a4438e73d674299cbaf39d8f817f4aa514",
    correspondence: "b7f4489c96a0d6a4ab44ec146f597851fca132193fbd8338e664bd2e350cae40",
    binding: "2be8a29dcbb1ad4c8d0a36fa76da425acc4f14f86d9aee6247f0a3ab2f2657ef",
    requested_bits: 0x3f30_624d_d2f1_a9fc,
    accepted_bits: 0x3f2f_8eb0_259f_1733,
    segments: 32,
    chord_bits: 0x3f84_12eb_c9ba_f68a,
    diameter_bits: 0x3faa_6763_1da0_b0f5,
};

#[test]
fn ordinary_positive_imports_binds_replays_and_crosses_three_by_three() {
    run_independent_oracle();
    assert_eq!(
        FIXTURE_MINIMUM_MEAN_RATIO.to_bits(),
        FIXTURE_MINIMUM_MEAN_RATIO_BITS
    );
    let accepted = admit_family(ordinary_candidate()).expect("ordinary MESH0 family");
    accepted.revalidate().expect("complete family replay");
    assert_eq!(accepted.source().digest_bytes(), hex32(SOURCE_SHA256));
    assert_eq!(accepted.primary_members().len(), 3);
    assert_eq!(accepted.space_time_cells().len(), 9);
    assert_eq!(accepted.probe_inventory(), &exact_probe_inventory());
    for (member, expected) in accepted
        .primary_members()
        .iter()
        .zip(EXPECTED_PRIMARY.iter())
    {
        assert_member(member, expected);
    }
    assert_member(accepted.bias_member(), &EXPECTED_BIAS);
    assert_eq!(
        accepted
            .space_time_cells()
            .iter()
            .map(|cell| (cell.spatial_ordinal(), cell.time_ordinal()))
            .collect::<Vec<_>>(),
        vec![
            (0, 0),
            (0, 1),
            (0, 2),
            (1, 0),
            (1, 1),
            (1, 2),
            (2, 0),
            (2, 1),
            (2, 2),
        ]
    );
}

#[test]
fn s1_spatial_only_positive_has_no_time_family_or_cells() {
    let accepted = admit_family(s1_candidate()).expect("ordinary S1 spatial family");
    accepted.revalidate().expect("complete S1 family replay");
    assert_eq!(accepted.primary_members().len(), 3);
    assert!(accepted.time_family().is_none());
    assert!(accepted.space_time_cells().is_empty());
}

#[test]
fn fixed_polygon_and_fake_refinement_mutants_are_rejected_after_positive() {
    rejected_candidate("fixed polygon", |candidate| {
        candidate.primary[1] = candidate.primary[0].clone();
        candidate.primary[1].ordinal = 1;
        candidate.primary[1].provider_seed = SEEDS[1];
    });
    rejected_candidate("coarse member relabelled fine", |candidate| {
        candidate.primary[2].accepted = candidate.primary[0].accepted.clone();
    });
    rejected_candidate("cell count only", |candidate| {
        candidate.primary[1].cell_count = candidate.primary[0].cell_count + 1;
        candidate.primary[1].accepted = candidate.primary[0].accepted.clone();
    });
    rejected_candidate("reversed levels", |candidate| candidate.primary.reverse());
    rejected_candidate("nondecreasing chord", |candidate| {
        candidate.primary[1].max_cylinder_chord = candidate.primary[0].max_cylinder_chord;
    });
    rejected_candidate("nondecreasing triangle diameter", |candidate| {
        candidate.primary[1].max_triangle_diameter = candidate.primary[0].max_triangle_diameter;
    });
}

#[test]
fn lineage_design_and_primary_bias_swaps_are_rejected_after_positive() {
    rejected_candidate("foreign exact source", |candidate| {
        candidate.source = source([0.21, 0.2], exact_sets());
    });
    rejected_candidate("foreign mesh correspondence binding", |candidate| {
        candidate.primary[1].accepted = candidate.primary[0].accepted.clone();
    });
    rejected_candidate("stale correspondence", |candidate| {
        candidate.primary[1].correspondence = candidate.primary[0].correspondence.clone();
    });
    rejected_candidate("finest primary and bias swapped", |candidate| {
        std::mem::swap(&mut candidate.primary[2], &mut candidate.bias);
    });
}

#[test]
fn byte_index_and_signed_zero_aliases_cannot_fake_the_bias_family() {
    rejected_raw_bias("byte reencoding", reencode_ascii(PRIMARY_L2));
    rejected_raw_bias("index permutation", renumber_nodes(PRIMARY_L2));
    rejected_raw_bias("signed zero only", flip_one_zero_sign(PRIMARY_L2));
    let accepted = admit_family(ordinary_candidate()).expect("ordinary positive first");
    assert_ne!(
        accepted.primary_members()[2].canonical_topology(),
        accepted.bias_member().canonical_topology()
    );
}

#[test]
fn exact_source_names_and_probe_inventory_reject_every_substitution() {
    rejected_candidate("missing outlet", |candidate| {
        candidate.source = source(
            [0.2, 0.2],
            vec![
                named("fluid", FACE_DIMENSION, &[0]),
                named("walls", EDGE_DIMENSION, &[2, 3]),
                named("inlet", EDGE_DIMENSION, &[0]),
                named("cylinder", EDGE_DIMENSION, &[4]),
            ],
        );
    });
    rejected_candidate("overlapping boundary sets", |candidate| {
        candidate.source = source(
            [0.2, 0.2],
            vec![
                named("fluid", FACE_DIMENSION, &[0]),
                named("walls", EDGE_DIMENSION, &[0, 2, 3]),
                named("inlet", EDGE_DIMENSION, &[0]),
                named("outlet", EDGE_DIMENSION, &[1]),
                named("cylinder", EDGE_DIMENSION, &[4]),
            ],
        );
    });
    rejected_candidate("solver-side label authority", |candidate| {
        candidate.probes[0].source_boundary = "solver:cylinder".to_owned();
    });
    rejected_candidate("recentered cylinder", |candidate| {
        candidate.source = source([0.21, 0.2], exact_sets());
    });
    rejected_candidate("front rear swap", |candidate| candidate.probes.reverse());
    rejected_candidate("off-circle probe", |candidate| {
        candidate.probes[0].coordinate = [0.151, 0.2];
    });
    rejected_candidate("different on-circle pair", |candidate| {
        candidate.probes[0].coordinate = [0.2, 0.25];
        candidate.probes[0].eta_s = [0.0, 1.0];
        candidate.probes[1].coordinate = [0.2, 0.15];
        candidate.probes[1].eta_s = [0.0, -1.0];
    });
    rejected_candidate("normal reassignment", |candidate| {
        candidate.probes[0].eta_s = [1.0, 0.0];
    });
    rejected_candidate("nearest-node substitution", |candidate| {
        candidate.probes[0].coordinate = candidate.primary[0].nearest_mesh_vertex([0.15, 0.2]);
    });
}

#[test]
fn provider_metadata_and_import_boundary_reject_each_bad_input() {
    for (name, mutate) in [
        ("role drift", ProviderMutation::Role),
        ("name drift", ProviderMutation::Name),
        ("version drift", ProviderMutation::Version),
        ("executable drift", ProviderMutation::Executable),
        ("recipe drift", ProviderMutation::Recipe),
    ] {
        rejected_candidate(name, |candidate| {
            mutate.apply(&mut candidate.primary[1].provider)
        });
    }
    assert!(ProviderFamilyRole::parse("primary-ish").is_err());
    assert!(
        ProviderFamilyIdentity::new(
            ProviderFamilyRole::Primary,
            &"n".repeat(65),
            "4.13.1",
            &hex32(EXECUTABLE_SHA256),
            &hex32(PRIMARY_RECIPE_SHA256),
        )
        .is_err()
    );
    assert!(
        ProviderFamilyIdentity::new(
            ProviderFamilyRole::Primary,
            "gmsh",
            &"v".repeat(129),
            &hex32(EXECUTABLE_SHA256),
            &hex32(PRIMARY_RECIPE_SHA256),
        )
        .is_err()
    );
    assert!(
        ProviderFamilyIdentity::new(
            ProviderFamilyRole::Primary,
            "gmsh",
            "4.13.1",
            &[0; 31],
            &hex32(PRIMARY_RECIPE_SHA256),
        )
        .is_err()
    );
    assert!(
        ProviderFamilyIdentity::new(
            ProviderFamilyRole::Primary,
            "gmsh",
            "4.13.1",
            &hex32(EXECUTABLE_SHA256),
            &[0; 33],
        )
        .is_err()
    );

    let oversized = vec![b' '; 16 * 1024 * 1024 + 1];
    rejected_prepare("oversized MSH", primary_input(0, oversized));
    rejected_prepare(
        "physical-group membership",
        primary_input(0, add_physical_group(PRIMARY_L0)),
    );
    rejected_prepare(
        "nontriangle full-dimensional block",
        primary_input(0, change_first_triangle_type(PRIMARY_L0, 3)),
    );
    rejected_prepare(
        "nonfinite coordinate",
        primary_input(0, replace_first(PRIMARY_L0, b"0 0 0", b"nan 0 0")),
    );
    rejected_prepare(
        "reversed triangle",
        primary_input(0, reverse_first_triangle(PRIMARY_L0)),
    );
    rejected_prepare(
        "degenerate triangle",
        primary_input(0, degenerate_first_triangle(PRIMARY_L0)),
    );
    rejected_prepare(
        "overlapping topology",
        primary_input(0, duplicate_first_triangle(PRIMARY_L0)),
    );
    rejected_prepare(
        "quality failure",
        primary_input(0, collapse_one_vertex(PRIMARY_L0)),
    );
    rejected_prepare(
        "resource count excess",
        primary_input(0, exceed_node_header_limit(PRIMARY_L0)),
    );
    rejected_prepare(
        "omitted cylinder chord/uncovered frontier",
        primary_input(0, remove_first_triangle(PRIMARY_L0)),
    );
}

#[test]
fn time_identity_product_and_s1_prohibition_reject_every_alias() {
    rejected_candidate("short carrier", |candidate| {
        candidate.time_family.as_mut().unwrap().members[1].method = vec![0; 31];
    });
    rejected_candidate("extended carrier", |candidate| {
        candidate.time_family.as_mut().unwrap().members[1].method = vec![0; 33];
    });
    rejected_candidate("differently typed carrier", |candidate| {
        candidate.time_family.as_mut().unwrap().members[1].method =
            METHOD_SHA256.as_bytes().to_vec();
    });
    rejected_candidate("duplicate time ordinal", |candidate| {
        candidate.time_family.as_mut().unwrap().members[1].ordinal = 0;
    });
    rejected_candidate("reordered time ordinal", |candidate| {
        candidate.time_family.as_mut().unwrap().members.swap(0, 1);
    });
    rejected_candidate("mixed method identities", |candidate| {
        candidate.time_family.as_mut().unwrap().members[1].method = vec![0x11; 32];
    });
    rejected_candidate("nonfinite step", |candidate| {
        candidate.time_family.as_mut().unwrap().members[1].step = f64::INFINITY;
    });
    rejected_candidate("nonpositive step", |candidate| {
        candidate.time_family.as_mut().unwrap().members[1].step = 0.0;
    });
    rejected_candidate("nondecreasing step", |candidate| {
        candidate.time_family.as_mut().unwrap().members[1].step = 4.0;
    });
    rejected_candidate("missing Cartesian pair", |candidate| {
        candidate.space_time_cells.pop();
    });
    rejected_candidate("duplicate Cartesian pair", |candidate| {
        candidate.space_time_cells[8] = candidate.space_time_cells[0].clone();
    });
    rejected_candidate("diagonal relabelled full product", |candidate| {
        candidate.space_time_cells = vec![cell(0, 0), cell(1, 1), cell(2, 2)];
    });
    let accepted_s1 = admit_family(s1_candidate()).expect("ordinary S1 positive before leakage");
    accepted_s1.revalidate().expect("ordinary S1 replay");
    let mut leakage = ordinary_candidate();
    leakage.benchmark = CylinderBenchmark::S1;
    assert!(admit_family(leakage).is_err(), "S1 time leakage survived");
}

#[derive(Clone, Copy)]
enum ProviderMutation {
    Role,
    Name,
    Version,
    Executable,
    Recipe,
}

impl ProviderMutation {
    fn apply(self, provider: &mut ProviderFamilyIdentity) {
        match self {
            Self::Role => provider.family_role = ProviderFamilyRole::Bias,
            Self::Name => provider.generator_name = "gmsh-drift".to_owned(),
            Self::Version => provider.generator_exact_version = "4.13".to_owned(),
            Self::Executable => provider.generator_executable_sha256[0] ^= 1,
            Self::Recipe => provider.recipe_template_sha256[0] ^= 1,
        }
    }
}

fn ordinary_candidate() -> CylinderMeshFamilyInput {
    let source = exact_source();
    let primary = [PRIMARY_L0, PRIMARY_L1, PRIMARY_L2]
        .into_iter()
        .enumerate()
        .map(|(ordinal, bytes)| {
            PreparedCylinderMeshMember::from_input(&source, primary_input(ordinal, bytes.to_vec()))
                .expect("ordinary primary member")
        })
        .collect();
    let bias = PreparedCylinderMeshMember::from_input(
        &source,
        member_input(bias_provider(), 2, 2032, REQUESTS[2], BIAS_FINE.to_vec()),
    )
    .expect("ordinary bias member");
    CylinderMeshFamilyInput {
        benchmark: CylinderBenchmark::S2,
        source,
        primary,
        bias,
        probes: exact_probe_inventory(),
        time_family: Some(structural_time_family()),
        space_time_cells: (0..3)
            .flat_map(|space| (0..3).map(move |time| cell(space, time)))
            .collect(),
    }
}

fn s1_candidate() -> CylinderMeshFamilyInput {
    let mut candidate = ordinary_candidate();
    candidate.benchmark = CylinderBenchmark::S1;
    candidate.time_family = None;
    candidate.space_time_cells.clear();
    candidate
}

fn rejected_candidate(name: &str, mutate: impl FnOnce(&mut CylinderMeshFamilyInput)) {
    admit_family(ordinary_candidate())
        .and_then(|family| family.revalidate())
        .expect("ordinary positive reaches replay before every mutant");
    let mut candidate = ordinary_candidate();
    mutate(&mut candidate);
    assert!(admit_family(candidate).is_err(), "mutant survived: {name}");
}

fn rejected_prepare(name: &str, input: CylinderMeshMemberInput) {
    admit_family(ordinary_candidate())
        .and_then(|family| family.revalidate())
        .expect("ordinary positive reaches replay before every import mutant");
    assert!(
        PreparedCylinderMeshMember::from_input(&exact_source(), input).is_err(),
        "mutant survived: {name}"
    );
}

fn rejected_raw_bias(name: &str, bytes: Vec<u8>) {
    admit_family(ordinary_candidate())
        .and_then(|family| family.revalidate())
        .expect("ordinary positive reaches replay before every topology mutant");
    let mut candidate = ordinary_candidate();
    candidate.bias = PreparedCylinderMeshMember::from_input(
        &candidate.source,
        member_input(bias_provider(), 2, 2032, REQUESTS[2], bytes),
    )
    .expect("alias remains an otherwise valid imported member");
    assert!(admit_family(candidate).is_err(), "mutant survived: {name}");
}

fn primary_input(ordinal: usize, bytes: Vec<u8>) -> CylinderMeshMemberInput {
    member_input(
        primary_provider(),
        ordinal,
        SEEDS[ordinal],
        REQUESTS[ordinal],
        bytes,
    )
}

fn member_input(
    provider: ProviderFamilyIdentity,
    ordinal: usize,
    seed: u64,
    request: f64,
    bytes: Vec<u8>,
) -> CylinderMeshMemberInput {
    CylinderMeshMemberInput {
        provider,
        provider_seed: seed,
        ordinal,
        requested_max_boundary_error_m: request,
        max_segments: 64,
        msh_bytes: bytes,
    }
}

fn primary_provider() -> ProviderFamilyIdentity {
    ProviderFamilyIdentity::new(
        ProviderFamilyRole::Primary,
        "gmsh",
        "4.13.1",
        &hex32(EXECUTABLE_SHA256),
        &hex32(PRIMARY_RECIPE_SHA256),
    )
    .expect("primary provider identity")
}

fn bias_provider() -> ProviderFamilyIdentity {
    ProviderFamilyIdentity::new(
        ProviderFamilyRole::Bias,
        "gmsh",
        "4.13.1",
        &hex32(EXECUTABLE_SHA256),
        &hex32(BIAS_RECIPE_SHA256),
    )
    .expect("bias provider identity")
}

fn exact_source() -> CanonicalGeometryV1 {
    source([0.2, 0.2], exact_sets())
}

fn exact_sets() -> Vec<NamedEntitySet> {
    vec![
        named("fluid", FACE_DIMENSION, &[0]),
        named("walls", EDGE_DIMENSION, &[2, 3]),
        named("inlet", EDGE_DIMENSION, &[0]),
        named("cylinder", EDGE_DIMENSION, &[4]),
        named("outlet", EDGE_DIMENSION, &[1]),
    ]
}

fn source(center: [f64; 2], sets: Vec<NamedEntitySet>) -> CanonicalGeometryV1 {
    CanonicalGeometryV1::from_circular_hole([[0.0, 2.2], [0.0, 0.41]], center, 0.05, sets, 1.0e-12)
        .expect("structurally valid circular-hole source")
}

fn named(name: &str, dimension: usize, members: &[usize]) -> NamedEntitySet {
    NamedEntitySet::new(name, dimension, members.to_vec())
}

fn exact_probe_inventory() -> ProbeInventoryIdentity {
    ProbeInventoryIdentity::new([
        ProbeIdentity::new("front", "cylinder", [0.15, 0.2], [-1.0, 0.0]),
        ProbeIdentity::new("rear", "cylinder", [0.25, 0.2], [1.0, 0.0]),
    ])
    .expect("exact ordered DFG source probes")
}

fn structural_time_family() -> TimeFamilyInput {
    let method = hex32(METHOD_SHA256).to_vec();
    TimeFamilyInput {
        members: [4.0, 2.0, 1.0]
            .into_iter()
            .enumerate()
            .map(|(ordinal, step)| TimeMemberInput {
                ordinal,
                method: method.clone(),
                step,
            })
            .collect(),
    }
}

fn cell(spatial: usize, time: usize) -> SpaceTimeCellInput {
    SpaceTimeCellInput {
        spatial_ordinal: spatial,
        time_ordinal: time,
    }
}

fn assert_member(member: &AcceptedCylinderMeshMember, expected: &ExpectedMember) {
    member.revalidate().expect("member replay");
    assert_eq!(
        member
            .accepted()
            .mesh()
            .mesh()
            .quality_gate()
            .minimum_mean_ratio()
            .to_bits(),
        FIXTURE_MINIMUM_MEAN_RATIO_BITS
    );
    assert_eq!(
        member.accepted().requested_max_boundary_error_m().to_bits(),
        expected.requested_bits
    );
    assert_eq!(
        member.accepted().boundary_error_bound_m().to_bits(),
        expected.accepted_bits
    );
    assert_eq!(member.accepted().circle_segments(), expected.segments);
    assert_eq!(member.max_cylinder_chord().to_bits(), expected.chord_bits);
    assert_eq!(
        member.max_triangle_diameter().to_bits(),
        expected.diameter_bits
    );
    assert_digest(
        member.accepted().realized_geometry().digest().unwrap(),
        expected.geometry,
    );
    assert_digest(member.accepted().mesh().digest().unwrap(), expected.mesh);
    assert_digest(
        member.accepted().correspondence().digest().unwrap(),
        expected.correspondence,
    );
    assert_digest(
        member.accepted().envelope().digest().unwrap(),
        expected.binding,
    );
}

fn assert_digest(actual: ArtifactDigest, expected: &str) {
    assert_eq!(actual.to_string(), expected);
}

fn run_independent_oracle() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../verify/fluid/flow-past-cylinder-mesh-family-private/oracle.py");
    let output = Command::new("python3")
        .arg(path)
        .output()
        .expect("run independent stdlib oracle");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("oracle UTF-8");
    assert!(stdout.contains("ordinary_positive=PASS"));
    assert!(stdout.contains("scientific_values_checked=none"));
}

fn hex32(value: &str) -> [u8; 32] {
    assert_eq!(value.len(), 64);
    let mut result = [0; 32];
    for (index, byte) in result.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[2 * index..2 * index + 2], 16).unwrap();
    }
    result
}

fn replace_first(bytes: &[u8], needle: &[u8], replacement: &[u8]) -> Vec<u8> {
    let offset = bytes
        .windows(needle.len())
        .position(|item| item == needle)
        .unwrap();
    let mut result = Vec::with_capacity(bytes.len() - needle.len() + replacement.len());
    result.extend_from_slice(&bytes[..offset]);
    result.extend_from_slice(replacement);
    result.extend_from_slice(&bytes[offset + needle.len()..]);
    result
}

fn reencode_ascii(bytes: &[u8]) -> Vec<u8> {
    String::from_utf8(bytes.to_vec())
        .unwrap()
        .replace('\n', "\r\n")
        .into_bytes()
}

fn flip_one_zero_sign(bytes: &[u8]) -> Vec<u8> {
    replace_first(bytes, b"\n0 ", b"\n-0 ")
}

fn add_physical_group(bytes: &[u8]) -> Vec<u8> {
    let text = String::from_utf8(bytes.to_vec()).unwrap();
    let mut lines = text.lines().map(str::to_owned).collect::<Vec<_>>();
    let entities = lines.iter().position(|line| line == "$Entities").unwrap();
    let point = entities + 2;
    let mut fields = lines[point]
        .split_whitespace()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    assert_eq!(fields[4], "0");
    fields[4] = "1".to_owned();
    fields.push("99".to_owned());
    lines[point] = fields.join(" ");
    (lines.join("\n") + "\n").into_bytes()
}

fn renumber_nodes(bytes: &[u8]) -> Vec<u8> {
    let text = String::from_utf8(bytes.to_vec()).unwrap();
    let mut lines = text.lines().map(str::to_owned).collect::<Vec<_>>();
    let nodes = lines.iter().position(|line| line == "$Nodes").unwrap();
    let node_end = lines.iter().position(|line| line == "$EndNodes").unwrap();
    let block_count = lines[nodes + 1]
        .split_whitespace()
        .next()
        .unwrap()
        .parse::<usize>()
        .unwrap();
    let mut cursor = nodes + 2;
    let mut mapping = HashMap::new();
    for _ in 0..block_count {
        let count = lines[cursor]
            .split_whitespace()
            .last()
            .unwrap()
            .parse::<usize>()
            .unwrap();
        cursor += 1;
        for line in &mut lines[cursor..cursor + count] {
            let old = line.parse::<u64>().unwrap();
            let new = old + 10_000;
            mapping.insert(old, new);
            *line = new.to_string();
        }
        cursor += 2 * count;
    }
    assert_eq!(cursor, node_end);
    let mut header = lines[nodes + 1]
        .split_whitespace()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    header[2] = (header[2].parse::<u64>().unwrap() + 10_000).to_string();
    header[3] = (header[3].parse::<u64>().unwrap() + 10_000).to_string();
    lines[nodes + 1] = header.join(" ");

    let elements = lines.iter().position(|line| line == "$Elements").unwrap();
    let block_count = lines[elements + 1]
        .split_whitespace()
        .next()
        .unwrap()
        .parse::<usize>()
        .unwrap();
    cursor = elements + 2;
    for _ in 0..block_count {
        let block = lines[cursor]
            .split_whitespace()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        let element_type = block[2].parse::<usize>().unwrap();
        let count = block[3].parse::<usize>().unwrap();
        cursor += 1;
        let arity = match element_type {
            15 => 1,
            1 => 2,
            2 => 3,
            _ => panic!("fixture type"),
        };
        for line in &mut lines[cursor..cursor + count] {
            let mut fields = line
                .split_whitespace()
                .map(str::to_owned)
                .collect::<Vec<_>>();
            for field in &mut fields[1..=arity] {
                *field = mapping[&field.parse::<u64>().unwrap()].to_string();
            }
            *line = fields.join(" ");
        }
        cursor += count;
    }
    (lines.join("\n") + "\n").into_bytes()
}

fn mutate_first_triangle(bytes: &[u8], update: impl FnOnce(&mut Vec<String>)) -> Vec<u8> {
    let text = String::from_utf8(bytes.to_vec()).unwrap();
    let mut lines = text.lines().map(str::to_owned).collect::<Vec<_>>();
    let elements = lines.iter().position(|line| line == "$Elements").unwrap();
    let mut cursor = elements + 2;
    loop {
        let block = lines[cursor]
            .split_whitespace()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        let element_type = block[2].parse::<usize>().unwrap();
        let count = block[3].parse::<usize>().unwrap();
        cursor += 1;
        if element_type == 2 {
            let mut triangle = lines[cursor]
                .split_whitespace()
                .map(str::to_owned)
                .collect();
            update(&mut triangle);
            lines[cursor] = triangle.join(" ");
            return (lines.join("\n") + "\n").into_bytes();
        }
        cursor += count;
    }
}

fn reverse_first_triangle(bytes: &[u8]) -> Vec<u8> {
    mutate_first_triangle(bytes, |triangle| triangle.swap(2, 3))
}

fn degenerate_first_triangle(bytes: &[u8]) -> Vec<u8> {
    mutate_first_triangle(bytes, |triangle| triangle[3] = triangle[2].clone())
}

fn duplicate_first_triangle(bytes: &[u8]) -> Vec<u8> {
    let text = String::from_utf8(bytes.to_vec()).unwrap();
    let mut lines = text.lines().map(str::to_owned).collect::<Vec<_>>();
    let first = first_triangle_line(&lines);
    let second = lines[first + 1]
        .split_whitespace()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let mut duplicate = lines[first]
        .split_whitespace()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    duplicate[1..4].clone_from_slice(&second[1..4]);
    lines[first] = duplicate.join(" ");
    (lines.join("\n") + "\n").into_bytes()
}

fn collapse_one_vertex(bytes: &[u8]) -> Vec<u8> {
    let text = String::from_utf8(bytes.to_vec()).unwrap();
    let mut lines = text.lines().map(str::to_owned).collect::<Vec<_>>();
    let first = first_triangle_line(&lines);
    let triangle = lines[first]
        .split_whitespace()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let locations = node_coordinate_lines(&lines);
    let near = locations[&triangle[2].parse::<u64>().unwrap()];
    let moved = locations[&triangle[3].parse::<u64>().unwrap()];
    let coordinate = lines[near]
        .split_whitespace()
        .map(str::parse::<f64>)
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    lines[moved] = format!("{} {} 0", coordinate[0] + 1.0e-14, coordinate[1]);
    (lines.join("\n") + "\n").into_bytes()
}

fn exceed_node_header_limit(bytes: &[u8]) -> Vec<u8> {
    let text = String::from_utf8(bytes.to_vec()).unwrap();
    let mut lines = text.lines().map(str::to_owned).collect::<Vec<_>>();
    let nodes = lines.iter().position(|line| line == "$Nodes").unwrap();
    let mut header = lines[nodes + 1]
        .split_whitespace()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    header[1] = "1000001".to_owned();
    header[3] = "1000001".to_owned();
    lines[nodes + 1] = header.join(" ");
    (lines.join("\n") + "\n").into_bytes()
}

fn remove_first_triangle(bytes: &[u8]) -> Vec<u8> {
    let text = String::from_utf8(bytes.to_vec()).unwrap();
    let mut lines = text.lines().map(str::to_owned).collect::<Vec<_>>();
    let elements = lines.iter().position(|line| line == "$Elements").unwrap();
    let first = first_triangle_line(&lines);
    let block = first - 1;
    let mut block_header = lines[block]
        .split_whitespace()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    block_header[3] = (block_header[3].parse::<usize>().unwrap() - 1).to_string();
    lines[block] = block_header.join(" ");
    let mut total = lines[elements + 1]
        .split_whitespace()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    total[1] = (total[1].parse::<usize>().unwrap() - 1).to_string();
    lines[elements + 1] = total.join(" ");
    lines.remove(first);
    (lines.join("\n") + "\n").into_bytes()
}

fn change_first_triangle_type(bytes: &[u8], element_type: usize) -> Vec<u8> {
    let text = String::from_utf8(bytes.to_vec()).unwrap();
    let mut lines = text.lines().map(str::to_owned).collect::<Vec<_>>();
    let block = first_triangle_line(&lines) - 1;
    let mut header = lines[block]
        .split_whitespace()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    header[2] = element_type.to_string();
    lines[block] = header.join(" ");
    (lines.join("\n") + "\n").into_bytes()
}

fn first_triangle_line(lines: &[String]) -> usize {
    let elements = lines.iter().position(|line| line == "$Elements").unwrap();
    let mut cursor = elements + 2;
    loop {
        let block = lines[cursor].split_whitespace().collect::<Vec<_>>();
        let element_type = block[2].parse::<usize>().unwrap();
        let count = block[3].parse::<usize>().unwrap();
        cursor += 1;
        if element_type == 2 {
            return cursor;
        }
        cursor += count;
    }
}

fn node_coordinate_lines(lines: &[String]) -> HashMap<u64, usize> {
    let nodes = lines.iter().position(|line| line == "$Nodes").unwrap();
    let block_count = lines[nodes + 1]
        .split_whitespace()
        .next()
        .unwrap()
        .parse::<usize>()
        .unwrap();
    let mut cursor = nodes + 2;
    let mut result = HashMap::new();
    for _ in 0..block_count {
        let count = lines[cursor]
            .split_whitespace()
            .last()
            .unwrap()
            .parse::<usize>()
            .unwrap();
        cursor += 1;
        let tags = lines[cursor..cursor + count]
            .iter()
            .map(|line| line.parse::<u64>().unwrap())
            .collect::<Vec<_>>();
        cursor += count;
        for (offset, tag) in tags.into_iter().enumerate() {
            result.insert(tag, cursor + offset);
        }
        cursor += count;
    }
    result
}
