use super::contract::SemanticContract;
use super::euler::EulerPhysics;
use super::*;
use eqiora_artifact::CartesianMeshEnvelopeV1;
use eqiora_core::{DynQuantity, ScalarDomain, ValueType};
use eqiora_graph::{GraphStore, InMemoryGraphStore};
use eqiora_meshing::CartesianMesh;
use eqiora_realization::RealizationRevision;
use eqiora_schema::kernel::{DomainKind, KernelNode};
use eqiora_sem::KernelProgram;

const EULER: &str = r#"
operator lift(input value: scalar): spatial[1] = component(value);
model M() {
 domain interval = box(0, 5);
 domain lower = boundary(interval, axis=0, side=lower);
 domain upper = boundary(interval, axis=0, side=upper);
 state density: kg/m^3 on interval;
 state momentum: kg/(m^2*s) on interval;
 state energy: kg/(m*s^2) on interval;
 variable velocity: m/s on interval;
 variable pressure: kg/(m*s^2) on interval;
 parameter gamma: 1 = 2;
 relation velocity_law on interval { momentum = density * velocity; }
 relation pressure_law on interval { pressure = (gamma-1)*(energy-0.5*momentum*velocity); }
 relation mass on interval { derivative(density)+div(lift(value=momentum))=0; }
 relation momentum_law on interval { derivative(momentum)+div(lift(value=momentum*velocity+pressure))=0; }
 relation energy_law on interval { derivative(energy)+div(lift(value=velocity*(energy+pressure)))=0; }
}
"#;

fn program(source: &str) -> KernelProgram {
    let compiled = eqiora_compiler::compile("conservative.eqi", source)
        .unwrap()
        .remove(0);
    let (transaction, model, _) = compiled.into_parts();
    let mut store = InMemoryGraphStore::new();
    store.commit(transaction).unwrap();
    KernelProgram::from_snapshot(&store.snapshot(), model).unwrap()
}
fn policy(contract: &SemanticContract, revision: u64) -> RusanovPolicy {
    RusanovPolicy::new(RealizationLineage::explicit(
        contract.model().model(),
        contract.model().semantic_revision(),
        RealizationRevision::new(revision),
    ))
}
fn euler(mesh: CartesianMesh) -> ConservativeSystemAction<EulerPhysics, 1> {
    let physics = EulerPhysics::from_program(&program(EULER)).unwrap();
    make_action(physics, mesh)
}
fn make_action<L: ConservativePhysics<D>, const D: usize>(
    physics: L,
    mesh: CartesianMesh,
) -> ConservativeSystemAction<L, D> {
    let policy = policy(physics.contract(), 7);
    let components = physics.contract().components().to_vec();
    ConservativeSystemAction::new(
        physics,
        &CartesianMeshEnvelopeV1::from_mesh(&mesh).unwrap(),
        policy,
        &components,
        Space::cell_constant(),
    )
    .unwrap()
}
fn state<L: ConservativePhysics<D>, const D: usize>(
    action: &ConservativeSystemAction<L, D>,
    values: Vec<f64>,
) -> CellAverageState {
    action
        .cell_averages(
            action.physics.contract().components(),
            CellValueMeaning::Average,
            values,
        )
        .unwrap()
}
fn exterior<L: ConservativePhysics<D>, const D: usize>(
    action: &ConservativeSystemAction<L, D>,
    values: &[f64],
) -> Vec<ConservativeBoundary> {
    action
        .physics
        .contract()
        .boundaries()
        .iter()
        .map(|id| {
            action
                .exterior_state(*id, action.physics.contract().components(), values.to_vec())
                .unwrap()
        })
        .collect()
}

#[test]
fn constant_euler_preserves_all_components_on_uniform_and_nonuniform_cells() {
    for mesh in [
        CartesianMesh::uniform(&[[0.0, 5.0]], &[4]).unwrap(),
        CartesianMesh::from_axes(vec![vec![0.0, 0.25, 2.0, 5.0]]).unwrap(),
    ] {
        let action = euler(mesh);
        // gamma=2, rho=2, momentum=4, E=6 gives p=2, velocity=2,
        // physical flux (4,10,16), independently from the conservation laws.
        let state = state(&action, [2.0, 4.0, 6.0].repeat(action.cells.len()));
        let boundaries = exterior(&action, &[2.0, 4.0, 6.0]);
        let receipt = action.evaluate(&state, &boundaries).unwrap();
        assert_eq!(receipt.packets().len(), action.cells.len() + 1);
        assert!(receipt.average_rates().iter().all(|x| *x == 0.0));
        for packet in receipt.packets() {
            assert_eq!(packet.area, 1.0); // 0D face measure, not interval length.
            for (actual, expected) in packet.normal_flux().iter().zip([4.0, 10.0, 16.0]) {
                assert_eq!(*actual, packet.normal[0] * expected);
            }
            if let Some(neighbor) = packet.neighbor {
                for component in 0..3 {
                    assert_eq!(
                        packet.scatter(packet.owner, component).unwrap(),
                        -packet.scatter(neighbor, component).unwrap()
                    );
                }
            }
        }
        assert_eq!(receipt.outward_boundary_flux(), &[0.0, 0.0, 0.0]);
        action.replay(&state, &boundaries, &receipt).unwrap();
    }
}

#[test]
fn coupled_rusanov_packets_inventory_and_average_rates_have_exact_independent_oracle() {
    let action = euler(CartesianMesh::from_axes(vec![vec![0.0, 2.0, 5.0]]).unwrap());
    let state = state(&action, vec![1.0, 0.0, 2.0, 4.0, 0.0, 8.0]);
    let ids = action.physics.contract().boundaries();
    let layout = action.physics.contract().components();
    let boundaries = vec![
        action
            .exterior_state(ids[0], layout, vec![1.0, 0.0, 2.0])
            .unwrap(),
        action
            .exterior_state(ids[1], layout, vec![4.0, 0.0, 8.0])
            .unwrap(),
    ];
    // p=(2,8), u=0 and both sound speeds=2: F*=(-3,5,-6).
    let receipt = action.evaluate(&state, &boundaries).unwrap();
    let packet = receipt
        .packets()
        .iter()
        .find(|p| p.neighbor.is_some())
        .unwrap();
    assert_eq!(packet.wave_bound, Some(2.0));
    assert_eq!(packet.integrated_outward_flux(), &[-3.0, 5.0, -6.0]);
    assert_eq!(
        receipt.inventory_rates(),
        &[3.0, -3.0, 6.0, -3.0, -3.0, -6.0]
    );
    assert_eq!(receipt.average_rates(), &[1.5, -1.5, 3.0, -1.0, -1.0, -2.0]);
    let inventory = action.inventories(&state).unwrap();
    assert_eq!(inventory.values(), &[2.0, 0.0, 4.0, 12.0, 0.0, 24.0]);
    for (actual, component) in inventory.dimensions().iter().zip(layout) {
        assert_eq!(
            *actual,
            component
                .value_type
                .dimension()
                .mul(contract::length_dimension())
                .unwrap()
        );
    }
    assert_eq!(receipt.outward_boundary_flux(), &[0.0, 6.0, 0.0]);
    assert_eq!(receipt.balance_defect(), &[0.0, 0.0, 0.0]);
    // Orientation reversal swaps the states and negates physical flux, not dissipation alone.
    let reverse = action
        .rusanov(&state.values[3..], &state.values[..3], &[-1.0])
        .unwrap();
    assert_eq!(reverse.0, vec![3.0, -5.0, 6.0]);
    assert_eq!(reverse.1, 2.0);
}

#[derive(Clone, Debug)]
struct ScalarAdvection {
    contract: SemanticContract,
}
impl ConservativePhysics<2> for ScalarAdvection {
    fn contract(&self) -> &SemanticContract {
        &self.contract
    }
    fn admit(&self, state: &[f64]) -> Result<(), Diagnostic> {
        if state.len() != 1 || !state[0].is_finite() {
            return Err(invalid("scalar advection state"));
        }
        Ok(())
    }
    fn normal_flux_and_wave_bound(
        &self,
        state: &[f64],
        normal: &[f64; 2],
    ) -> Result<(Vec<DynQuantity>, DynQuantity), Diagnostic> {
        self.admit(state)?;
        assert_eq!(
            normal.iter().map(|value| value.abs()).sum::<f64>(),
            1.0,
            "the physical adapter receives only a real unit face normal"
        );
        let speed = 2.0 * normal[0] - normal[1];
        Ok((
            vec![DynQuantity::new(
                speed * state[0],
                contract::velocity_dimension(),
            )],
            DynQuantity::new(speed.abs(), contract::velocity_dimension()),
        ))
    }
}
fn scalar() -> ConservativeSystemAction<ScalarAdvection, 2> {
    let program = program(
        "model M() { domain body=box(0,5,0,4); domain left=boundary(body,axis=0,side=lower); domain right=boundary(body,axis=0,side=upper); domain bottom=boundary(body,axis=1,side=lower); domain top=boundary(body,axis=1,side=upper); state q:1 on body; variable psi:m^2/s on body; relation velocity on body { psi=2[m/s]*coordinate(0)-1[m/s]*coordinate(1); } relation transport on body { derivative(q)+div(grad(psi)*q)=0; } }",
    );
    let domain = program
        .nodes()
        .find_map(|node| match node {
            KernelNode::Domain(d) if matches!(d.kind(), DomainKind::CartesianBox { .. }) => {
                Some(d.id().erase())
            }
            _ => None,
        })
        .unwrap();
    let field = program
        .nodes()
        .find_map(|node| match node {
            KernelNode::Field(f) if f.dimension() == DimExponents::DIMENSIONLESS => {
                Some(f.id().erase())
            }
            _ => None,
        })
        .unwrap();
    let mut boundaries = vec![domain; 4];
    for node in program.nodes() {
        if let KernelNode::Domain(d) = node
            && let DomainKind::CartesianBoundary { axis, side } = d.kind()
        {
            boundaries[*axis * 2 + usize::from(*side == BoundarySide::Upper)] = d.id().erase();
        }
    }
    let contract = SemanticContract::from_program(&program, domain, &[field], &boundaries).unwrap();
    make_action(
        ScalarAdvection { contract },
        CartesianMesh::from_axes(vec![vec![0.0, 2.0, 5.0], vec![0.0, 4.0]]).unwrap(),
    )
}

#[test]
fn scalar_consumer_uses_nonunit_face_measures_and_anisotropic_volumes_once() {
    let action = scalar();
    let state = state(&action, vec![3.0, 3.0]);
    let exterior = exterior(&action, &[3.0]);
    let receipt = action.evaluate(&state, &exterior).unwrap();
    assert_eq!(receipt.average_rates(), &[0.0, 0.0]);
    assert_eq!(action.inventories(&state).unwrap().values(), &[24.0, 36.0]);
    for packet in receipt.packets() {
        let expected_flux = 3.0 * (2.0 * packet.normal[0] - packet.normal[1]);
        let expected_area = if packet.normal[0] != 0.0 {
            4.0
        } else if packet.owner == 0 {
            2.0
        } else {
            3.0
        };
        assert_eq!(packet.area, expected_area);
        assert_eq!(
            packet.integrated_outward_flux(),
            &[expected_flux * expected_area]
        );
    }
    // Typed prescribed physical flux densities reach the same action; no coordinate classifier.
    let boundaries = exterior
        .iter()
        .enumerate()
        .map(|(i, b)| {
            action
                .outward_flux(
                    b.boundary,
                    &action.flux_dimensions().unwrap(),
                    vec![[-6.0, 6.0, 3.0, -3.0][i]],
                )
                .unwrap()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        action
            .evaluate(&state, &boundaries)
            .unwrap()
            .average_rates(),
        &[0.0, 0.0]
    );
    let mut mutant = receipt.clone();
    mutant.packets[0].area *= 2.0;
    assert!(action.replay(&state, &exterior, &mutant).is_err());
    let mut mutant = receipt.clone();
    mutant.packets[0].integrated[0] /= mutant.packets[0].area;
    assert!(action.replay(&state, &exterior, &mutant).is_err());
}

#[test]
fn typed_state_boundary_mesh_model_policy_and_closure_cross_wires_reject() {
    let action = euler(CartesianMesh::uniform(&[[0.0, 5.0]], &[2]).unwrap());
    let layout = action.physics.contract().components();
    let values = vec![2.0, 4.0, 6.0, 2.0, 4.0, 6.0];
    let state = state(&action, values.clone());
    let boundaries = exterior(&action, &[2.0, 4.0, 6.0]);
    action.evaluate(&state, &boundaries).unwrap();
    let mut swapped = layout.to_vec();
    swapped.swap(0, 1);
    assert!(
        action
            .cell_averages(&swapped, CellValueMeaning::Average, values.clone())
            .is_err()
    );
    let mut wrong_type = layout.to_vec();
    wrong_type[0].value_type =
        ValueType::scalar(ScalarDomain::Real, DimExponents::DIMENSIONLESS).unwrap();
    assert!(
        action
            .cell_averages(&wrong_type, CellValueMeaning::Average, values.clone())
            .is_err()
    );
    for meaning in [
        CellValueMeaning::IntegratedInventory,
        CellValueMeaning::PointSample,
    ] {
        assert!(
            action
                .cell_averages(layout, meaning, values.clone())
                .is_err()
        );
    }
    for values in [
        vec![1.0],
        vec![0.0; 6],
        vec![1.0, 2.0, 1.0, 1.0, 2.0, 1.0],
        vec![f64::NAN; 6],
    ] {
        assert!(
            action
                .cell_averages(layout, CellValueMeaning::Average, values)
                .is_err()
        );
    }
    assert!(
        action
            .exterior_state(layout[0].field, layout, vec![2.0, 4.0, 6.0])
            .is_err()
    );
    assert!(
        action
            .exterior_state(boundaries[0].boundary, &swapped, vec![2.0, 4.0, 6.0])
            .is_err()
    );
    assert!(
        action
            .outward_flux(
                boundaries[0].boundary,
                &[DimExponents::DIMENSIONLESS; 3],
                vec![0.0; 3]
            )
            .is_err()
    );
    assert!(action.evaluate(&state, &boundaries[..1]).is_err());
    assert!(
        action
            .evaluate(&state, &[boundaries[0].clone(), boundaries[0].clone()])
            .is_err()
    );
    let different_mesh = make_action(
        action.physics.clone(),
        CartesianMesh::from_axes(vec![vec![0.0, 1.0, 5.0]]).unwrap(),
    );
    assert!(different_mesh.evaluate(&state, &boundaries).is_err());
    let mut wrong_policy = state.clone();
    wrong_policy.identity.policy = policy(action.physics.contract(), 8);
    assert!(action.evaluate(&wrong_policy, &boundaries).is_err());
    let mesh =
        CartesianMeshEnvelopeV1::from_mesh(&CartesianMesh::uniform(&[[0.0, 5.0]], &[2]).unwrap())
            .unwrap();
    let wrong_policy = RusanovPolicy::new(RealizationLineage::explicit(
        action.physics.contract().model().model(),
        eqiora_realization::SemanticRevision::new(u64::MAX),
        RealizationRevision::new(7),
    ));
    assert!(
        ConservativeSystemAction::new(
            action.physics.clone(),
            &mesh,
            wrong_policy,
            layout,
            Space::cell_constant()
        )
        .is_err()
    );
    assert!(
        ConservativeSystemAction::new(
            action.physics.clone(),
            &mesh,
            policy(action.physics.contract(), 7),
            layout,
            Space::continuous_lagrange(std::num::NonZeroU16::new(1).unwrap())
        )
        .is_err()
    );
    let wrong_mesh =
        CartesianMeshEnvelopeV1::from_mesh(&CartesianMesh::uniform(&[[0.0, 6.0]], &[2]).unwrap())
            .unwrap();
    assert!(
        ConservativeSystemAction::new(
            action.physics.clone(),
            &wrong_mesh,
            policy(action.physics.contract(), 7),
            layout,
            Space::cell_constant()
        )
        .is_err()
    );
    let foreign =
        EulerPhysics::from_program(&program(&EULER.replace("gamma: 1 = 2", "gamma: 1 = 1.4")))
            .unwrap();
    let other = make_action(
        foreign,
        CartesianMesh::uniform(&[[0.0, 5.0]], &[2]).unwrap(),
    );
    assert!(other.evaluate(&state, &boundaries).is_err());
    assert!(
        EulerPhysics::from_program(&program(&EULER.replace("0.5*momentum", "0.25*momentum")))
            .is_err()
    );
}

#[test]
fn replay_detects_every_packet_accounting_mutant_after_a_positive_action() {
    let action = euler(CartesianMesh::from_axes(vec![vec![0.0, 2.0, 5.0]]).unwrap());
    let state = state(&action, vec![1.0, 0.0, 2.0, 4.0, 0.0, 8.0]);
    let boundaries = exterior(&action, &[1.0, 0.0, 2.0]);
    let receipt = action.evaluate(&state, &boundaries).unwrap();
    action.replay(&state, &boundaries, &receipt).unwrap();
    let interior = receipt
        .packets
        .iter()
        .position(|p| p.neighbor.is_some())
        .unwrap();
    let mut mutants = vec![];
    let mut m = receipt.clone();
    m.packets[interior].wave_bound = Some(0.0);
    mutants.push(m);
    let mut m = receipt.clone();
    m.packets[interior].normal[0] *= -1.0;
    mutants.push(m);
    let mut m = receipt.clone();
    m.packets[interior].neighbor = None;
    mutants.push(m);
    let mut m = receipt.clone();
    m.packets.pop();
    mutants.push(m);
    let mut m = receipt.clone();
    m.packets.push(m.packets[0].clone());
    mutants.push(m);
    let mut m = receipt.clone();
    m.inventory_rates[3] = 0.0;
    mutants.push(m);
    let mut m = receipt.clone();
    m.average_rates[0] = m.inventory_rates[0];
    mutants.push(m);
    let mut m = receipt.clone();
    m.inventory_rates[0] *= 2.0;
    mutants.push(m);
    let mut m = receipt.clone();
    m.packets[interior].integrated[0] *= 2.0;
    mutants.push(m);
    let mut m = receipt.clone();
    m.outward_boundary_flux[1] += 1.0;
    mutants.push(m);
    for mutant in mutants {
        assert!(action.replay(&state, &boundaries, &mutant).is_err());
    }
}

#[test]
fn semantic_flux_and_wave_quantities_are_checked_before_numerical_composition() {
    #[derive(Clone)]
    struct Mutant {
        scalar: ScalarAdvection,
        mutation: usize,
    }
    impl ConservativePhysics<2> for Mutant {
        fn contract(&self) -> &SemanticContract {
            self.scalar.contract()
        }
        fn admit(&self, state: &[f64]) -> Result<(), Diagnostic> {
            self.scalar.admit(state)
        }
        fn normal_flux_and_wave_bound(
            &self,
            state: &[f64],
            normal: &[f64; 2],
        ) -> Result<(Vec<DynQuantity>, DynQuantity), Diagnostic> {
            let (mut flux, mut speed) = self.scalar.normal_flux_and_wave_bound(state, normal)?;
            match self.mutation {
                0 => flux[0] = DynQuantity::new(flux[0].value(), DimExponents::DIMENSIONLESS),
                1 => speed = DynQuantity::new(speed.value(), DimExponents::DIMENSIONLESS),
                2 => speed = DynQuantity::new(-1.0, contract::velocity_dimension()),
                3 => flux.clear(),
                _ => speed = DynQuantity::new(f64::NAN, contract::velocity_dimension()),
            }
            Ok((flux, speed))
        }
    }
    let good = scalar();
    let state = state(&good, vec![1.0, 2.0]);
    let boundary = exterior(&good, &[1.0]);
    good.evaluate(&state, &boundary).unwrap();
    for mutation in 0..5 {
        let bad = ConservativeSystemAction {
            physics: Mutant {
                scalar: good.physics.clone(),
                mutation,
            },
            identity: good.identity.clone(),
            cells: good.cells.clone(),
            faces: good.faces.clone(),
        };
        assert!(
            bad.evaluate(&state, &boundary)
                .unwrap_err()
                .message()
                .contains("semantic adapter")
        );
    }
}

#[test]
fn action_does_not_read_declaration_or_benchmark_names() {
    let original = euler(CartesianMesh::uniform(&[[0.0, 5.0]], &[2]).unwrap());
    let renamed = EULER
        .replace("model M", "model Unrelated")
        .replace("density", "rho")
        .replace("pressure", "p")
        .replace("gamma", "ratio");
    let renamed = make_action(
        EulerPhysics::from_program(&program(&renamed)).unwrap(),
        CartesianMesh::uniform(&[[0.0, 5.0]], &[2]).unwrap(),
    );
    let values = vec![1.0, 0.0, 2.0, 4.0, 0.0, 8.0];
    let first = original
        .evaluate(
            &state(&original, values.clone()),
            &exterior(&original, &[1.0, 0.0, 2.0]),
        )
        .unwrap();
    let second = renamed
        .evaluate(
            &state(&renamed, values),
            &exterior(&renamed, &[1.0, 0.0, 2.0]),
        )
        .unwrap();
    assert_ne!(
        original.identity.semantic.model,
        renamed.identity.semantic.model
    );
    assert_eq!(first.inventory_rates(), second.inventory_rates());
    assert_eq!(first.average_rates(), second.average_rates());
}

#[test]
fn accumulated_roundoff_accounting_is_distinct_from_exact_structural_scatter() {
    let action = scalar();
    let state = state(&action, vec![0.1, 0.3]);
    let boundaries = exterior(&action, &[0.7]);
    let receipt = action.evaluate(&state, &boundaries).unwrap();
    // Exact decimal arithmetic: x-exchange (-5.6,0.8,2.4), bottom
    // (0.2,0.9), top (-1.4,-2.1), hence inventory rates (6,-0.4).
    // 64 eps times the absolute exact exchange budget 14 dominates the
    // bounded multiply/add chain for this independently fixed two-cell case.
    let tolerance = 64.0 * f64::EPSILON * 14.0;
    for (actual, expected) in receipt.inventory_rates().iter().zip([6.0, -0.4]) {
        assert!((actual - expected).abs() <= tolerance);
    }
    assert!((receipt.outward_boundary_flux()[0] + 5.6).abs() <= tolerance);
    assert!(receipt.balance_defect()[0].abs() <= receipt.accounting_tolerance()[0]);
    assert!(receipt.accounting_tolerance()[0] > 0.0);
    for packet in receipt.packets() {
        if let Some(neighbor) = packet.neighbor() {
            assert_eq!(
                packet.scatter(packet.owner(), 0).unwrap(),
                -packet.scatter(neighbor, 0).unwrap()
            );
        }
    }
}

#[test]
fn shape_and_geometry_bounds_fail_before_allocating_action_work() {
    for shape in [
        (0, 1, 2),
        (17, 1, 2),
        (1, 100_001, 2),
        (1, 1, 400_001),
        (16, 100_000, 400_000),
        (usize::MAX, usize::MAX, usize::MAX),
    ] {
        assert!(validate_work::<2>(shape.0, shape.1, shape.2).is_err());
    }
    validate_work::<2>(3, 2, 3).unwrap();
    let mesh = CartesianMesh::uniform(&[[0.0, 1.0]], &[1]).unwrap();
    assert!(cartesian_fvm_geometry::<0>(&mesh).is_err());
    assert!(cartesian_fvm_geometry::<2>(&mesh).is_err());
    assert!(cartesian_fvm_geometry::<3>(&mesh).is_err());
}
