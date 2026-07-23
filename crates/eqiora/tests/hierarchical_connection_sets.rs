use eqiora::Span;
use eqiora::artifact::ModelEnvelopeV2;
use eqiora::compiler::connection_sets::{
    ConnectionFragment, ConnectionSetError, ConnectionSetLimits, normalize_connection_sets,
};
use eqiora::compiler::identity::{
    DeclarationPath, ElaborationIdentityLimits, ElaborationKey, FullElaborationIdentity,
    IdentityNamespace, InstancePath,
};
use eqiora::compiler::projection::PhysicalExposureContract;
use eqiora::compiler::provenance::{ElaborationSourceOrigin, ProvenanceBuilder, ProvenanceLimits};
use eqiora::compiler::{CompiledModel, compile};
use eqiora::diagnostic::codes;
use eqiora::graph::{GraphStore, InMemoryGraphStore};
use eqiora::kernel::KernelNode;
use eqiora::sem::KernelProgram;

mod support;

use support::connection_set_conformance::{observe_connection_sets, require_diagnostic};

const NARY: &str =
    include_str!("../../../verify/language/hierarchical-connection-sets/models/nary.eqi");
const CHAIN: &str =
    include_str!("../../../verify/language/hierarchical-connection-sets/models/chain.eqi");
const WRAPPER_EXPOSURE: &str = include_str!(
    "../../../verify/language/hierarchical-connection-sets/models/wrapper-exposure.eqi"
);
const TWO_LEVEL_FORWARDING: &str = include_str!(
    "../../../verify/language/hierarchical-connection-sets/models/two-level-forwarding.eqi"
);
const DISTINCT_EXPOSURE_CUTS: &str = include_str!(
    "../../../verify/language/hierarchical-connection-sets/models/distinct-exposure-cuts.eqi"
);
const DISJOINT: &str =
    include_str!("../../../verify/language/hierarchical-connection-sets/models/disjoint.eqi");
const JOINED: &str =
    include_str!("../../../verify/language/hierarchical-connection-sets/models/joined.eqi");
const FLAT_NARY: &str =
    include_str!("../../../verify/language/hierarchical-connection-sets/models/flat-nary.eqi");
const FLAT_CHAIN: &str =
    include_str!("../../../verify/language/hierarchical-connection-sets/models/flat-chain.eqi");
const INVALID_MISSING_MEMBERSHIP: &str = include_str!(
    "../../../verify/language/hierarchical-connection-sets/models/invalid-missing-membership.eqi"
);
const INVALID_DOUBLE_OWNER: &str = include_str!(
    "../../../verify/language/hierarchical-connection-sets/models/invalid-double-owner.eqi"
);
const INVALID_OWNERLESS_SET: &str = include_str!(
    "../../../verify/language/hierarchical-connection-sets/models/invalid-ownerless-set.eqi"
);
const INVALID_IMPLICIT_GRANDCHILD: &str = include_str!(
    "../../../verify/language/hierarchical-connection-sets/models/invalid-implicit-grandchild.eqi"
);
const OWNED_PUBLIC_FORWARDING: &str = include_str!(
    "../../../verify/language/hierarchical-connection-sets/models/owned-public-forwarding.eqi"
);

const THREE_PERMUTATIONS: [[&str; 3]; 6] = [
    ["a", "b", "c"],
    ["a", "c", "b"],
    ["b", "a", "c"],
    ["b", "c", "a"],
    ["c", "a", "b"],
    ["c", "b", "a"],
];

fn compile_one(file: &str, source: &str) -> CompiledModel {
    let mut compiled = compile(file, source).expect("verification fixture compiles");
    assert_eq!(compiled.len(), 1, "fixture has one root Model");
    compiled.pop().unwrap()
}

fn admit(compiled: CompiledModel) -> KernelProgram {
    let model = compiled.model();
    let (transaction, _, _) = compiled.into_parts();
    let mut store = InMemoryGraphStore::new();
    store.commit(transaction).expect("atomic model admission");
    KernelProgram::from_snapshot(&store.snapshot(), model).expect("accepted canonical program")
}

fn sole_connection_set(
    program: &KernelProgram,
) -> support::connection_set_conformance::ConnectionSetObservation {
    let observations = observe_connection_sets(program);
    assert_eq!(observations.len(), 1, "one maximal physical set");
    observations.into_iter().next().unwrap()
}

fn canonical_program(compiled: CompiledModel) -> Vec<u8> {
    let program = admit(compiled);
    ModelEnvelopeV2::from_program(&program)
        .unwrap()
        .canonical_json()
        .unwrap()
}

fn terminal_network(instances: [&str; 3], fragments: &[Vec<&str>]) -> String {
    let instances = instances
        .into_iter()
        .map(|name| format!("  instance {name}: Terminal;"))
        .collect::<Vec<_>>()
        .join("\n");
    let fragments = fragments
        .iter()
        .map(|members| {
            let ports = members
                .iter()
                .map(|member| format!("{member}.p"))
                .collect::<Vec<_>>()
                .join(", ");
            format!("  connect conserving {ports};")
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "connector Pin = scalar_physical(across = 1, through = 1);\n\
         component Terminal {{\n\
           public port p: conserving on Pin;\n\
           relation owner continuous {{ across(p) = 0; }}\n\
         }}\n\
         model Network {{\n{instances}\n{fragments}\n}}\n"
    )
}

fn span(file: &str, start: u32) -> Span {
    Span {
        file: file.to_owned(),
        start,
        end: start + 1,
    }
}

#[test]
fn nary_and_chained_fragments_have_one_canonical_model() {
    let nary = compile_one("nary.eqi", NARY);
    let chain = compile_one("chain.eqi", CHAIN);

    assert_eq!(nary.model(), chain.model());
    assert_eq!(nary.symbols(), chain.symbols());
    assert_eq!(nary.transaction().label(), chain.transaction().label());
    assert_eq!(nary.transaction().ops(), chain.transaction().ops());
    assert_eq!(
        nary.transaction().preconditions(),
        chain.transaction().preconditions()
    );

    let nary_provenance = nary.provenance().unwrap().clone();
    let chain_provenance = chain.provenance().unwrap().clone();
    let nary_program = admit(nary);
    let chain_program = admit(chain);
    let nary_set = sole_connection_set(&nary_program);
    let chain_set = sole_connection_set(&chain_program);
    assert_eq!(nary_set, chain_set);
    assert_eq!(nary_set.members.len(), 3);

    assert_eq!(
        nary_provenance
            .get_by_graph_id(nary_set.connection)
            .unwrap()
            .origins()
            .len(),
        1
    );
    assert_eq!(
        chain_provenance
            .get_by_graph_id(chain_set.connection)
            .unwrap()
            .origins()
            .len(),
        2
    );

    let nary_artifact = ModelEnvelopeV2::from_program(&nary_program).unwrap();
    let chain_artifact = ModelEnvelopeV2::from_program(&chain_program).unwrap();
    assert_eq!(
        nary_artifact.canonical_json().unwrap(),
        chain_artifact.canonical_json().unwrap()
    );
    assert_eq!(
        nary_artifact.digest().unwrap(),
        chain_artifact.digest().unwrap()
    );
}

#[test]
fn declaration_fragment_and_member_permutations_preserve_the_exact_model() {
    let baseline = compile_one("baseline.eqi", NARY);
    let baseline_model = baseline.model();
    let baseline_symbols = baseline
        .symbols()
        .iter()
        .map(|(name, id)| (name.to_owned(), id))
        .collect::<Vec<_>>();
    let baseline_ops = baseline.transaction().ops().to_vec();
    let baseline_bytes = canonical_program(baseline);

    for instances in THREE_PERMUTATIONS {
        for members in THREE_PERMUTATIONS {
            let source = terminal_network(instances, &[members.to_vec()]);
            let compiled = compile_one("nary-permutation.eqi", &source);
            assert_eq!(compiled.model(), baseline_model);
            assert_eq!(
                compiled
                    .symbols()
                    .iter()
                    .map(|(name, id)| (name.to_owned(), id))
                    .collect::<Vec<_>>(),
                baseline_symbols
            );
            assert_eq!(compiled.transaction().ops(), baseline_ops);
            assert_eq!(canonical_program(compiled), baseline_bytes);
        }

        for fragments in [
            [["a", "b"], ["b", "c"]],
            [["b", "a"], ["b", "c"]],
            [["a", "b"], ["c", "b"]],
            [["b", "a"], ["c", "b"]],
            [["b", "c"], ["a", "b"]],
            [["b", "c"], ["b", "a"]],
            [["c", "b"], ["a", "b"]],
            [["c", "b"], ["b", "a"]],
        ] {
            let fragments = fragments
                .into_iter()
                .map(|members| members.to_vec())
                .collect::<Vec<_>>();
            let source = terminal_network(instances, &fragments);
            let compiled = compile_one("chain-permutation.eqi", &source);
            assert_eq!(compiled.model(), baseline_model);
            assert_eq!(
                compiled
                    .symbols()
                    .iter()
                    .map(|(name, id)| (name.to_owned(), id))
                    .collect::<Vec<_>>(),
                baseline_symbols
            );
            assert_eq!(compiled.transaction().ops(), baseline_ops);
            assert_eq!(canonical_program(compiled), baseline_bytes);
        }
    }
}

#[test]
fn two_levels_of_explicit_forwarding_form_one_set_without_alias_entities() {
    let compiled = compile_one("two-level-forwarding.eqi", TWO_LEVEL_FORWARDING);
    assert_eq!(compiled.symbols().get("left.p"), None);
    assert_eq!(compiled.symbols().get("left.inner.p"), None);
    let internal = compiled
        .symbols()
        .get("left.inner.leaf.p")
        .expect("owned innermost Leaf Port");
    let external = compiled
        .symbols()
        .get("right.p")
        .expect("owned external Leaf Port");
    let provenance = compiled.provenance().unwrap().clone();
    let projections = compiled.physical_exposures().clone();
    assert_eq!(
        projections
            .iter()
            .map(|projection| projection.selector())
            .collect::<Vec<_>>(),
        ["left.inner.p", "left.p"]
    );
    for projection in projections.iter() {
        assert_eq!(projection.interior().len(), 1);
        assert_eq!(projection.interior()[0].id().erase(), internal);
        assert!(provenance.get(projection.exposure()).is_some());
    }
    assert_ne!(
        projections.get("left.inner.p").unwrap().exposure(),
        projections.get("left.p").unwrap().exposure()
    );

    let program = admit(compiled);
    let connection = sole_connection_set(&program);
    assert_eq!(
        connection.members,
        [internal, external].into_iter().collect()
    );
    assert_eq!(
        program
            .nodes()
            .filter(|node| matches!(node, KernelNode::Port(_)))
            .count(),
        2
    );
    assert_eq!(
        provenance
            .get_by_graph_id(connection.connection)
            .unwrap()
            .origins()
            .len(),
        3,
        "inner, outer, and root fragments are complete provenance origins"
    );
    assert!(
        projections
            .iter()
            .all(|projection| { projection.connection().id().erase() == connection.connection })
    );
}

#[test]
fn independent_occurrences_remain_disjoint_until_an_explicit_fragment_joins_them() {
    let disjoint = compile_one("disjoint.eqi", DISJOINT);
    assert_ne!(disjoint.symbols().get("a.p"), disjoint.symbols().get("c.p"));
    let disjoint_sets = observe_connection_sets(&admit(disjoint));
    assert_eq!(disjoint_sets.len(), 2);
    assert!(disjoint_sets.iter().all(|set| set.members.len() == 2));

    let joined = compile_one("joined.eqi", JOINED);
    let joined_set = sole_connection_set(&admit(joined));
    assert_eq!(joined_set.members.len(), 4);
}

#[test]
fn sibling_exposures_retain_distinct_internal_cuts_after_external_union() {
    let compiled = compile_one("distinct-exposure-cuts.eqi", DISTINCT_EXPOSURE_CUTS);
    let left = compiled.symbols().get("pair.left.p").unwrap();
    let right = compiled.symbols().get("pair.right.p").unwrap();
    let outside = compiled.symbols().get("outside.p").unwrap();
    let projections = compiled.physical_exposures();
    assert_eq!(projections.len(), 2);
    let p = projections.get("pair.p").unwrap();
    let q = projections.get("pair.q").unwrap();
    assert_eq!(p.interior().len(), 1);
    assert_eq!(q.interior().len(), 1);
    assert_eq!(p.interior()[0].id().erase(), left);
    assert_eq!(q.interior()[0].id().erase(), right);
    assert_ne!(p.exposure(), q.exposure());
    assert_eq!(p.connection(), q.connection());
    assert!(
        projections
            .iter()
            .flat_map(|projection| projection.interior())
            .all(|member| member.id().erase() != outside)
    );
}

#[test]
fn duplicate_fragments_are_idempotent_but_duplicate_members_are_rejected() {
    let duplicate_source = DISJOINT.replace(
        "  connect conserving a.p, b.p;",
        "  connect conserving a.p, b.p;\n  connect conserving b.p, a.p;",
    );
    let single = compile_one("single.eqi", DISJOINT);
    let duplicate = compile_one("duplicate.eqi", &duplicate_source);
    assert_eq!(single.model(), duplicate.model());
    assert_eq!(single.symbols(), duplicate.symbols());
    assert_eq!(single.transaction().ops(), duplicate.transaction().ops());
    let duplicated_member = duplicate.symbols().get("a.p").unwrap();
    let duplicate_provenance = duplicate.provenance().unwrap().clone();
    let duplicate_connection = observe_connection_sets(&admit(duplicate))
        .into_iter()
        .find(|set| set.members.contains(&duplicated_member))
        .expect("duplicated fragment remains one canonical set")
        .connection;
    assert_eq!(
        duplicate_provenance
            .get_by_graph_id(duplicate_connection)
            .unwrap()
            .origins()
            .len(),
        2,
        "duplicate topology claims retain both complete source origins"
    );
    let duplicate = compile_one("duplicate.eqi", &duplicate_source);
    assert_eq!(canonical_program(single), canonical_program(duplicate));

    let duplicate_member = DISJOINT.replace(
        "  connect conserving a.p, b.p;",
        "  connect conserving a.p, a.p;",
    );
    let diagnostics =
        compile("duplicate-member.eqi", &duplicate_member).expect_err("member repeats");
    require_diagnostic(
        &diagnostics,
        codes::LANGUAGE_LOWERING_ERROR,
        "repeats a member",
    );
}

#[test]
fn ownerless_wrapper_port_is_not_fabricated_as_a_kernel_unknown_or_alias() {
    let compiled = compile_one("wrapper-exposure.eqi", WRAPPER_EXPOSURE);
    assert_eq!(compiled.symbols().get("left.p"), None);
    let internal = compiled
        .symbols()
        .get("left.leaf.p")
        .expect("owned internal Leaf Port");
    let external = compiled
        .symbols()
        .get("right.p")
        .expect("owned external Leaf Port");
    let provenance = compiled.provenance().unwrap().clone();
    let projections = compiled.physical_exposures().clone();
    let projection = projections.get("left.p").unwrap();
    assert_eq!(projections.len(), 1);
    assert_eq!(projection.interior().len(), 1);
    assert_eq!(projection.interior()[0].id().erase(), internal);
    assert_ne!(projection.interior()[0].id().erase(), external);
    assert!(provenance.get(projection.exposure()).is_some());
    let PhysicalExposureContract::ScalarPhysical { connector } = projection.contract() else {
        panic!("wrapper exposure retains the exact scalar connector");
    };
    assert_eq!(
        connector.id().erase(),
        compiled.symbols().get("connector::Pin").unwrap()
    );

    let program = admit(compiled);
    let connection = sole_connection_set(&program);
    assert_eq!(
        connection.members,
        [internal, external].into_iter().collect()
    );
    assert_eq!(
        program
            .nodes()
            .filter(|node| matches!(node, KernelNode::Port(_)))
            .count(),
        2,
        "the exposure does not become a third physical unknown"
    );
    assert_eq!(
        provenance
            .get_by_graph_id(connection.connection)
            .unwrap()
            .origins()
            .len(),
        2,
        "both explicit fragments remain provenance witnesses"
    );
    assert_eq!(projection.connection().id().erase(), connection.connection);
}

#[test]
fn relation_owned_public_port_remains_an_endpoint_in_the_maximal_set() {
    let compiled = compile_one("owned-public-forwarding.eqi", OWNED_PUBLIC_FORWARDING);
    let public = compiled
        .symbols()
        .get("left.p")
        .expect("Relation-owned public Port remains a semantic endpoint");
    let internal = compiled.symbols().get("left.leaf.p").unwrap();
    let external = compiled.symbols().get("right.p").unwrap();

    assert_eq!(
        sole_connection_set(&admit(compiled)).members,
        [public, internal, external].into_iter().collect()
    );
}

#[test]
fn direct_flat_fragments_enter_the_same_pre_kernel_normalizer() {
    for (file, source) in [("flat-nary.eqi", FLAT_NARY), ("flat-chain.eqi", FLAT_CHAIN)] {
        let compiled = compile_one(file, source);
        let expected_members = ["a", "b", "c"]
            .into_iter()
            .map(|name| compiled.symbols().get(name).unwrap())
            .collect();
        let program = admit(compiled);
        let connection = sole_connection_set(&program);
        assert_eq!(connection.members, expected_members);
    }
}

#[test]
fn invalid_physical_owner_and_membership_claims_fail_before_transaction_exposure() {
    for (file, source, expected) in [
        (
            "invalid-missing-membership.eqi",
            INVALID_MISSING_MEMBERSHIP,
            "conserving Connection membership",
        ),
        (
            "invalid-double-owner.eqi",
            INVALID_DOUBLE_OWNER,
            "more than one owning Relation",
        ),
        (
            "invalid-ownerless-set.eqi",
            INVALID_OWNERLESS_SET,
            "retains 0 members",
        ),
        (
            "invalid-implicit-grandchild.eqi",
            INVALID_IMPLICIT_GRANDCHILD,
            "retains 1 members",
        ),
    ] {
        let diagnostics = compile(file, source).expect_err("invalid source must expose no model");
        require_diagnostic(&diagnostics, codes::LANGUAGE_TYPE_ERROR, expected);
    }
}

#[test]
fn nominal_physical_types_and_signal_connections_never_enter_the_union() {
    let nominal_mismatch = r#"
connector LeftPin = scalar_physical(across = 1, through = 1);
connector RightPin = scalar_physical(across = 1, through = 1);
component Left {
  public port p: conserving on LeftPin;
  relation owner continuous { across(p) = 0; }
}
component Right {
  public port p: conserving on RightPin;
  relation owner continuous { across(p) = 0; }
}
model Network {
  instance left: Left;
  instance right: Right;
  connect conserving left.p, right.p;
}
"#;
    let diagnostics = compile("nominal-mismatch.eqi", nominal_mismatch)
        .expect_err("equal dimensions cannot erase nominal Connector identity");
    require_diagnostic(
        &diagnostics,
        codes::LANGUAGE_TYPE_ERROR,
        "exact same nominal Connector or Domain",
    );

    let signal = r#"
model SignalFanout {
  port source: signal output 1;
  port left: signal input 1;
  port right: signal input 1;
  relation sinks continuous { left - source = 0; right - source = 0; }
  connect signal source -> left, right;
}
"#;
    let compiled = compile_one("signal-fanout.eqi", signal);
    let program = admit(compiled);
    assert_eq!(
        program
            .nodes()
            .filter(|node| matches!(node, KernelNode::Connection(_)))
            .count(),
        1
    );
}

#[test]
fn topology_identity_and_provenance_resources_fail_closed_independently() {
    let default = ConnectionSetLimits::default();
    assert!(matches!(
        ConnectionFragment::try_new(
            [1_u32, 2],
            ConnectionSetLimits {
                max_members_per_fragment: 1,
                ..default
            }
        ),
        Err(ConnectionSetError::LimitExceeded {
            resource: "members in one connection fragment",
            ..
        })
    ));
    let fragment = |members| ConnectionFragment::try_new(members, default).unwrap();
    let joined = [fragment([1_u32, 2]), fragment([2, 3])];
    for (limits, expected) in [
        (
            ConnectionSetLimits {
                max_fragments: 1,
                ..default
            },
            "connection fragments",
        ),
        (
            ConnectionSetLimits {
                max_memberships: 3,
                ..default
            },
            "connection fragment memberships",
        ),
        (
            ConnectionSetLimits {
                max_endpoints: 2,
                ..default
            },
            "distinct connection endpoints",
        ),
        (
            ConnectionSetLimits {
                max_sets: 0,
                ..default
            },
            "normalized connection sets",
        ),
        (
            ConnectionSetLimits {
                max_members_per_set: 2,
                ..default
            },
            "members in one normalized connection set",
        ),
    ] {
        assert!(matches!(
            normalize_connection_sets(&joined, limits),
            Err(ConnectionSetError::LimitExceeded { resource, .. }) if resource == expected
        ));
    }

    let identities = [
        FullElaborationIdentity::from_sha256([1; 32]),
        FullElaborationIdentity::from_sha256([2; 32]),
    ];
    let identity_limits = ElaborationIdentityLimits {
        max_anonymous_connection_members: 1,
        ..ElaborationIdentityLimits::default()
    };
    assert!(
        ElaborationKey::anonymous_connection_with_limits(
            IdentityNamespace::new(["root"]).unwrap(),
            InstancePath::new(["Network"]).unwrap(),
            DeclarationPath::new(["model", "Network", "net"]).unwrap(),
            identities,
            identity_limits,
        )
        .is_err()
    );
    let path_limits = ElaborationIdentityLimits {
        max_path_bytes: 3,
        ..ElaborationIdentityLimits::default()
    };
    assert!(IdentityNamespace::with_limits(["root"], path_limits).is_err());
    let key_limits = ElaborationIdentityLimits {
        max_canonical_key_bytes: 1,
        ..ElaborationIdentityLimits::default()
    };
    assert!(
        ElaborationKey::anonymous_connection_with_limits(
            IdentityNamespace::new(["root"]).unwrap(),
            InstancePath::new(["Network"]).unwrap(),
            DeclarationPath::new(["model", "Network", "net"]).unwrap(),
            identities,
            key_limits,
        )
        .is_err()
    );

    let provenance_limits = ProvenanceLimits {
        max_entries: 1,
        max_origins_per_entry: 1,
        max_binding_spans_per_entry: 1,
        max_source_path_bytes: 8,
        max_total_source_path_bytes: 24,
    };
    let mut too_many_origins = ProvenanceBuilder::with_limits(provenance_limits);
    assert!(
        too_many_origins
            .insert_origins(
                identities[0],
                [
                    ElaborationSourceOrigin::new(
                        span("def.eqi", 0),
                        span("use.eqi", 0),
                        Vec::new(),
                    ),
                    ElaborationSourceOrigin::new(
                        span("def.eqi", 1),
                        span("use.eqi", 0),
                        Vec::new(),
                    ),
                ],
            )
            .is_err()
    );
    let mut path_too_long = ProvenanceBuilder::with_limits(provenance_limits);
    assert!(
        path_too_long
            .insert(
                identities[0],
                span("definition.eqi", 0),
                span("use.eqi", 0),
                [],
            )
            .is_err()
    );
}
