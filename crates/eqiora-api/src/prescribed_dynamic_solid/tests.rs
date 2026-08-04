use std::num::NonZeroUsize;

use eqiora_artifact::{DiscreteFieldEnvelopeV1, RunManifestV2, SpatialStateEnvelopeV1};
use eqiora_assembly::REFERENCE_ASSEMBLY_BACKEND;
use eqiora_core::{DimExponents, DynQuantity};
use eqiora_meshing::VertexId;
use eqiora_numerics::solid::PrescribedDynamicSolidReference3d;
use eqiora_solver::REFERENCE_LINEAR_SOLVER;
use serde_json::{Value, json};

use crate::ModelDocument;

use super::PrescribedDynamicSolidStateRun3d;

const DIRECT_SOURCE: &str = include_str!(
    "../../../../verify/artifacts/prescribed-dynamic-solid-state-run-3d/models/direct.eqi"
);
const TIME: DimExponents = DimExponents {
    time: 1,
    ..DimExponents::DIMENSIONLESS
};

fn owner() -> PrescribedDynamicSolidStateRun3d {
    let document = ModelDocument::compile("private-owner-role-oracle.eqi", DIRECT_SOURCE).unwrap();
    PrescribedDynamicSolidStateRun3d::solve_reference(
        &document,
        &REFERENCE_ASSEMBLY_BACKEND,
        &REFERENCE_LINEAR_SOLVER,
    )
    .expect("the private role oracle starts from one valid complete owner")
}

#[test]
fn revalidate_rejects_prior_and_accepted_state_substitution_or_swap() {
    let valid = owner();

    let mut accepted_in_prior = valid.clone();
    accepted_in_prior.prior_state = accepted_in_prior.accepted_state.clone();
    assert!(accepted_in_prior.revalidate().is_err());

    let mut prior_in_accepted = valid.clone();
    prior_in_accepted.accepted_state = prior_in_accepted.prior_state.clone();
    assert!(prior_in_accepted.revalidate().is_err());

    let mut swapped = valid.clone();
    std::mem::swap(&mut swapped.prior_state, &mut swapped.accepted_state);
    assert!(swapped.revalidate().is_err());
}

#[test]
fn revalidate_rejects_role_specific_snapshot_and_block_substitution() {
    let valid = owner();

    let mut accepted_snapshot_in_prior = valid.clone();
    accepted_snapshot_in_prior.prior_displacement_snapshot = accepted_snapshot_in_prior
        .accepted_displacement_snapshot
        .clone();
    assert!(accepted_snapshot_in_prior.revalidate().is_err());

    let mut prior_snapshot_in_accepted = valid.clone();
    prior_snapshot_in_accepted.accepted_displacement_snapshot = prior_snapshot_in_accepted
        .prior_displacement_snapshot
        .clone();
    assert!(prior_snapshot_in_accepted.revalidate().is_err());

    let mut accepted_block_in_prior = valid.clone();
    accepted_block_in_prior.prior_displacement_block =
        accepted_block_in_prior.accepted_displacement_block.clone();
    assert!(accepted_block_in_prior.revalidate().is_err());

    let mut prior_block_in_accepted = valid.clone();
    prior_block_in_accepted.accepted_displacement_block =
        prior_block_in_accepted.prior_displacement_block.clone();
    assert!(prior_block_in_accepted.revalidate().is_err());

    let mut changed_leaf = valid.clone();
    let mut wire: Value = serde_json::from_slice(
        &changed_leaf
            .accepted_displacement_block
            .canonical_json()
            .unwrap(),
    )
    .unwrap();
    wire["values"][24] = json!(0.008);
    changed_leaf.accepted_displacement_block =
        DiscreteFieldEnvelopeV1::from_json(&serde_json::to_vec(&wire).unwrap(), Default::default())
            .expect("the changed numerical block is locally valid");
    assert!(changed_leaf.revalidate().is_err());

    let mut coherent_prior_leaf_replacement = valid.clone();
    coherent_prior_leaf_replacement.prior_displacement_block =
        valid.accepted_displacement_block.clone();
    coherent_prior_leaf_replacement.prior_displacement_snapshot =
        valid.accepted_displacement_snapshot.clone();
    let prior_snapshot = valid
        .prior_displacement_snapshot
        .digest()
        .unwrap()
        .to_string();
    let accepted_snapshot = valid
        .accepted_displacement_snapshot
        .digest()
        .unwrap()
        .to_string();
    let prior_state = valid.prior_state.canonical_json().unwrap();
    let prior_state = std::str::from_utf8(&prior_state).unwrap();
    assert_eq!(prior_state.matches(&prior_snapshot).count(), 1);
    let replaced_state = prior_state.replacen(&prior_snapshot, &accepted_snapshot, 1);
    coherent_prior_leaf_replacement.prior_state = SpatialStateEnvelopeV1::from_json(
        replaced_state.as_bytes(),
        Default::default(),
    )
    .expect("the coherently replaced prior leaf chain remains locally valid State data");
    assert!(coherent_prior_leaf_replacement.revalidate().is_err());
}

#[test]
fn revalidate_rejects_prior_additional_or_foreign_run_outputs_and_provider_drift() {
    let valid = owner();
    let prior = valid.prior_state.digest().unwrap().to_string();
    let accepted = valid.accepted_state.digest().unwrap().to_string();

    let mut prior_output = valid.clone();
    prior_output.run = run_with_outputs(&valid, vec![prior.clone()]);
    assert!(prior_output.revalidate().is_err());

    let mut missing_output = valid.clone();
    missing_output.run = run_with_outputs(&valid, Vec::new());
    assert!(missing_output.revalidate().is_err());

    let mut additional_output = valid.clone();
    let mut outputs = vec![prior, accepted];
    outputs.sort();
    additional_output.run = run_with_outputs(&valid, outputs);
    assert!(additional_output.revalidate().is_err());

    let mut foreign_output = valid.clone();
    foreign_output.run =
        run_with_outputs(&valid, vec![valid.geometry.digest().unwrap().to_string()]);
    assert!(foreign_output.revalidate().is_err());

    let mut stale_model = valid.clone();
    let mut wire: Value =
        serde_json::from_slice(&stale_model.run.canonical_json().unwrap()).unwrap();
    wire[concat!("model_", "sha256")] = json!(valid.geometry.digest().unwrap().to_string());
    stale_model.run =
        RunManifestV2::from_json(&serde_json::to_vec(&wire).unwrap(), Default::default())
            .expect("stale Model lineage is locally valid Run data");
    assert!(stale_model.revalidate().is_err());

    let mut stale_realization = valid.clone();
    let mut wire: Value =
        serde_json::from_slice(&stale_realization.run.canonical_json().unwrap()).unwrap();
    wire["realization_sha256"] = json!(valid.geometry.digest().unwrap().to_string());
    stale_realization.run =
        RunManifestV2::from_json(&serde_json::to_vec(&wire).unwrap(), Default::default())
            .expect("stale Realization lineage is locally valid Run data");
    assert!(stale_realization.revalidate().is_err());

    let mut provider = valid.clone();
    let mut wire: Value = serde_json::from_slice(&provider.run.canonical_json().unwrap()).unwrap();
    wire["execution"]["solver_backend"] = json!("eqiora.oracle.foreign-solver");
    provider.run =
        RunManifestV2::from_json(&serde_json::to_vec(&wire).unwrap(), Default::default())
            .expect("provider substitution is locally valid Run data");
    assert!(provider.revalidate().is_err());

    let mut topology = valid.clone();
    let mut wire: Value = serde_json::from_slice(&topology.run.canonical_json().unwrap()).unwrap();
    wire["execution"]["topology"]["workers"] = json!(NonZeroUsize::new(2).unwrap().get());
    topology.run =
        RunManifestV2::from_json(&serde_json::to_vec(&wire).unwrap(), Default::default())
            .expect("two-worker topology is locally valid Run data");
    assert!(topology.revalidate().is_err());

    let mut reduction = valid.clone();
    let mut wire: Value = serde_json::from_slice(&reduction.run.canonical_json().unwrap()).unwrap();
    wire["execution"]["reduction"] = json!("fast");
    reduction.run =
        RunManifestV2::from_json(&serde_json::to_vec(&wire).unwrap(), Default::default())
            .expect("reduction substitution is locally valid Run data");
    assert!(reduction.revalidate().is_err());
}

#[test]
fn revalidate_rejects_a_nonforgeable_result_from_another_candidate() {
    let valid = owner();
    let prior_displacement = coefficients(valid.prior_displacement_block.values());
    let prior_velocity = coefficients(valid.prior_velocity_block.values());
    let mut reference = PrescribedDynamicSolidReference3d::new(
        &valid.model,
        &valid.geometry,
        &valid.mesh,
        &valid.correspondence,
        DynQuantity::new(0.25, TIME),
        &prior_displacement,
        &prior_velocity,
        valid.realization.driven_boundary(),
    )
    .unwrap();
    let changed_candidate = [1, 3, 5, 7]
        .into_iter()
        .map(|vertex| (VertexId::new(vertex), [0.02, 0.0, 0.0]))
        .collect::<Vec<_>>();
    let foreign = reference
        .accept_candidate(
            0,
            &changed_candidate,
            &REFERENCE_ASSEMBLY_BACKEND,
            &REFERENCE_LINEAR_SOLVER,
        )
        .expect("the other candidate produces its own nonforgeable accepted result");
    let mut substituted = valid.clone();
    substituted.accepted = foreign;
    assert!(substituted.revalidate().is_err());

    let later = reference
        .accept_candidate(
            1,
            &changed_candidate,
            &REFERENCE_ASSEMBLY_BACKEND,
            &REFERENCE_LINEAR_SOLVER,
        )
        .expect("the advanced reference produces a different generation");
    let mut wrong_generation = valid;
    wrong_generation.accepted = later;
    assert!(wrong_generation.revalidate().is_err());
}

fn run_with_outputs(
    owner: &PrescribedDynamicSolidStateRun3d,
    outputs: Vec<String>,
) -> RunManifestV2 {
    let mut wire: Value = serde_json::from_slice(&owner.run.canonical_json().unwrap()).unwrap();
    wire["output_sha256"] = json!(outputs);
    RunManifestV2::from_json(&serde_json::to_vec(&wire).unwrap(), Default::default())
        .expect("the substituted output vector is locally valid Run data")
}

fn coefficients(values: &[f64]) -> Vec<(VertexId, [f64; 3])> {
    values
        .chunks_exact(3)
        .enumerate()
        .map(|(vertex, value)| {
            (
                VertexId::new(vertex),
                value.try_into().expect("every vector has three components"),
            )
        })
        .collect()
}
