use eqiora::meshing::{LineMesh, MeshEntity, MeshTopology};
use eqiora::numerics::compare_sine_poisson_1d;

#[test]
fn public_facade_exposes_spatial_contracts_and_poisson_evidence() {
    let mesh = LineMesh::uniform(0.0, 1.0, 8).unwrap();
    let closure = mesh.incidence(MeshEntity::new(1, 0), 0).unwrap();
    assert_eq!(closure.len(), 2);

    let report = compare_sine_poisson_1d(&[8, 16]).unwrap();
    assert!(report[1].fem_order.unwrap() > 1.9);
    assert!(report[1].fvm_order.unwrap() > 1.9);
    assert!(report[1].fem_relative_balance_error < 2.0e-12);
    assert!(report[1].fvm_relative_balance_error < 2.0e-12);
}
