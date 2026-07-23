use eqiora_cad_truck::{KERNEL_VERSION, TruckCadAdapterV1};
use eqiora_geometry::{
    AxisAlignedBox3, CadBoxDesignV1, CadKernelAdapter, ConstrainedRectangleV1, StepLengthUnitV1,
    StepSourceDigest,
};

const OUTER_BOX_MM: &[u8] =
    include_bytes!("../../../verify/geometry/cad-semantic-selection-box/models/outer-box-mm.step");

fn design(source: &[u8], unit: StepLengthUnitV1) -> CadBoxDesignV1 {
    CadBoxDesignV1::new(
        eqiora_core::Id::new(),
        StepSourceDigest::from_source_bytes(source),
        unit,
        AxisAlignedBox3::new([(-1.0, 1.0), (-1.0, 1.0), (-1.0, 1.0)]).unwrap(),
        ConstrainedRectangleV1::new((-0.5, 0.5), (-0.5, 0.5), -0.5).unwrap(),
        1.0,
        1.0e-12,
        1.0e-10,
    )
    .unwrap()
}

#[test]
fn imports_mm_step_extrudes_and_intersects_without_kernel_identity_leakage() {
    let design = design(OUTER_BOX_MM, StepLengthUnitV1::Millimetre);
    let adapter = TruckCadAdapterV1;
    let accepted = adapter.realize_box_design(&design, OUTER_BOX_MM).unwrap();

    assert_eq!(
        accepted.imported_stock().bounds().bounds_m(),
        [(-1.0, 1.0), (-1.0, 1.0), (-1.0, 1.0)]
    );
    assert_eq!(
        accepted.intersection().bounds().bounds_m(),
        [(-0.5, 0.5), (-0.5, 0.5), (-0.5, 0.5)]
    );
    assert_eq!(accepted.intersection().planar_face_count(), 6);
    assert_eq!(adapter.identity().kernel_version(), KERNEL_VERSION);
}

#[test]
fn rejects_source_unit_and_closed_topology_drift() {
    let adapter = TruckCadAdapterV1;
    let accepted_design = design(OUTER_BOX_MM, StepLengthUnitV1::Millimetre);

    let mut substituted = OUTER_BOX_MM.to_vec();
    let last = substituted.len() - 1;
    substituted[last] ^= 1;
    assert!(
        adapter
            .realize_box_design(&accepted_design, &substituted)
            .is_err()
    );

    assert!(
        adapter
            .realize_box_design(&design(OUTER_BOX_MM, StepLengthUnitV1::Metre), OUTER_BOX_MM,)
            .is_err()
    );

    let open_shell = String::from_utf8(OUTER_BOX_MM.to_vec())
        .unwrap()
        .replacen("CLOSED_SHELL", "OPEN_SHELL", 1)
        .into_bytes();
    assert!(
        adapter
            .realize_box_design(
                &design(&open_shell, StepLengthUnitV1::Millimetre),
                &open_shell,
            )
            .is_err()
    );
}
