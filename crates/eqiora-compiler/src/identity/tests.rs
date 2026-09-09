use eqiora_core::entity::kinds;

use super::*;

fn namespace() -> IdentityNamespace {
    IdentityNamespace::new(["org", "components", "Resistor"]).unwrap()
}

fn instance(name: &str) -> InstancePath {
    InstancePath::new(["plant", name]).unwrap()
}

fn declaration(name: &str) -> DeclarationPath {
    DeclarationPath::new(["private", name]).unwrap()
}

fn entity_key(instance_name: &str, declaration_name: &str, kind: EntityKind) -> ElaborationKey {
    ElaborationKey::entity(
        namespace(),
        instance(instance_name),
        declaration(declaration_name),
        kind,
    )
    .unwrap()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[test]
fn canonical_encoding_and_full_digest_are_golden() {
    let key = entity_key("r1", "voltage", EntityKind::Field);

    assert_eq!(
        hex(&key.canonical_bytes().unwrap()),
        concat!(
            "4551494f5241454b0001",
            "01000000230003000000036f72670000000a636f6d706f6e656e7473000000085265736973746f72",
            "0200000011000200000005706c616e74000000027231",
            "03000000180002000000077072697661746500000007766f6c74616765",
            "0400000003000003"
        )
    );
    assert_eq!(
        key.full_identity().unwrap().to_string(),
        "990233f65afc07e15fa2fb899081e1d19385d4764a09ae61d49858ba391114cb"
    );
}

#[test]
fn namespace_instance_declaration_and_kind_are_separate_domains() {
    let base = entity_key("r1", "voltage", EntityKind::Field)
        .full_identity()
        .unwrap();
    let other_namespace = ElaborationKey::entity(
        IdentityNamespace::new(["org", "other", "Resistor"]).unwrap(),
        instance("r1"),
        declaration("voltage"),
        EntityKind::Field,
    )
    .unwrap()
    .full_identity()
    .unwrap();
    let other_instance = entity_key("r2", "voltage", EntityKind::Field)
        .full_identity()
        .unwrap();
    let other_declaration = entity_key("r1", "current", EntityKind::Field)
        .full_identity()
        .unwrap();
    let other_kind = entity_key("r1", "voltage", EntityKind::Parameter)
        .full_identity()
        .unwrap();

    assert_ne!(base, other_namespace);
    assert_ne!(base, other_instance);
    assert_ne!(base, other_declaration);
    assert_ne!(base, other_kind);
}

#[test]
fn generated_role_is_distinct_from_user_entity_of_same_kind() {
    let entity = ElaborationKey::entity(
        namespace(),
        instance("r1"),
        declaration("law"),
        EntityKind::Activation,
    )
    .unwrap();
    let generated = ElaborationKey::generated(
        namespace(),
        instance("r1"),
        declaration("law"),
        GeneratedRole::RelationActivation,
    )
    .unwrap();

    assert_eq!(entity.entity_kind(), generated.entity_kind());
    assert_ne!(
        entity.full_identity().unwrap(),
        generated.full_identity().unwrap()
    );
}

#[test]
fn complete_exterior_family_identity_uses_the_exact_boundary_discriminator() {
    let first_boundary = FullElaborationIdentity::from_sha256([0x11; 32]);
    let second_boundary = FullElaborationIdentity::from_sha256([0x22; 32]);
    let first = ElaborationKey::boundary_family_entity(
        namespace(),
        instance("solid"),
        declaration("mechanical"),
        EntityKind::Port,
        first_boundary,
    )
    .unwrap();
    let repeated = ElaborationKey::boundary_family_entity(
        namespace(),
        instance("solid"),
        declaration("mechanical"),
        EntityKind::Port,
        first_boundary,
    )
    .unwrap();
    let second = ElaborationKey::boundary_family_entity(
        namespace(),
        instance("solid"),
        declaration("mechanical"),
        EntityKind::Port,
        second_boundary,
    )
    .unwrap();

    assert_eq!(first.entity_kind(), EntityKind::Port);
    assert_eq!(
        first.canonical_bytes().unwrap(),
        repeated.canonical_bytes().unwrap()
    );
    assert_eq!(
        first.full_identity().unwrap(),
        repeated.full_identity().unwrap()
    );
    assert_ne!(
        first.full_identity().unwrap(),
        second.full_identity().unwrap()
    );
    assert!(
        first
            .canonical_bytes()
            .unwrap()
            .ends_with(first_boundary.as_bytes())
    );
    assert!(
        second
            .canonical_bytes()
            .unwrap()
            .ends_with(second_boundary.as_bytes())
    );
}

#[test]
fn complete_exterior_family_subjects_are_domain_separated() {
    let boundary = FullElaborationIdentity::from_sha256([0x33; 32]);
    let ordinary_port = entity_key("solid", "mechanical", EntityKind::Port);
    let family_port = ElaborationKey::boundary_family_entity(
        namespace(),
        instance("solid"),
        declaration("mechanical"),
        EntityKind::Port,
        boundary,
    )
    .unwrap();
    let family_relation = ElaborationKey::boundary_family_entity(
        namespace(),
        instance("solid"),
        declaration("boundary_law"),
        EntityKind::Relation,
        boundary,
    )
    .unwrap();
    let family_activation = ElaborationKey::boundary_family_generated(
        namespace(),
        instance("solid"),
        declaration("boundary_law"),
        GeneratedRole::RelationActivation,
        boundary,
    )
    .unwrap();

    assert_eq!(family_relation.entity_kind(), EntityKind::Relation);
    assert_eq!(family_activation.entity_kind(), EntityKind::Activation);
    assert_ne!(
        ordinary_port.full_identity().unwrap(),
        family_port.full_identity().unwrap()
    );
    assert_ne!(
        family_port.full_identity().unwrap(),
        family_relation.full_identity().unwrap()
    );
    assert_ne!(
        family_relation.full_identity().unwrap(),
        family_activation.full_identity().unwrap()
    );
    assert!(
        ElaborationKey::boundary_family_entity(
            namespace(),
            instance("solid"),
            declaration("not_a_family_entity"),
            EntityKind::Field,
            boundary,
        )
        .is_err()
    );
}

#[test]
fn complete_exterior_family_identity_obeys_the_existing_byte_limit() {
    let limits = ElaborationIdentityLimits {
        max_canonical_key_bytes: 1,
        ..ElaborationIdentityLimits::default()
    };
    let boundary = FullElaborationIdentity::from_sha256([0x44; 32]);

    assert!(
        ElaborationKey::boundary_family_entity_with_limits(
            IdentityNamespace::new(["org"]).unwrap(),
            InstancePath::new(["root"]).unwrap(),
            DeclarationPath::new(["mechanical"]).unwrap(),
            EntityKind::Port,
            boundary,
            limits,
        )
        .is_err()
    );
    assert!(
        ElaborationKey::boundary_family_generated_with_limits(
            IdentityNamespace::new(["org"]).unwrap(),
            InstancePath::new(["root"]).unwrap(),
            DeclarationPath::new(["boundary_law"]).unwrap(),
            GeneratedRole::RelationActivation,
            boundary,
            limits,
        )
        .is_err()
    );
}

#[test]
fn resource_policy_is_not_part_of_key_equality_or_identity() {
    let namespace = namespace();
    let instance = instance("r1");
    let declaration = declaration("voltage");
    let default = ElaborationKey::entity(
        namespace.clone(),
        instance.clone(),
        declaration.clone(),
        EntityKind::Field,
    )
    .unwrap();
    let relaxed = ElaborationKey::entity_with_limits(
        namespace,
        instance,
        declaration,
        EntityKind::Field,
        ElaborationIdentityLimits {
            max_canonical_key_bytes: 128 * 1_024,
            ..ElaborationIdentityLimits::default()
        },
    )
    .unwrap();

    assert_eq!(default, relaxed);
    assert_eq!(
        default.full_identity().unwrap(),
        relaxed.full_identity().unwrap()
    );
}

#[test]
fn model_view_has_a_separate_domain_and_only_one_root_is_staged() {
    let root = ModelViewKey::new(namespace(), InstancePath::new(["plant"]).unwrap()).unwrap();
    let root_identity = root.full_identity().unwrap();
    let similarly_named_relation = ElaborationKey::entity(
        namespace(),
        InstancePath::new(["plant"]).unwrap(),
        DeclarationPath::new(["model"]).unwrap(),
        EntityKind::Relation,
    )
    .unwrap();
    assert_ne!(
        root_identity,
        similarly_named_relation.full_identity().unwrap()
    );

    let mut allocator = StagingIdAllocator::new();
    assert_eq!(allocator.stage_model_view(&root).unwrap(), root_identity);
    assert_eq!(allocator.stage_model_view(&root).unwrap(), root_identity);
    let other_root = ModelViewKey::new(namespace(), InstancePath::new(["other"]).unwrap()).unwrap();
    assert!(allocator.stage_model_view(&other_root).is_err());

    let staged = allocator.finish();
    let projected = staged.resolve_model_view(root_identity).unwrap();
    assert_eq!(projected.full_identity(), root_identity);
    assert_eq!(
        projected.id().ulid(),
        Ulid::from(u128::from_be_bytes({
            let mut bytes = [0_u8; 16];
            bytes.copy_from_slice(&root_identity.as_bytes()[..16]);
            bytes
        }))
    );
}

#[test]
fn anonymous_connection_identity_is_member_permutation_invariant() {
    let a = entity_key("r1", "p", EntityKind::Port)
        .full_identity()
        .unwrap();
    let b = entity_key("r2", "p", EntityKind::Port)
        .full_identity()
        .unwrap();
    let first = ElaborationKey::anonymous_connection(
        namespace(),
        InstancePath::new(["plant"]).unwrap(),
        DeclarationPath::new(["network"]).unwrap(),
        [a, b],
    )
    .unwrap();
    let second = ElaborationKey::anonymous_connection(
        namespace(),
        InstancePath::new(["plant"]).unwrap(),
        DeclarationPath::new(["network"]).unwrap(),
        [b, a],
    )
    .unwrap();

    assert_eq!(
        first.canonical_bytes().unwrap(),
        second.canonical_bytes().unwrap()
    );
    assert_eq!(
        first.full_identity().unwrap(),
        second.full_identity().unwrap()
    );
}

#[derive(Debug)]
struct ConstantProjector;

impl ShortIdProjector for ConstantProjector {
    fn project(&self, _: FullElaborationIdentity) -> [u8; 16] {
        [7; 16]
    }
}

#[test]
fn projected_collision_fails_before_ids_are_exposed() {
    let mut allocator = StagingIdAllocator::with_projector_and_limits(
        ConstantProjector,
        ElaborationIdentityLimits::default(),
    );
    let first = entity_key("r1", "voltage", EntityKind::Field);
    let second = entity_key("r2", "voltage", EntityKind::Field);
    allocator.stage(&first).unwrap();

    let diagnostic = allocator.stage(&second).unwrap_err();
    assert_eq!(diagnostic.code(), codes::LANGUAGE_LOWERING_ERROR);
    assert!(diagnostic.message().contains("collision"));
}

#[test]
fn sealed_allocator_resolves_typed_ids_and_retains_full_identity() {
    let mut allocator = StagingIdAllocator::new();
    let key = entity_key("r1", "voltage", EntityKind::Field);
    let full = allocator.stage(&key).unwrap();
    assert_eq!(allocator.stage(&key).unwrap(), full);
    let staged = allocator.finish();

    let projected = staged.resolve::<kinds::Field>(full).unwrap();
    assert_eq!(projected.full_identity(), full);
    assert_eq!(
        projected.id().ulid(),
        Ulid::from(u128::from_be_bytes({
            let mut bytes = [0_u8; 16];
            bytes.copy_from_slice(&full.as_bytes()[..16]);
            bytes
        }))
    );
    assert!(staged.resolve::<kinds::Parameter>(full).is_err());
}

#[test]
fn construction_limits_fail_closed() {
    let mut limits = ElaborationIdentityLimits {
        max_instance_depth: 1,
        ..ElaborationIdentityLimits::default()
    };
    assert!(InstancePath::with_limits(["root", "child"], limits).is_err());

    limits = ElaborationIdentityLimits {
        max_segment_bytes: 3,
        ..ElaborationIdentityLimits::default()
    };
    assert!(IdentityNamespace::with_limits(["four"], limits).is_err());

    limits = ElaborationIdentityLimits {
        max_anonymous_connection_members: 1,
        ..ElaborationIdentityLimits::default()
    };
    let a = FullElaborationIdentity::from_sha256([1; 32]);
    let b = FullElaborationIdentity::from_sha256([2; 32]);
    assert!(
        ElaborationKey::anonymous_connection_with_limits(
            IdentityNamespace::new(["org"]).unwrap(),
            InstancePath::new(["root"]).unwrap(),
            DeclarationPath::new(["net"]).unwrap(),
            [a, b],
            limits,
        )
        .is_err()
    );

    limits = ElaborationIdentityLimits {
        max_canonical_key_bytes: 1,
        ..ElaborationIdentityLimits::default()
    };
    assert!(
        ElaborationKey::entity_with_limits(
            IdentityNamespace::new(["org"]).unwrap(),
            InstancePath::new(["root"]).unwrap(),
            DeclarationPath::new(["field"]).unwrap(),
            EntityKind::Field,
            limits,
        )
        .is_err()
    );

    limits = ElaborationIdentityLimits {
        max_staged_identities: 1,
        ..ElaborationIdentityLimits::default()
    };
    let mut allocator =
        StagingIdAllocator::with_projector_and_limits(Sha256PrefixProjector, limits);
    allocator
        .stage(&entity_key("r1", "a", EntityKind::Field))
        .unwrap();
    assert!(
        allocator
            .stage(&entity_key("r1", "b", EntityKind::Field))
            .is_err()
    );
}

#[test]
fn anonymous_connections_reject_ambiguous_membership() {
    let member = FullElaborationIdentity::from_sha256([1; 32]);
    let make = |members| {
        ElaborationKey::anonymous_connection(
            IdentityNamespace::new(["org"]).unwrap(),
            InstancePath::new(["root"]).unwrap(),
            DeclarationPath::new(["net"]).unwrap(),
            members,
        )
    };

    assert!(make(vec![member]).is_err());
    assert!(make(vec![member, member]).is_err());
}
