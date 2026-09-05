use eqiora_core::entity::kinds;
use eqiora_core::{DimExponents, DynQuantity, Id, OntologyId};
use eqiora_graph::{EdgeKind, GraphStore, InMemoryGraphStore, Op, Transaction};
use eqiora_schema::kernel::{
    ActivationDef, ActivationKind, EventDirection, ExprDagBuilder, FieldDef, KernelNode,
    ParameterDef, RelationDef, SymbolRef,
};
use eqiora_schema::{Model, ModelView};
use eqiora_sem::{Interpreter, KernelProgram, ReferenceConfig, Sample};

struct BouncingFixture {
    program: KernelProgram,
    height: Id<kinds::Field>,
    velocity: Id<kinds::Field>,
}

#[test]
fn falling_zero_crossing_commits_split_resets_atomically() {
    let fixture = bouncing_fixture(EventDirection::Falling, true);
    let config = ReferenceConfig::new(0.7, 0.01)
        .unwrap()
        .with_event_tolerances(1.0e-11, 1.0e-10)
        .unwrap();
    let trajectory = Interpreter::new()
        .run(&fixture.program, config)
        .expect("bouncing-ball trajectory");
    let heights = field_samples(trajectory.samples(), fixture.height);
    let velocities = field_samples(trajectory.samples(), fixture.velocity);

    let height_reset = equal_time_pair(&heights).expect("pre/post height samples");
    let velocity_reset = equal_time_pair(&velocities).expect("pre/post velocity samples");
    assert!((height_reset.0.0 - velocity_reset.0.0).abs() < 1.0e-12);
    assert!((0.43..0.46).contains(&height_reset.0.0));
    assert!(height_reset.0.1.abs() < 2.0e-9);
    assert_eq!(height_reset.1.1, 0.0);
    assert!(velocity_reset.0.1 < 0.0);
    assert!(velocity_reset.1.1 > 0.0);
    assert!((velocity_reset.1.1 + 0.8 * velocity_reset.0.1).abs() < 1.0e-8);
}

#[test]
fn crossing_direction_and_node_insertion_order_are_semantic() {
    let falling = bouncing_fixture(EventDirection::Falling, false);
    let reversed = bouncing_fixture(EventDirection::Falling, true);
    let rising = bouncing_fixture(EventDirection::Rising, true);
    let config = ReferenceConfig::new(0.55, 0.01).unwrap();

    let falling_trajectory = Interpreter::new().run(&falling.program, config).unwrap();
    let reversed_trajectory = Interpreter::new().run(&reversed.program, config).unwrap();
    let rising_trajectory = Interpreter::new().run(&rising.program, config).unwrap();

    let falling_height = falling_trajectory
        .last_value(falling.height.erase())
        .unwrap()
        .value();
    let reversed_height = reversed_trajectory
        .last_value(reversed.height.erase())
        .unwrap()
        .value();
    let falling_velocity = falling_trajectory
        .last_value(falling.velocity.erase())
        .unwrap()
        .value();
    let reversed_velocity = reversed_trajectory
        .last_value(reversed.velocity.erase())
        .unwrap()
        .value();
    assert!((falling_height - reversed_height).abs() < 1.0e-10);
    assert!((falling_velocity - reversed_velocity).abs() < 1.0e-10);
    assert!(
        equal_time_pair(&field_samples(falling_trajectory.samples(), falling.height)).is_some()
    );
    assert!(equal_time_pair(&field_samples(rising_trajectory.samples(), rising.height)).is_none());
    assert!(
        rising_trajectory
            .last_value(rising.height.erase())
            .unwrap()
            .value()
            < 0.0
    );
}

#[test]
fn coarse_step_localizes_the_backward_euler_impact_time() {
    let fixture = bouncing_fixture(EventDirection::Falling, false);
    let max_step = 0.2;
    let time_tolerance = 1.0e-11;
    let trajectory = Interpreter::new()
        .run(
            &fixture.program,
            ReferenceConfig::new(0.5, max_step)
                .unwrap()
                .with_event_tolerances(time_tolerance, 1.0e-10)
                .unwrap(),
        )
        .expect("coarse-step event trajectory");
    let impact = equal_time_pair(&field_samples(trajectory.samples(), fixture.height))
        .expect("localized pre/post impact");

    // The reference integrator is backward Euler. After the accepted step to
    // t=0.2, solve h + tau * (v - g*tau) = 0 for the positive tau.
    let gravity = 9.81;
    let velocity_at_step_start = -gravity * max_step;
    let height_at_step_start = 1.0 - gravity * max_step * max_step;
    let tau = (velocity_at_step_start
        + (velocity_at_step_start.powi(2) + 4.0 * gravity * height_at_step_start).sqrt())
        / (2.0 * gravity);
    let expected_time = max_step + tau;

    assert!((impact.0.0 - expected_time).abs() <= 2.0 * time_tolerance);
    assert!(impact.0.1.abs() <= 2.0e-9);
    assert_eq!(impact.1.1, 0.0);
}

#[test]
fn zero_time_chatter_terminates_with_a_zeno_diagnostic() {
    let program = chattering_program();
    let config = ReferenceConfig::new(0.01, 0.001)
        .unwrap()
        .with_event_tolerances(1.0e-5, 1.0e-12)
        .unwrap()
        .with_event_limits(80, 2)
        .unwrap();
    let diagnostics = Interpreter::new().run(&program, config).unwrap_err();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(
        diagnostics[0].code(),
        eqiora_core::diagnostic::codes::INVALID_EXECUTION_CONFIG
    );
    assert!(diagnostics[0].message().contains("Zeno"));
}

fn field_samples(samples: &[Sample], field: Id<kinds::Field>) -> Vec<(f64, f64)> {
    samples
        .iter()
        .filter(|sample| sample.field() == field.erase())
        .map(|sample| (sample.time(), sample.value().value()))
        .collect()
}

fn equal_time_pair(samples: &[(f64, f64)]) -> Option<((f64, f64), (f64, f64))> {
    samples
        .windows(2)
        .find_map(|pair| ((pair[0].0 - pair[1].0).abs() < 1.0e-13).then_some((pair[0], pair[1])))
}

fn bouncing_fixture(direction: EventDirection, reverse_nodes: bool) -> BouncingFixture {
    let length = DimExponents::from_integers([0, 1, 0, 0, 0, 0, 0]).expect("bounded dimension");
    let velocity_dimension =
        DimExponents::from_integers([0, 1, -1, 0, 0, 0, 0]).expect("bounded dimension");
    let acceleration_dimension =
        DimExponents::from_integers([0, 1, -2, 0, 0, 0, 0]).expect("bounded dimension");

    let height = Id::<kinds::Field>::new();
    let velocity = Id::<kinds::Field>::new();
    let gravity = Id::<kinds::Parameter>::new();
    let restitution = Id::<kinds::Parameter>::new();
    let flight = Id::<kinds::Relation>::new();
    let reset_height = Id::<kinds::Relation>::new();
    let reset_velocity = Id::<kinds::Relation>::new();
    let continuous = Id::<kinds::Activation>::new();
    let height_event = Id::<kinds::Activation>::new();
    let velocity_event = Id::<kinds::Activation>::new();
    let model = OntologyId::<Model>::new();

    let mut flight_expression = ExprDagBuilder::new();
    let height_rate = flight_expression
        .symbol(SymbolRef::Derivative(height))
        .unwrap();
    let velocity_value = flight_expression
        .symbol(SymbolRef::Field(velocity))
        .unwrap();
    let height_residual = flight_expression.sub(height_rate, velocity_value).unwrap();
    let velocity_rate = flight_expression
        .symbol(SymbolRef::Derivative(velocity))
        .unwrap();
    let gravity_value = flight_expression
        .symbol(SymbolRef::Parameter(gravity))
        .unwrap();
    let velocity_residual = flight_expression.add(velocity_rate, gravity_value).unwrap();

    let mut height_reset_expression = ExprDagBuilder::new();
    let next_height = height_reset_expression
        .symbol(SymbolRef::Next(height))
        .unwrap();
    let zero_height = height_reset_expression
        .constant(DynQuantity::new(0.0, length))
        .unwrap();
    let height_reset_residual = height_reset_expression
        .sub(next_height, zero_height)
        .unwrap();

    let mut velocity_reset_expression = ExprDagBuilder::new();
    let next_velocity = velocity_reset_expression
        .symbol(SymbolRef::Next(velocity))
        .unwrap();
    let restitution_value = velocity_reset_expression
        .symbol(SymbolRef::Parameter(restitution))
        .unwrap();
    let previous_velocity = velocity_reset_expression
        .symbol(SymbolRef::Pre(velocity))
        .unwrap();
    let reflected_velocity = velocity_reset_expression
        .mul(restitution_value, previous_velocity)
        .unwrap();
    let velocity_reset_residual = velocity_reset_expression
        .add(next_velocity, reflected_velocity)
        .unwrap();

    let event_guard = || {
        let mut guard = ExprDagBuilder::new();
        let height_value = guard.symbol(SymbolRef::Field(height)).unwrap();
        guard.finish([height_value]).unwrap()
    };

    let mut nodes = vec![
        KernelNode::from(
            FieldDef::new(height, length)
                .with_initial(DynQuantity::new(1.0, length))
                .unwrap(),
        ),
        KernelNode::from(
            FieldDef::new(velocity, velocity_dimension)
                .with_initial(DynQuantity::new(0.0, velocity_dimension))
                .unwrap(),
        ),
        KernelNode::from(ParameterDef::new(
            gravity,
            DynQuantity::new(9.81, acceleration_dimension),
        )),
        KernelNode::from(ParameterDef::new(
            restitution,
            DynQuantity::new(0.8, DimExponents::DIMENSIONLESS),
        )),
        KernelNode::from(RelationDef::new(
            flight,
            flight_expression
                .finish([height_residual, velocity_residual])
                .unwrap(),
        )),
        KernelNode::from(RelationDef::new(
            reset_height,
            height_reset_expression
                .finish([height_reset_residual])
                .unwrap(),
        )),
        KernelNode::from(RelationDef::new(
            reset_velocity,
            velocity_reset_expression
                .finish([velocity_reset_residual])
                .unwrap(),
        )),
        KernelNode::from(ActivationDef::continuous(continuous)),
        KernelNode::from(
            ActivationDef::new(
                height_event,
                ActivationKind::Event {
                    guard: event_guard(),
                    direction,
                },
            )
            .unwrap(),
        ),
        KernelNode::from(
            ActivationDef::new(
                velocity_event,
                ActivationKind::Event {
                    guard: event_guard(),
                    direction,
                },
            )
            .unwrap(),
        ),
    ];
    let members = nodes.iter().map(KernelNode::id).collect::<Vec<_>>();
    if reverse_nodes {
        nodes.reverse();
    }
    let mut transaction = Transaction::new("bouncing ball with split atomic reset");
    for node in nodes {
        transaction.push(Op::DefineKernelNode { node });
    }
    connect_dependencies(
        &mut transaction,
        flight.erase(),
        [height.erase(), velocity.erase(), gravity.erase()],
    );
    connect_dependencies(&mut transaction, reset_height.erase(), [height.erase()]);
    connect_dependencies(
        &mut transaction,
        reset_velocity.erase(),
        [velocity.erase(), restitution.erase()],
    );
    for (activation, relation) in [
        (continuous, flight),
        (height_event, reset_height),
        (velocity_event, reset_velocity),
    ] {
        transaction.push(Op::Connect {
            from: activation.erase(),
            to: relation.erase(),
            edge: EdgeKind::Activates,
        });
    }
    transaction.push(Op::DefineOntologyView {
        view: ModelView::new(model, members, []).unwrap().into(),
    });

    let mut store = InMemoryGraphStore::new();
    store.commit(transaction).unwrap();
    let program = KernelProgram::from_snapshot(&store.snapshot(), model).unwrap();
    BouncingFixture {
        program,
        height,
        velocity,
    }
}

fn chattering_program() -> KernelProgram {
    let inverse_time =
        DimExponents::from_integers([0, 0, -1, 0, 0, 0, 0]).expect("bounded dimension");
    let state = Id::<kinds::Field>::new();
    let rate = Id::<kinds::Parameter>::new();
    let reset_value = Id::<kinds::Parameter>::new();
    let flow = Id::<kinds::Relation>::new();
    let reset = Id::<kinds::Relation>::new();
    let continuous = Id::<kinds::Activation>::new();
    let event = Id::<kinds::Activation>::new();
    let model = OntologyId::<Model>::new();

    let mut flow_expression = ExprDagBuilder::new();
    let derivative = flow_expression
        .symbol(SymbolRef::Derivative(state))
        .unwrap();
    let rate_value = flow_expression.symbol(SymbolRef::Parameter(rate)).unwrap();
    let flow_residual = flow_expression.add(derivative, rate_value).unwrap();

    let mut reset_expression = ExprDagBuilder::new();
    let next = reset_expression.symbol(SymbolRef::Next(state)).unwrap();
    let reset_parameter = reset_expression
        .symbol(SymbolRef::Parameter(reset_value))
        .unwrap();
    let reset_residual = reset_expression.sub(next, reset_parameter).unwrap();

    let mut guard = ExprDagBuilder::new();
    let guard_state = guard.symbol(SymbolRef::Field(state)).unwrap();
    let event_definition = ActivationDef::new(
        event,
        ActivationKind::Event {
            guard: guard.finish([guard_state]).unwrap(),
            direction: EventDirection::Falling,
        },
    )
    .unwrap();

    let nodes = [
        KernelNode::from(
            FieldDef::new(state, DimExponents::DIMENSIONLESS)
                .with_initial(DynQuantity::new(1.0e-6, DimExponents::DIMENSIONLESS))
                .unwrap(),
        ),
        KernelNode::from(ParameterDef::new(rate, DynQuantity::new(1.0, inverse_time))),
        KernelNode::from(ParameterDef::new(
            reset_value,
            DynQuantity::new(1.0e-6, DimExponents::DIMENSIONLESS),
        )),
        KernelNode::from(RelationDef::new(
            flow,
            flow_expression.finish([flow_residual]).unwrap(),
        )),
        KernelNode::from(RelationDef::new(
            reset,
            reset_expression.finish([reset_residual]).unwrap(),
        )),
        KernelNode::from(ActivationDef::continuous(continuous)),
        KernelNode::from(event_definition),
    ];
    let members = nodes.iter().map(KernelNode::id).collect::<Vec<_>>();
    let mut transaction = Transaction::new("deliberate zero-time chatter");
    for node in nodes {
        transaction.push(Op::DefineKernelNode { node });
    }
    connect_dependencies(
        &mut transaction,
        flow.erase(),
        [state.erase(), rate.erase()],
    );
    connect_dependencies(
        &mut transaction,
        reset.erase(),
        [state.erase(), reset_value.erase()],
    );
    for (activation, relation) in [(continuous, flow), (event, reset)] {
        transaction.push(Op::Connect {
            from: activation.erase(),
            to: relation.erase(),
            edge: EdgeKind::Activates,
        });
    }
    transaction.push(Op::DefineOntologyView {
        view: ModelView::new(model, members, []).unwrap().into(),
    });
    let mut store = InMemoryGraphStore::new();
    store.commit(transaction).unwrap();
    KernelProgram::from_snapshot(&store.snapshot(), model).unwrap()
}

fn connect_dependencies<const N: usize>(
    transaction: &mut Transaction,
    relation: eqiora_core::RawId,
    dependencies: [eqiora_core::RawId; N],
) {
    for dependency in dependencies {
        transaction.push(Op::Connect {
            from: relation,
            to: dependency,
            edge: EdgeKind::DependsOn,
        });
    }
}
