use eqiora_core::entity::kinds;
use eqiora_core::{DimExponents, DynQuantity, Id, OntologyId};
use eqiora_graph::{EdgeKind, GraphStore, InMemoryGraphStore, Op, Transaction};
use eqiora_runtime::{CanonicalEventProgram, CpuProgram};
use eqiora_schema::kernel::{
    ActivationDef, ActivationKind, EventDirection, ExprDagBuilder, FieldDef, KernelNode,
    ParameterDef, RelationDef, SymbolRef,
};
use eqiora_schema::{Model, ModelView};
use eqiora_sem::KernelProgram;

struct BouncingBall {
    kernel: KernelProgram,
    flow: Id<kinds::Relation>,
    event: Id<kinds::Activation>,
    height: Id<kinds::Field>,
    velocity: Id<kinds::Field>,
    gravity: Id<kinds::Parameter>,
    restitution: Id<kinds::Parameter>,
}

#[test]
fn canonical_bouncing_ball_produces_event_time_reset_and_saltation_derivatives() {
    let fixture = bouncing_ball(EventDirection::Falling);
    let cpu = CpuProgram::lower(&fixture.kernel).unwrap();
    let event = CanonicalEventProgram::lower(&cpu, fixture.flow, fixture.event).unwrap();

    assert_eq!(event.activations().len(), 2);
    assert_eq!(
        event.flow().state_fields(),
        [fixture.height, fixture.velocity]
    );
    assert_eq!(
        event.parameter_fields(),
        [fixture.gravity, fixture.restitution]
    );
    assert_eq!(event.parameters(), [9.81, 0.8]);

    let gravity = 9.81;
    let restitution = 0.8;
    let impact_time = (2.0_f64 / gravity).sqrt();
    let impact_velocity = -gravity * impact_time;
    let point = event
        .linearize_at(impact_time, &[0.0, impact_velocity], 1.0e-14)
        .unwrap();
    assert_eq!(point.guard_residual(), 0.0);
    assert_eq!(point.pre_state(), [0.0, impact_velocity]);
    assert_close(point.post_state()[0], 0.0);
    assert_close(point.post_state()[1], -restitution * impact_velocity);

    let derivatives = point.derivatives();
    assert_close(derivatives.guard().state_gradient()[0], 1.0);
    assert_close(derivatives.guard().state_gradient()[1], 0.0);
    assert_close(derivatives.transversality(), impact_velocity);
    assert_slice_close(
        derivatives.reset().state_jacobian(),
        &[0.0, 0.0, 0.0, -restitution],
    );
    assert_slice_close(
        derivatives.reset().parameter_jacobian(),
        &[0.0, 0.0, 0.0, -impact_velocity],
    );
    assert_close(derivatives.saltation_matrix()[0], -restitution);
    assert_close(derivatives.saltation_matrix()[1], 0.0);
    assert_close(
        derivatives.saltation_matrix()[2],
        -(1.0 + restitution) * gravity / impact_velocity,
    );
    assert_close(derivatives.saltation_matrix()[3], -restitution);

    // Fixed-time pre-impact sensitivities in row-major (state, Parameter)
    // order: gravity first, then restitution.
    let pre_sensitivity = vec![-0.5 * impact_time.powi(2), 0.0, -impact_time, 0.0];
    let propagated = derivatives.propagate_forward(&pre_sensitivity).unwrap();
    assert_slice_close(
        propagated.event_time(),
        &[-impact_time / (2.0 * gravity), 0.0],
    );
    assert_slice_close(
        propagated.post_state(),
        &[
            restitution / gravity,
            0.0,
            (restitution - 1.0) * impact_time / 2.0,
            -impact_velocity,
        ],
    );
}

#[test]
fn canonical_direction_and_guard_point_fail_closed() {
    let fixture = bouncing_ball(EventDirection::Rising);
    let cpu = CpuProgram::lower(&fixture.kernel).unwrap();
    let event = CanonicalEventProgram::lower(&cpu, fixture.flow, fixture.event).unwrap();
    let gravity = 9.81;
    let impact_time = (2.0_f64 / gravity).sqrt();
    let impact_velocity = -gravity * impact_time;

    let direction = event
        .linearize_at(impact_time, &[0.0, impact_velocity], 1.0e-14)
        .unwrap_err();
    assert!(direction.message().contains("rising direction"));

    let off_guard = event
        .linearize_at(impact_time, &[1.0e-3, impact_velocity], 1.0e-6)
        .unwrap_err();
    assert!(off_guard.message().contains("guard-localization"));
}

fn bouncing_ball(direction: EventDirection) -> BouncingBall {
    let length = DimExponents::from_integers([0, 1, 0, 0, 0, 0, 0]).expect("bounded dimension");
    let velocity_dimension =
        DimExponents::from_integers([0, 1, -1, 0, 0, 0, 0]).expect("bounded dimension");
    let acceleration_dimension =
        DimExponents::from_integers([0, 1, -2, 0, 0, 0, 0]).expect("bounded dimension");
    let height = Id::<kinds::Field>::new();
    let velocity = Id::<kinds::Field>::new();
    let gravity = Id::<kinds::Parameter>::new();
    let restitution = Id::<kinds::Parameter>::new();
    let flow = Id::<kinds::Relation>::new();
    let reset_height = Id::<kinds::Relation>::new();
    let reset_velocity = Id::<kinds::Relation>::new();
    let continuous = Id::<kinds::Activation>::new();
    let height_event = Id::<kinds::Activation>::new();
    let velocity_event = Id::<kinds::Activation>::new();
    let model = OntologyId::<Model>::new();

    let mut flow_expression = ExprDagBuilder::new();
    let height_rate = flow_expression
        .symbol(SymbolRef::Derivative(height))
        .unwrap();
    let velocity_value = flow_expression.symbol(SymbolRef::Field(velocity)).unwrap();
    let height_residual = flow_expression.sub(height_rate, velocity_value).unwrap();
    let velocity_rate = flow_expression
        .symbol(SymbolRef::Derivative(velocity))
        .unwrap();
    let gravity_value = flow_expression
        .symbol(SymbolRef::Parameter(gravity))
        .unwrap();
    let velocity_residual = flow_expression.add(velocity_rate, gravity_value).unwrap();

    let mut height_reset = ExprDagBuilder::new();
    let next_height = height_reset.symbol(SymbolRef::Next(height)).unwrap();
    let zero = height_reset
        .constant(DynQuantity::new(0.0, length))
        .unwrap();
    let height_reset_residual = height_reset.sub(next_height, zero).unwrap();

    let mut velocity_reset = ExprDagBuilder::new();
    let next_velocity = velocity_reset.symbol(SymbolRef::Next(velocity)).unwrap();
    let restitution_value = velocity_reset
        .symbol(SymbolRef::Parameter(restitution))
        .unwrap();
    let pre_velocity = velocity_reset.symbol(SymbolRef::Pre(velocity)).unwrap();
    let reflected = velocity_reset.mul(restitution_value, pre_velocity).unwrap();
    let velocity_reset_residual = velocity_reset.add(next_velocity, reflected).unwrap();

    let guard = || {
        let mut expression = ExprDagBuilder::new();
        let height = expression.symbol(SymbolRef::Field(height)).unwrap();
        expression.finish([height]).unwrap()
    };
    let nodes = vec![
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
            flow,
            flow_expression
                .finish([height_residual, velocity_residual])
                .unwrap(),
        )),
        KernelNode::from(RelationDef::new(
            reset_height,
            height_reset.finish([height_reset_residual]).unwrap(),
        )),
        KernelNode::from(RelationDef::new(
            reset_velocity,
            velocity_reset.finish([velocity_reset_residual]).unwrap(),
        )),
        KernelNode::from(ActivationDef::continuous(continuous)),
        KernelNode::from(
            ActivationDef::new(
                height_event,
                ActivationKind::Event {
                    guard: guard(),
                    direction,
                },
            )
            .unwrap(),
        ),
        KernelNode::from(
            ActivationDef::new(
                velocity_event,
                ActivationKind::Event {
                    guard: guard(),
                    direction,
                },
            )
            .unwrap(),
        ),
    ];
    let members = nodes.iter().map(KernelNode::id).collect::<Vec<_>>();
    let mut transaction = Transaction::new("canonical bouncing-ball derivatives");
    for node in nodes {
        transaction.push(Op::DefineKernelNode { node });
    }
    connect_dependencies(
        &mut transaction,
        flow.erase(),
        [height.erase(), velocity.erase(), gravity.erase()],
    );
    connect_dependencies(&mut transaction, reset_height.erase(), [height.erase()]);
    connect_dependencies(
        &mut transaction,
        reset_velocity.erase(),
        [velocity.erase(), restitution.erase()],
    );
    for (activation, relation) in [
        (continuous, flow),
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
    BouncingBall {
        kernel: KernelProgram::from_snapshot(&store.snapshot(), model).unwrap(),
        flow,
        event: height_event,
        height,
        velocity,
        gravity,
        restitution,
    }
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

fn assert_slice_close(actual: &[f64], expected: &[f64]) {
    assert_eq!(actual.len(), expected.len());
    for (actual, expected) in actual.iter().zip(expected) {
        assert_close(*actual, *expected);
    }
}

fn assert_close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() <= 1.0e-12,
        "{actual} != {expected}"
    );
}
