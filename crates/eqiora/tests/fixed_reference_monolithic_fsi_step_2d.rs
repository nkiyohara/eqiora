use eqiora::numerics::{
    FixedReferenceFsiCartesianModel2d, PhysicalBoundaryDisposition,
    lower_fixed_reference_fsi_cartesian_2d,
};
use eqiora::realization::{SolveRoot, TransformationNode};
use support::fixed_reference_fsi::{
    direct_document, exact_spatial_witness, execute_initial_step, packaged_document,
};

mod support;

#[derive(Debug, PartialEq)]
struct SemanticObservation {
    fluid_bounds: [[u64; 2]; 2],
    solid_bounds: [[u64; 2]; 2],
    fluid_density: u64,
    fluid_viscosity: u64,
    solid_density: u64,
    solid_mu: u64,
    solid_lambda: u64,
    interface_axis: usize,
    fluid_side: eqiora::kernel::BoundarySide,
    solid_side: eqiora::kernel::BoundarySide,
}

#[test]
fn direct_and_exact_packages_share_one_fixed_reference_fsi_meaning() {
    let direct = direct_document();
    let packaged = packaged_document();

    let direct_model = lower_fixed_reference_fsi_cartesian_2d(direct.program())
        .expect("direct fixed-reference FSI semantics lower");
    let packaged_model = lower_fixed_reference_fsi_cartesian_2d(packaged.model().program())
        .expect("exact-package fixed-reference FSI semantics lower");
    assert_eq!(observe(&direct_model), observe(&packaged_model));
    let direct_spatial = exact_spatial_witness(direct.program(), &direct_model);
    let packaged_spatial = exact_spatial_witness(packaged.model().program(), &packaged_model);
    assert_eq!(direct_spatial, packaged_spatial);

    for model in [&direct_model, &packaged_model] {
        let interface = model.interface();
        assert_eq!(interface.axis(), 0);
        assert_ne!(interface.fluid().boundary(), interface.solid().boundary());
        assert_ne!(interface.fluid().port(), interface.solid().port());
        assert_eq!(
            model.fluid().conservative_body_force(&[0.25, 0.5]).unwrap(),
            [0.0; 2]
        );
        assert_eq!(
            model
                .solid()
                .load_potential_expression()
                .evaluate(&[1.5, 0.5])
                .unwrap(),
            0.0
        );
        assert_eq!(live_boundary_count(model), 2);
    }
}

#[test]
fn fixed_reference_monolithic_fsi_step_2d() {
    let direct = direct_document();
    let packaged = packaged_document();

    let direct_model = lower_fixed_reference_fsi_cartesian_2d(direct.program())
        .expect("direct fixed-reference FSI semantics lower");
    let packaged_model = lower_fixed_reference_fsi_cartesian_2d(packaged.model().program())
        .expect("exact-package fixed-reference FSI semantics lower");
    let direct = execute_initial_step(direct.program(), &direct_model);
    let packaged = execute_initial_step(packaged.model().program(), &packaged_model);

    assert_eq!(direct.operator, direct.replayed_operator);
    assert_eq!(packaged.operator, packaged.replayed_operator);
    assert_eq!(direct.operator, packaged.operator);
    assert_eq!(
        direct.solution.vertex_velocity_coefficients(),
        packaged.solution.vertex_velocity_coefficients()
    );
    assert_eq!(
        direct.solution.fluid_velocity_bubble_coefficients(),
        packaged.solution.fluid_velocity_bubble_coefficients()
    );
    assert_eq!(
        direct.solution.fluid_pressure_coefficients(),
        packaged.solution.fluid_pressure_coefficients()
    );
    assert_eq!(
        direct.solution.solid_displacement_coefficients(),
        packaged.solution.solid_displacement_coefficients()
    );

    for execution in [&direct, &packaged] {
        let solution = &execution.solution;
        assert!(matches!(
            solution.realization_graph().root(),
            SolveRoot::Linear(_)
        ));
        assert!(matches!(
            solution.realization_graph().transformations(),
            [
                TransformationNode::BackwardEulerElimination { .. },
                TransformationNode::ConformingTraceQuotient { .. }
            ]
        ));
        let evidence = solution.numerical_evidence();
        let interface_midpoint = solution
            .fluid_velocity_vertices()
            .iter()
            .copied()
            .find(|vertex| {
                solution.solid_velocity_vertices().contains(vertex)
                    && solution
                        .fluid_velocity_coefficient(*vertex)
                        .is_some_and(|value| {
                            value.into_iter().any(|component| component.abs() > 1.0e-10)
                        })
            })
            .expect("the exact shared interface has nonzero motion");
        assert_eq!(
            solution.fluid_velocity_coefficient(interface_midpoint),
            solution.solid_velocity_coefficient(interface_midpoint)
        );
        assert!(evidence.pressure_constant_action_norm() > 1.0e-10);
        assert!(evidence.residual_norm() < 1.0e-9);
        assert!(evidence.continuity_residual_norm() < 1.0e-9);
        assert!(evidence.kinematic_residual_norm() < 1.0e-14);
        assert_eq!(evidence.interface_velocity_jump_norm(), 0.0);
        assert!(!evidence.interface_actions().is_empty());
        assert!(evidence.interface_action_imbalance_norm() < 1.0e-9);
        assert!(evidence.energy_balance().defect().abs() < 1.0e-9);
        assert_eq!(solution.interface_facets().len(), 2);
        assert_eq!(solution.fluid_velocity_cells().len(), 4);
        assert_eq!(solution.solid_cells().len(), 4);
    }
}

fn observe(model: &FixedReferenceFsiCartesianModel2d) -> SemanticObservation {
    SemanticObservation {
        fluid_bounds: model.fluid().bounds().map(|axis| axis.map(f64::to_bits)),
        solid_bounds: model.solid().bounds().map(|axis| axis.map(f64::to_bits)),
        fluid_density: model.fluid().mass_density().to_bits(),
        fluid_viscosity: model.fluid().dynamic_viscosity().to_bits(),
        solid_density: model.solid().mass_density().to_bits(),
        solid_mu: model.solid().shear_modulus().to_bits(),
        solid_lambda: model.solid().first_lame_parameter().to_bits(),
        interface_axis: model.interface().axis(),
        fluid_side: model.interface().fluid().side(),
        solid_side: model.interface().solid().side(),
    }
}

fn live_boundary_count(model: &FixedReferenceFsiCartesianModel2d) -> usize {
    [
        model.fluid().boundary_inventory(),
        model.solid().boundary_inventory(),
    ]
    .into_iter()
    .flat_map(|inventory| {
        [0, 1].into_iter().flat_map(move |axis| {
            [
                eqiora::kernel::BoundarySide::Lower,
                eqiora::kernel::BoundarySide::Upper,
            ]
            .into_iter()
            .map(move |side| inventory.boundary(axis, side).expect("complete boundary"))
        })
    })
    .filter(|entry| {
        matches!(
            entry.disposition(),
            PhysicalBoundaryDisposition::PortBinding { .. }
        )
    })
    .count()
}
