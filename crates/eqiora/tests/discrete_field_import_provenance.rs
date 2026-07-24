use std::num::NonZeroU32;

use eqiora::artifact::{
    DataExchangeDecoderLimits, DiscreteFieldEnvelopeV1, ExternalAdapterIdentityV1,
    ExternalImportManifestV1, ExternalImportObservationV1, ExternalImportSelectionV1,
    ExternalImportSourceV1, ResolvedArrayV1, ResolvedImportArrayV1, SelectedSourceEntityV1,
    SimplicialMeshEnvelopeV1, SpatialDecoderLimits, StructuralSelectorV1,
};
use eqiora::meshing::{
    DiscreteFieldAssociation, DiscreteFieldPayload, DiscreteFieldShape, MeshQualityGate,
    SimplicialMesh,
};

struct ImportAssertion {
    mesh: SimplicialMeshEnvelopeV1,
    fields: Vec<DiscreteFieldEnvelopeV1>,
    observation: ExternalImportObservationV1,
    manifest: ExternalImportManifestV1,
}

fn selector(path: &[u32]) -> StructuralSelectorV1 {
    StructuralSelectorV1::new(path.to_vec())
}

fn resolved(
    source_ordinal: u32,
    path: &[u32],
    storage_display_selector: String,
    array: ResolvedArrayV1,
) -> ResolvedImportArrayV1 {
    ResolvedImportArrayV1::new(
        source_ordinal,
        selector(path),
        Some(storage_display_selector),
        array,
    )
    .unwrap()
}

fn import_assertion(label: &str) -> ImportAssertion {
    let mesh = SimplicialMeshEnvelopeV1::from_mesh(
        &SimplicialMesh::new(
            2,
            vec![
                vec![0.0, 0.0],
                vec![1.0, 0.0],
                vec![1.0, 1.0],
                vec![0.0, 1.0],
            ],
            vec![vec![0, 1, 2], vec![0, 2, 3]],
            MeshQualityGate::new(0.2).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();

    let vertex_scalar = DiscreteFieldPayload::new(
        mesh.mesh(),
        DiscreteFieldAssociation::Vertex,
        DiscreteFieldShape::Scalar,
        vec![10.0, 20.0, 30.0, 40.0],
    )
    .unwrap();
    let cell_vector = DiscreteFieldPayload::new(
        mesh.mesh(),
        DiscreteFieldAssociation::Cell,
        DiscreteFieldShape::Vector {
            components: NonZeroU32::new(2).unwrap(),
        },
        vec![1.0, 0.0, 0.0, 1.0],
    )
    .unwrap();
    let fields = vec![
        DiscreteFieldEnvelopeV1::from_payload(&mesh, &vertex_scalar).unwrap(),
        DiscreteFieldEnvelopeV1::from_payload(&mesh, &cell_vector).unwrap(),
    ];

    let metadata = ExternalImportSourceV1::metadata_document(
        format!("metadata-{label}").into_bytes(),
        Some(format!("{label}.metadata")),
    )
    .unwrap();
    let external_sources = [
        ([0, 0].as_slice(), "geometry"),
        ([0, 1].as_slice(), "topology"),
        ([0, 2].as_slice(), "temperature"),
        ([0, 3].as_slice(), "cell-velocity"),
    ]
    .into_iter()
    .map(|(path, role)| {
        ExternalImportSourceV1::external_array_source(
            selector(path),
            format!("raw-{label}-{role}").into_bytes(),
            Some(format!("{label}.store")),
        )
        .unwrap()
    })
    .collect();
    let observation = ExternalImportObservationV1::new(
        metadata,
        external_sources,
        resolved(
            1,
            &[0, 0],
            format!("/{label}/geometry"),
            ResolvedArrayV1::from_f64(vec![4, 2], vec![0.0, 0.0, 1.0, 0.0, 1.0, 1.0, 0.0, 1.0])
                .unwrap(),
        ),
        resolved(
            2,
            &[0, 1],
            format!("/{label}/topology"),
            ResolvedArrayV1::from_u64(vec![2, 3], vec![0, 1, 2, 0, 2, 3]).unwrap(),
        ),
        vec![
            resolved(
                3,
                &[0, 2],
                format!("/{label}/temperature"),
                ResolvedArrayV1::from_f64(vec![4], vec![10.0, 20.0, 30.0, 40.0]).unwrap(),
            ),
            resolved(
                4,
                &[0, 3],
                format!("/{label}/cell-velocity"),
                ResolvedArrayV1::from_f64(vec![2, 2], vec![1.0, 0.0, 0.0, 1.0]).unwrap(),
            ),
        ],
    )
    .unwrap();
    let selection = ExternalImportSelectionV1::new(
        SelectedSourceEntityV1::new(selector(&[0]), Some(format!("grid-{label}"))).unwrap(),
        vec![
            SelectedSourceEntityV1::new(selector(&[0, 2]), Some(format!("temperature-{label}")))
                .unwrap(),
            SelectedSourceEntityV1::new(selector(&[0, 3]), Some(format!("cell-velocity-{label}")))
                .unwrap(),
        ],
    )
    .unwrap();
    let manifest = ExternalImportManifestV1::from_observation(
        ExternalAdapterIdentityV1::new("eqiora.evidence-import", "1.0.0").unwrap(),
        Vec::new(),
        selection,
        &observation,
        &mesh,
        &fields,
    )
    .unwrap();

    ImportAssertion {
        mesh,
        fields,
        observation,
        manifest,
    }
}

#[test]
fn accepted_field_identity_is_separate_from_source_and_layout_provenance() {
    let alpha = import_assertion("alpha");
    let beta = import_assertion("beta");

    assert_eq!(alpha.mesh.digest().unwrap(), beta.mesh.digest().unwrap());
    assert_eq!(alpha.fields.len(), 2);
    assert_eq!(
        alpha.fields[0].association(),
        DiscreteFieldAssociation::Vertex
    );
    assert_eq!(
        alpha.fields[0].component_shape(),
        DiscreteFieldShape::Scalar
    );
    assert_eq!(
        alpha.fields[1].association(),
        DiscreteFieldAssociation::Cell
    );
    assert_eq!(
        alpha.fields[1].component_shape(),
        DiscreteFieldShape::Vector {
            components: NonZeroU32::new(2).unwrap(),
        }
    );

    for (alpha_field, beta_field) in alpha.fields.iter().zip(&beta.fields) {
        assert_eq!(
            alpha_field.canonical_json().unwrap(),
            beta_field.canonical_json().unwrap()
        );
        assert_eq!(alpha_field.digest().unwrap(), beta_field.digest().unwrap());
        alpha_field.validate_mesh_artifact(&alpha.mesh).unwrap();
    }
    assert_eq!(
        alpha.manifest.accepted_field_artifacts(),
        alpha
            .fields
            .iter()
            .map(|field| field.digest().unwrap())
            .collect::<Vec<_>>()
    );
    assert_ne!(
        alpha.manifest.canonical_json().unwrap(),
        beta.manifest.canonical_json().unwrap()
    );
    assert_ne!(
        alpha.manifest.digest().unwrap(),
        beta.manifest.digest().unwrap()
    );
}

#[test]
fn manifest_references_fail_closed_but_do_not_claim_derivation() {
    let alpha = import_assertion("alpha");
    let beta = import_assertion("beta");

    alpha
        .manifest
        .validate_references(&alpha.observation, &alpha.mesh, &alpha.fields)
        .unwrap();
    assert!(
        alpha
            .manifest
            .validate_references(&beta.observation, &alpha.mesh, &alpha.fields)
            .is_err()
    );
    let reversed_fields = alpha.fields.iter().cloned().rev().collect::<Vec<_>>();
    assert!(
        alpha
            .manifest
            .validate_references(&alpha.observation, &alpha.mesh, &reversed_fields)
            .is_err()
    );

    // A manifest is an exact assertion over independently identified objects,
    // not proof that arbitrary source bytes derived the normalized array.
    assert_ne!(
        alpha.observation.external_sources()[0].bytes(),
        alpha
            .observation
            .mesh_geometry()
            .array()
            .canonical_json()
            .unwrap()
    );
}

#[test]
fn public_field_and_decoder_boundaries_reject_invalid_data() {
    let fixture = import_assertion("bounded");
    assert!(
        DiscreteFieldPayload::new(
            fixture.mesh.mesh(),
            DiscreteFieldAssociation::Vertex,
            DiscreteFieldShape::Scalar,
            vec![0.0, 1.0, f64::NAN, 3.0],
        )
        .is_err()
    );
    assert!(
        DiscreteFieldPayload::new(
            fixture.mesh.mesh(),
            DiscreteFieldAssociation::Cell,
            DiscreteFieldShape::Vector {
                components: NonZeroU32::new(2).unwrap(),
            },
            vec![1.0, 2.0, 3.0],
        )
        .is_err()
    );
    let field_bytes = fixture.fields[0].canonical_json().unwrap();
    assert!(
        DiscreteFieldEnvelopeV1::from_json(
            &field_bytes,
            SpatialDecoderLimits {
                max_discrete_field_values: 3,
                ..Default::default()
            },
        )
        .is_err()
    );
    let manifest_bytes = fixture.manifest.canonical_json().unwrap();
    assert!(
        ExternalImportManifestV1::from_json(
            &manifest_bytes,
            DataExchangeDecoderLimits {
                max_import_sources: 4,
                ..Default::default()
            },
        )
        .is_err()
    );
}
