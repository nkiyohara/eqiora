//! Native event acceptance at the accepted-state transaction boundary.
use super::*;
use eqiora_core::{DimExponents, Id, OntologyId, ValueType, entity::kinds};
use eqiora_graph::{EdgeKind, GraphStore, InMemoryGraphStore, Op, Transaction};
use eqiora_schema::kernel::{
    ActivationDef, ActivationKind, EventDirection, ExprDagBuilder, FieldDef, FieldRole, RelationDef,
};
use eqiora_schema::{Model, ModelView};

fn program(nodes: Vec<KernelNode>, activations: &[(RawId, RawId)]) -> KernelProgram {
    let model = OntologyId::<Model>::new();
    let members = nodes.iter().map(KernelNode::id).collect::<Vec<_>>();
    let mut tx = Transaction::new("native event contract");
    for node in &nodes {
        tx.push(Op::DefineKernelNode { node: node.clone() });
    }
    for node in &nodes {
        if let KernelNode::Relation(relation) = node {
            let dependencies = relation
                .expression()
                .nodes()
                .iter()
                .filter_map(|node| match node {
                    ExprNode::Symbol(
                        SymbolRef::Field(id)
                        | SymbolRef::Derivative(id)
                        | SymbolRef::Pre(id)
                        | SymbolRef::Next(id),
                    ) => Some(id.erase()),
                    _ => None,
                })
                .collect::<BTreeSet<_>>();
            for to in dependencies {
                tx.push(Op::Connect {
                    from: relation.id().erase(),
                    to,
                    edge: EdgeKind::DependsOn,
                });
            }
        }
    }
    for &(from, to) in activations {
        tx.push(Op::Connect {
            from,
            to,
            edge: EdgeKind::Activates,
        });
    }
    tx.push(Op::DefineOntologyView {
        view: ModelView::new(model, members, []).unwrap().into(),
    });
    let mut store = InMemoryGraphStore::new();
    store.commit(tx).unwrap();
    KernelProgram::from_snapshot(&store.snapshot(), model).unwrap()
}

fn initial(field: Id<kinds::Field>, dimension: DimExponents, value: f64) -> KernelNode {
    let mut b = ExprDagBuilder::new();
    let target = b.symbol(SymbolRef::Field(field)).unwrap();
    let value = b.constant(DynQuantity::new(value, dimension)).unwrap();
    RelationDef::initial(Id::new(), b.finish([target, value]).unwrap())
        .unwrap()
        .into()
}

#[test]
fn thermostat_thresholds_rearm_and_omitted_states_remain_continuous() {
    let temperature = Id::<kinds::Field>::new();
    let rate = Id::<kinds::Field>::new();
    let kelvin = DimExponents::from_integers([0, 0, 0, 0, 1, 0, 0]).unwrap();
    let per_second = DimExponents::from_integers([0, 0, -1, 0, 0, 0, 0]).unwrap();
    let rate_dim = kelvin.mul(per_second).unwrap();
    let continuous = Id::<kinds::Activation>::new();
    let flow = Id::<kinds::Relation>::new();
    let mut b = ExprDagBuilder::new();
    let dt = b.symbol(SymbolRef::Derivative(temperature)).unwrap();
    let r = b.symbol(SymbolRef::Field(rate)).unwrap();
    let dr = b.symbol(SymbolRef::Derivative(rate)).unwrap();
    let zero = b
        .constant(DynQuantity::new(0., rate_dim.mul(per_second).unwrap()))
        .unwrap();
    let mut nodes = vec![
        FieldDef::new(
            temperature,
            ValueType::scalar(eqiora_core::ScalarDomain::Real, kelvin),
            FieldRole::State,
        )
        .into(),
        FieldDef::new(
            rate,
            ValueType::scalar(eqiora_core::ScalarDomain::Real, rate_dim),
            FieldRole::State,
        )
        .into(),
        initial(temperature, kelvin, 20.),
        initial(rate, rate_dim, 1.),
        ActivationDef::continuous(continuous).into(),
        RelationDef::new(flow, b.finish([dt, r, dr, zero]).unwrap())
            .unwrap()
            .into(),
    ];
    let mut activations = vec![(continuous.erase(), flow.erase())];
    for (threshold, direction) in [
        (22., EventDirection::Rising),
        (18., EventDirection::Falling),
    ] {
        let event = Id::<kinds::Activation>::new();
        let reset = Id::<kinds::Relation>::new();
        let mut b = ExprDagBuilder::new();
        let t = b.symbol(SymbolRef::Field(temperature)).unwrap();
        let threshold = b.constant(DynQuantity::new(threshold, kelvin)).unwrap();
        let guard = b.sub(t, threshold).unwrap();
        nodes.push(
            ActivationDef::new(
                event,
                ActivationKind::Event {
                    guard: b.finish([guard]).unwrap(),
                    direction,
                },
            )
            .unwrap()
            .into(),
        );
        let mut b = ExprDagBuilder::new();
        let next = b.symbol(SymbolRef::Next(rate)).unwrap();
        let pre = b.symbol(SymbolRef::Pre(rate)).unwrap();
        let sum = b.add(next, pre).unwrap();
        let zero = b.constant(DynQuantity::new(0., rate_dim)).unwrap();
        nodes.push(
            RelationDef::new(reset, b.finish([sum, zero]).unwrap())
                .unwrap()
                .into(),
        );
        activations.push((event.erase(), reset.erase()));
    }
    let program = program(nodes, &activations);
    let config = ReferenceConfig::new(12., 0.25)
        .unwrap()
        .with_nonlinear_tolerances(1e-12, 0.)
        .unwrap()
        .with_event_tolerances(1e-11, 1e-10)
        .unwrap();
    let trajectory = Interpreter::new().run(&program, config).unwrap();
    // Piecewise constant rates integrate exactly: heating reaches22 at2s,
    // cooling reaches18 at6s, reheating reaches22 at10s, cooling returns20 at12s.
    assert!((trajectory.last_value(temperature.erase()).unwrap().value() - 20.).abs() < 1e-8);
    assert_eq!(trajectory.last_value(rate.erase()).unwrap().value(), -1.);
    for (field, expected) in [
        (
            temperature,
            [(2., 22., 22.), (6., 18., 18.), (10., 22., 22.)],
        ),
        (rate, [(2., 1., -1.), (6., -1., 1.), (10., 1., -1.)]),
    ] {
        let samples = trajectory
            .samples()
            .iter()
            .filter(|s| s.field() == field.erase())
            .collect::<Vec<_>>();
        let resets = samples
            .windows(2)
            .filter(|p| p[0].time() == p[1].time())
            .collect::<Vec<_>>();
        assert_eq!(
            resets.len(),
            3,
            "reset onto the guard must not immediately retrigger"
        );
        for (pair, (time, before, after)) in resets.iter().zip(expected) {
            assert!((pair[0].time() - time).abs() < 2e-9);
            assert!((pair[0].value().value() - before).abs() < 2e-9);
            assert!((pair[1].value().value() - after).abs() < 2e-9);
        }
    }
}

#[test]
fn grouped_reset_and_post_reset_consistency_failures_leave_accepted_state_unchanged() {
    for conflicting_reset in [false, true] {
        let state = Id::<kinds::Field>::new();
        let algebraic = Id::<kinds::Field>::new();
        let companion = Id::<kinds::Field>::new();
        let continuous = Id::<kinds::Activation>::new();
        let event = Id::<kinds::Activation>::new();
        let flow = Id::<kinds::Relation>::new();
        let reset = Id::<kinds::Relation>::new();
        let d = DimExponents::DIMENSIONLESS;
        let mut b = ExprDagBuilder::new();
        let x = b.symbol(SymbolRef::Field(state)).unwrap();
        let y = b.symbol(SymbolRef::Field(algebraic)).unwrap();
        let product = b.mul(x, y).unwrap();
        let one = b.constant(DynQuantity::new(1., d)).unwrap();
        let dx = b.symbol(SymbolRef::Derivative(state)).unwrap();
        let dc = b.symbol(SymbolRef::Derivative(companion)).unwrap();
        let zero = b
            .constant(DynQuantity::new(
                0.,
                DimExponents::from_integers([0, 0, -1, 0, 0, 0, 0]).unwrap(),
            ))
            .unwrap();
        let mut nodes = vec![
            FieldDef::new(
                state,
                ValueType::scalar(eqiora_core::ScalarDomain::Real, d),
                FieldRole::State,
            )
            .into(),
            FieldDef::new(
                algebraic,
                ValueType::scalar(eqiora_core::ScalarDomain::Real, d),
                FieldRole::Variable,
            )
            .into(),
            initial(state, d, 1.),
            FieldDef::new(
                companion,
                ValueType::scalar(eqiora_core::ScalarDomain::Real, d),
                FieldRole::State,
            )
            .into(),
            initial(companion, d, 7.),
            ActivationDef::continuous(continuous).into(),
            RelationDef::new(flow, b.finish([dx, zero, dc, zero, product, one]).unwrap())
                .unwrap()
                .into(),
        ];
        let mut b = ExprDagBuilder::new();
        let x = b.symbol(SymbolRef::Field(state)).unwrap();
        nodes.push(
            ActivationDef::new(
                event,
                ActivationKind::Event {
                    guard: b.finish([x]).unwrap(),
                    direction: EventDirection::Falling,
                },
            )
            .unwrap()
            .into(),
        );
        let mut b = ExprDagBuilder::new();
        let next = b.symbol(SymbolRef::Next(state)).unwrap();
        let zero = b.constant(DynQuantity::new(0., d)).unwrap();
        nodes.push(
            RelationDef::new(reset, b.finish([next, zero]).unwrap())
                .unwrap()
                .into(),
        );
        let mut activations = vec![
            (continuous.erase(), flow.erase()),
            (event.erase(), reset.erase()),
        ];
        if conflicting_reset {
            let conflict = Id::<kinds::Relation>::new();
            let mut b = ExprDagBuilder::new();
            let next = b.symbol(SymbolRef::Next(state)).unwrap();
            let other = b.symbol(SymbolRef::Next(companion)).unwrap();
            let zero = b.constant(DynQuantity::new(0., d)).unwrap();
            let cancelled = b.mul(zero, other).unwrap();
            let conflicting = b.add(next, cancelled).unwrap();
            let two = b.constant(DynQuantity::new(2., d)).unwrap();
            nodes.push(
                RelationDef::new(conflict, b.finish([conflicting, two]).unwrap())
                    .unwrap()
                    .into(),
            );
            activations.push((event.erase(), conflict.erase()));
        }
        let program = program(nodes, &activations);
        let plan = ExecutionPlan::new(&program).unwrap();
        let mut accepted = RuntimeState::new(&program, &plan).unwrap();
        // The bilinear consistency law needs a nonsingular numerical seed.
        let config = ReferenceConfig::new(1., 0.1)
            .unwrap()
            .with_initial_guess(1.)
            .unwrap();
        solve_initialization(
            &program,
            &plan,
            &mut accepted,
            config,
            &ReferenceExpressionBackend,
        )
        .unwrap();
        let before = accepted.clone();
        let error = execute_activated_relations(
            &program,
            &plan,
            &mut accepted,
            0.,
            &plan.events[0].relations,
            "event-activation",
            config,
            &ReferenceExpressionBackend,
        )
        .unwrap_err();
        assert_eq!(error.code(), codes::NONLINEAR_SOLVE_FAILED);
        assert_eq!(accepted.fields, before.fields);
        assert_eq!(accepted.derivatives, before.derivatives);
        assert_eq!(accepted.ports, before.ports);
        assert_eq!(accepted.typed_fields, before.typed_fields);
        assert_eq!(accepted.typed_ports, before.typed_ports);
        assert_eq!(accepted.typed_next, before.typed_next);
        assert_eq!(accepted.physical, before.physical);
    }
}

#[test]
fn reference_config_rejects_non_advancing_steps() {
    let diagnostic = ReferenceConfig::new(1.0, 0.0).expect_err("zero step");

    assert_eq!(diagnostic.code(), codes::INVALID_EXECUTION_CONFIG);
}
