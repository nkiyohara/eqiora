use eqiora_artifact::{
    AcceptedCircularHoleChordalRealizationV1, GeometryMeshCorrespondenceEnvelopeV1,
    ModelDecoderLimits, ModelEnvelope, SimplicialMeshEnvelopeV1,
};
use eqiora_geometry::{CanonicalGeometryV1, EDGE_DIMENSION, FACE_DIMENSION, NamedEntitySet};
use eqiora_graph::{GraphStore, InMemoryGraphStore};
use eqiora_meshing::{MeshQualityGate, SimplicialMesh};
use eqiora_sem::KernelProgram;

use super::*;

const PACKAGED_MODEL: &[u8] =
    include_bytes!("../../../../../../examples/steady-flow-past-cylinder.model.json");

fn source() -> CanonicalGeometryV1 {
    CanonicalGeometryV1::from_circular_hole(
        [[0.0, 2.2], [0.0, 0.41]],
        [0.2, 0.2],
        0.05,
        vec![
            NamedEntitySet::new("inlet", EDGE_DIMENSION, vec![0]),
            NamedEntitySet::new("outlet", EDGE_DIMENSION, vec![1]),
            NamedEntitySet::new("walls", EDGE_DIMENSION, vec![2, 3]),
            NamedEntitySet::new("cylinder", EDGE_DIMENSION, vec![4]),
            NamedEntitySet::new("fluid", FACE_DIMENSION, vec![0]),
        ],
        1.0e-12,
    )
    .expect("exact-cylinder source")
}

fn model_and_program(source: &CanonicalGeometryV1) -> (ModelEnvelope, KernelProgram) {
    let model = ModelEnvelope::from_json(PACKAGED_MODEL, ModelDecoderLimits::default())
        .expect("accepted packaged Model");
    let (transaction, model_id) = model.to_transaction().expect("Model transaction");
    let mut store = InMemoryGraphStore::new();
    store.commit(transaction).expect("Model replay");
    let program =
        KernelProgram::from_snapshot_with_geometry(&store.snapshot(), model_id, &[source.into()])
            .expect("source-bound Stokes program");
    (model, program)
}

fn reference(source: &CanonicalGeometryV1) -> AcceptedCircularHoleChordalRealizationV1 {
    AcceptedCircularHoleChordalRealizationV1::from_reference(
        source,
        1.0e-4,
        50,
        MeshQualityGate::new(1.0e-5).expect("quality policy"),
    )
    .expect("reference witness")
}

fn alternate_bound_witness(
    reference: &AcceptedCircularHoleChordalRealizationV1,
) -> AcceptedCircularHoleChordalRealizationV1 {
    let native = reference.mesh().mesh();
    let mut cells = native.cells().to_vec();
    cells.reverse();
    let mesh = SimplicialMesh::new(2, native.vertices().to_vec(), cells, native.quality_gate())
        .and_then(|mesh| SimplicialMeshEnvelopeV1::from_mesh(&mesh))
        .expect("alternate conforming Mesh");
    let correspondence =
        GeometryMeshCorrespondenceEnvelopeV1::from_region(reference.realized_geometry(), &mesh)
            .expect("alternate correspondence");
    reference
        .bind_conforming_mesh(&mesh, &correspondence)
        .expect("alternate exact-source witness")
}

#[test]
fn every_manual_automatic_l_u_p_disposition_has_exact_receipt_pruning() {
    let source = source();
    let (model, program) = model_and_program(&source);
    let binding = SteadyStokesGeometryBinding2d::new(&program, reference(&source))
        .expect("authenticated Stokes binding");
    for manual_mask in 0_u8..8 {
        let request = IncompressibleScalingRequest2d::new(
            (manual_mask & 1 != 0).then(|| DynQuantity::new(0.82, LENGTH)),
            (manual_mask & 2 != 0).then(|| DynQuantity::new(0.6, VELOCITY)),
            (manual_mask & 4 != 0).then(|| DynQuantity::new(0.02, PRESSURE)),
        )
        .expect("valid component-wise request");
        let resolved = binding
            .resolve_incompressible_scaling(&model, Some(request))
            .expect("all eight fixed-topology dispositions resolve");
        let receipt = resolved.receipt();

        for (bit, component) in [
            (1, ScalingComponent2d::Length),
            (2, ScalingComponent2d::Velocity),
            (4, ScalingComponent2d::Pressure),
        ] {
            assert_eq!(
                receipt.component(component).mode(),
                if manual_mask & bit != 0 {
                    ScalingMode2d::Manual
                } else {
                    ScalingMode2d::Automatic
                },
                "wrong disposition for mask {manual_mask:03b} and {component:?}",
            );
            let authorities = receipt.component(component).authorities();
            if manual_mask & bit != 0 {
                assert_eq!(authorities.as_slice(), &[ScalingAuthority2d::ManualRequest]);
            } else {
                match component {
                    ScalingComponent2d::Length => assert!(matches!(
                        authorities.as_slice(),
                        [ScalingAuthority2d::ExactGeometrySpan { .. }]
                    )),
                    ScalingComponent2d::Velocity => assert!(matches!(
                        authorities.as_slice(),
                        [
                            ScalingAuthority2d::ExactGeometrySpan { .. },
                            ScalingAuthority2d::ModelInletMaximum { .. }
                        ]
                    )),
                    ScalingComponent2d::Pressure => {}
                    ScalingComponent2d::Gauge | ScalingComponent2d::WeakFunctional => {
                        unreachable!("the loop contains only L/U/P")
                    }
                }
            }
        }

        let pressure = receipt.component(ScalingComponent2d::Pressure);
        if manual_mask & 4 != 0 {
            assert_eq!(pressure.dependencies(), ScalingDependencies2d::None);
            assert_eq!(
                pressure.authorities().as_slice(),
                &[ScalingAuthority2d::ManualRequest]
            );
        } else {
            assert_eq!(
                pressure.dependencies().as_slice(),
                &[ScalingComponent2d::Length, ScalingComponent2d::Velocity]
            );
            assert!(matches!(
                pressure.authorities().as_slice(),
                [ScalingAuthority2d::ModelDynamicViscosity { .. }]
            ));
            let expected =
                0.001 * resolved.scales().velocity().value() / resolved.scales().length().value();
            assert_eq!(
                resolved.scales().pressure().value().to_bits(),
                expected.to_bits()
            );
        }
        assert_eq!(
            receipt.component(ScalingComponent2d::Gauge).mode(),
            ScalingMode2d::Derived
        );
        assert_eq!(
            receipt.component(ScalingComponent2d::WeakFunctional).mode(),
            ScalingMode2d::Derived
        );
    }
}

#[test]
fn automatic_receipt_is_typed_and_bound_meshes_are_bit_equal() {
    let source = source();
    let (model, program) = model_and_program(&source);
    let reference_witness = reference(&source);
    let alternate = alternate_bound_witness(&reference_witness);
    assert_ne!(
        reference_witness.mesh().digest().unwrap(),
        alternate.mesh().digest().unwrap()
    );
    let first = SteadyStokesGeometryBinding2d::new(&program, reference_witness)
        .and_then(|binding| binding.resolve_incompressible_scaling(&model, None))
        .expect("reference scaling");
    let explicit_none = SteadyStokesGeometryBinding2d::new(&program, reference(&source))
        .and_then(|binding| {
            binding.resolve_incompressible_scaling(
                &model,
                Some(IncompressibleScalingRequest2d::default()),
            )
        })
        .expect("explicit all-None scaling");
    assert_eq!(first, explicit_none);
    let second = SteadyStokesGeometryBinding2d::new(&program, alternate)
        .and_then(|binding| binding.resolve_incompressible_scaling(&model, None))
        .expect("alternate bound-Mesh scaling");

    for (left, right) in first
        .receipt()
        .components()
        .iter()
        .zip(second.receipt().components())
    {
        assert_eq!(left.component(), right.component());
        assert_eq!(
            left.value().value().to_bits(),
            right.value().value().to_bits()
        );
        assert_eq!(left.value().dim(), right.value().dim());
        assert_eq!(left.mode(), right.mode());
        assert_eq!(left.rule(), right.rule());
        assert_eq!(left.dependencies(), right.dependencies());
        assert_eq!(left.authorities(), right.authorities());
    }
    assert_ne!(first.receipt().mesh(), second.receipt().mesh());
    assert_eq!(first.receipt().model(), second.receipt().model());
    assert_eq!(first.receipt().geometry(), second.receipt().geometry());
}

#[test]
fn manual_validation_and_derived_overflow_fail_closed() {
    for value in [0.0, -0.0, -1.0, f64::INFINITY, f64::NEG_INFINITY, f64::NAN] {
        IncompressibleScalingRequest2d::new(Some(DynQuantity::new(value, LENGTH)), None, None)
            .expect_err("invalid manual L must reject");
    }
    IncompressibleScalingRequest2d::new(Some(DynQuantity::new(1.0, VELOCITY)), None, None)
        .expect_err("dimensionally wrong manual L must reject");

    let source = source();
    let (model, program) = model_and_program(&source);
    let binding = SteadyStokesGeometryBinding2d::new(&program, reference(&source)).unwrap();
    let request = IncompressibleScalingRequest2d::new(
        Some(DynQuantity::new(f64::MIN_POSITIVE, LENGTH)),
        Some(DynQuantity::new(f64::MAX, VELOCITY)),
        None,
    )
    .unwrap();
    binding
        .resolve_incompressible_scaling(&model, Some(request))
        .expect_err("overflowing automatic P must reject");

    let foreign_bytes = std::str::from_utf8(PACKAGED_MODEL).unwrap().replacen(
        "\"value\":0.001",
        "\"value\":0.002",
        1,
    );
    let foreign_model =
        ModelEnvelope::from_json(foreign_bytes.as_bytes(), ModelDecoderLimits::default())
            .expect("foreign Model remains structurally valid");
    binding
        .resolve_incompressible_scaling(&foreign_model, None)
        .expect_err("foreign Model meaning must reject before observations");
}
