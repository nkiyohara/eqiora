//! Ordinary checked source properties share batched value and active derivative execution.
use eqiora::api::ModelDocument;
use eqiora::ir::{DifferentiationRole, LinearizedRelation, RelationTangent, ScalarOperatorIr};
use eqiora::kernel::{ExprDagBuilder, ExprId, ExprNode, KernelNode, SymbolRef};
use eqiora::runtime::{CpuExecutor, CpuProgram};
use eqiora::sem::{Interpreter, ReferenceConfig};
use eqiora::{DimExponents, ScalarDomain, ValueLiteral, ValueType};

const CONDUCTANCE: &str = r#"
operator law(input v:V,input forward:S,input reverse:S):A = if v > 0[V] then forward*v else reverse*v;
model Device(){parameter voltage:V=-4;parameter forward:S=2;parameter reverse:S=0.5;variable current:A;
relation constitutive{current=law(reverse=reverse,v=voltage,forward=forward);}}
"#;
fn real(dimension: DimExponents, value: f64) -> ValueLiteral {
    ValueLiteral::from_real(ValueType::scalar(ScalarDomain::Real, dimension), value).unwrap()
}
fn voltage() -> DimExponents {
    DimExponents::from_integers([1, 2, -3, -1, 0, 0, 0]).unwrap()
}
fn current() -> DimExponents {
    DimExponents::from_integers([0, 0, 0, 1, 0, 0, 0]).unwrap()
}
fn scalar(value: &ValueLiteral) -> f64 {
    value.real_scalar_value().unwrap().value()
}

// Select the retained application from an accepted Model; preserve its original
// Parameter identities and definition. This does not evaluate a fabricated model.
fn property_ir(document: &ModelDocument) -> (ScalarOperatorIr, ExprId) {
    let expression = document
        .program()
        .nodes()
        .find_map(|node| match node {
            KernelNode::Relation(r) if !r.is_initial() => Some(r.expression()),
            _ => None,
        })
        .unwrap();
    let application = expression
        .nodes()
        .iter()
        .find_map(|node| match node {
            ExprNode::PureOperatorApplication(app) => Some(app),
            _ => None,
        })
        .unwrap();
    let definition = expression.definition(application.definition()).unwrap();
    let mut builder = ExprDagBuilder::new();
    let arguments = application
        .arguments()
        .iter()
        .map(|id| {
            let ExprNode::Symbol(SymbolRef::Parameter(id)) = expression.node(*id).unwrap() else {
                panic!("fixture arguments must be the authored Parameter identities")
            };
            builder.symbol(SymbolRef::Parameter(*id)).unwrap()
        })
        .collect::<Vec<_>>();
    let root = builder.pure_operator(definition, arguments).unwrap();
    let dag = builder.finish([root]).unwrap();
    (ScalarOperatorIr::lower(&dag).unwrap(), root)
}
fn row(
    document: &ModelDocument,
    ir: &ScalarOperatorIr,
    name: &str,
    value: ValueLiteral,
) -> Vec<ValueLiteral> {
    ir.symbols()
        .iter()
        .map(|symbol| {
            let SymbolRef::Parameter(id) = symbol else {
                panic!("property formals are Parameters")
            };
            if id.erase() == document.aliases()[name] {
                value.clone()
            } else {
                document.program().typed_value(id.erase()).unwrap().clone()
            }
        })
        .collect()
}

#[test]
fn source_conductance_batches_match_reference_cpu_and_point_bound_slopes() {
    let document = ModelDocument::compile("piecewise-conductance.eqi", CONDUCTANCE).unwrap();
    let (ir, root) = property_ir(&document);
    let rows = [-4., 0., 3.].map(|v| row(&document, &ir, "voltage", real(voltage(), v)));
    let outputs = ir.evaluate_typed_batch(&[root], &rows).unwrap();
    assert_eq!(
        outputs
            .iter()
            .map(|output| scalar(&output[0]))
            .collect::<Vec<_>>(),
        [-2., 0., 6.]
    );
    assert!(
        outputs
            .iter()
            .all(|output| output[0].value_type().dimension() == current())
    );
    let roles = ir
        .symbols()
        .iter()
        .map(|symbol| match symbol {
            SymbolRef::Parameter(id) if id.erase() == document.aliases()["voltage"] => {
                DifferentiationRole::Unknown
            }
            _ => DifferentiationRole::Parameter,
        })
        .collect::<Vec<_>>();
    assert!(ir.linearize_typed(&rows[1], &roles).is_err());
    for (index, slope) in [(0, 0.5), (2, 2.)] {
        let linear = ir.linearize_typed(&rows[index], &roles).unwrap();
        let mut result = [0.];
        linear
            .jvp(RelationTangent::Unknown(&[1.]), &mut result)
            .unwrap();
        assert_eq!(result, [slope]);
    }
    let config = ReferenceConfig::new(0., 1.)
        .unwrap()
        .with_nonlinear_tolerances(1e-13, 0.)
        .unwrap();
    for (v, expected) in [(-4., -2.), (0., 0.), (3., 6.)] {
        let changed = (v != -4.).then(|| {
            let edit = document
                .preview_value_edit(document.aliases()["voltage"], real(voltage(), v))
                .unwrap();
            document.commit_value_edit(edit).unwrap().into_document()
        });
        let edited = changed.as_ref().unwrap_or(&document);
        for trajectory in [
            Interpreter::new().run(edited.program(), config).unwrap(),
            CpuExecutor::new()
                .run(&CpuProgram::lower(edited.program()).unwrap(), config)
                .unwrap(),
        ] {
            let actual = trajectory.last_value(edited.aliases()["current"]).unwrap();
            assert!((actual.value() - expected).abs() < 2e-12);
            assert_eq!(actual.dim(), current());
        }
    }
}

#[test]
fn source_guarded_sqrt_uses_one_lazy_batch_and_derivative_path() {
    let source = r#"operator guarded(input x:V^2):V=if x>=0[V^2] then math.sqrt(x) else 0[V];
model Guarded(){parameter x:V^2=4;variable y:V;relation value{y=guarded(x=x);}}"#;
    let document = ModelDocument::compile("guarded-sqrt.eqi", source).unwrap();
    let (ir, root) = property_ir(&document);
    let square = voltage().pow(2, 1).unwrap();
    let rows = [4., -1., 0.].map(|x| row(&document, &ir, "x", real(square, x)));
    let outputs = ir.evaluate_typed_batch(&[root], &rows).unwrap();
    assert_eq!(
        outputs
            .iter()
            .map(|output| scalar(&output[0]))
            .collect::<Vec<_>>(),
        [2., 0., 0.]
    );
    for (index, slope) in [(0, 0.25), (1, 0.)] {
        let linear = ir
            .linearize_typed(&rows[index], &[DifferentiationRole::Unknown])
            .unwrap();
        let mut derivative = [0.];
        linear
            .jvp(RelationTangent::Unknown(&[1.]), &mut derivative)
            .unwrap();
        assert_eq!(derivative, [slope]);
    }
    assert!(
        ir.linearize_typed(&rows[2], &[DifferentiationRole::Unknown])
            .is_err()
    );
    assert!(
        outputs
            .iter()
            .all(|output| output[0].value_type().dimension() == voltage())
    );
}

fn crossing_program(
    with_event: bool,
) -> (eqiora::sem::KernelProgram, eqiora::RawId, eqiora::RawId) {
    use eqiora::Id;
    use eqiora::entity::kinds;
    use eqiora::graph::{EdgeKind, GraphStore, InMemoryGraphStore, Op, Transaction};
    use eqiora::kernel::{
        ActivationDef, ActivationKind, EventDirection, FieldDef, FieldRole, RelationDef,
    };
    use eqiora::ontology::OntologyId;
    use eqiora::ontology::{Model, ModelView};
    let state = Id::<kinds::Field>::new();
    let output = Id::<kinds::Field>::new();
    let flow = Id::<kinds::Relation>::new();
    let initial = Id::<kinds::Relation>::new();
    let continuous = Id::<kinds::Activation>::new();
    let d = DimExponents::DIMENSIONLESS;
    let mut b = ExprDagBuilder::new();
    let x = b.symbol(SymbolRef::Field(state)).unwrap();
    let y = b.symbol(SymbolRef::Field(output)).unwrap();
    let dx = b.symbol(SymbolRef::Derivative(state)).unwrap();
    let rate = b
        .constant(real(
            DimExponents::from_integers([0, 0, -1, 0, 0, 0, 0]).unwrap(),
            -1.,
        ))
        .unwrap();
    let zero = b.constant(real(d, 0.)).unwrap();
    let two = b.constant(real(d, 2.)).unwrap();
    let half = b.constant(real(d, 0.5)).unwrap();
    let condition = b
        .compare(eqiora::kernel::ComparisonOp::Greater, x, zero)
        .unwrap();
    let forward = b.mul(two, x).unwrap();
    let reverse = b.mul(half, x).unwrap();
    let law = b.select(condition, forward, reverse).unwrap();
    let equations = b.finish([dx, rate, y, law]).unwrap();
    let mut b = ExprDagBuilder::new();
    let x = b.symbol(SymbolRef::Field(state)).unwrap();
    let one = b.constant(real(d, 1.)).unwrap();
    let init = b.finish([x, one]).unwrap();
    let mut nodes = vec![
        KernelNode::from(FieldDef::new(
            state,
            ValueType::scalar(ScalarDomain::Real, d),
            FieldRole::State,
        )),
        FieldDef::new(
            output,
            ValueType::scalar(ScalarDomain::Real, d),
            FieldRole::Variable,
        )
        .into(),
        RelationDef::new(flow, equations).unwrap().into(),
        RelationDef::initial(initial, init).unwrap().into(),
        ActivationDef::continuous(continuous).into(),
    ];
    let mut edges = vec![
        (flow.erase(), state.erase(), EdgeKind::DependsOn),
        (flow.erase(), output.erase(), EdgeKind::DependsOn),
        (initial.erase(), state.erase(), EdgeKind::DependsOn),
        (continuous.erase(), flow.erase(), EdgeKind::Activates),
    ];
    if with_event {
        let event = Id::<kinds::Activation>::new();
        let reset = Id::<kinds::Relation>::new();
        let mut b = ExprDagBuilder::new();
        let guard = b.symbol(SymbolRef::Field(state)).unwrap();
        nodes.push(
            ActivationDef::new(
                event,
                ActivationKind::Event {
                    guard: b.finish([guard]).unwrap(),
                    direction: EventDirection::Falling,
                },
            )
            .unwrap()
            .into(),
        );
        let mut b = ExprDagBuilder::new();
        let next = b.symbol(SymbolRef::Next(state)).unwrap();
        let one = b.constant(real(d, 1.)).unwrap();
        nodes.push(
            RelationDef::new(reset, b.finish([next, one]).unwrap())
                .unwrap()
                .into(),
        );
        edges.extend([
            (event.erase(), reset.erase(), EdgeKind::Activates),
            (reset.erase(), state.erase(), EdgeKind::DependsOn),
        ]);
    }
    let members = nodes.iter().map(KernelNode::id).collect::<Vec<_>>();
    let model = OntologyId::<Model>::new();
    let mut transaction =
        Transaction::new("separately declared crossing event with memoryless property");
    for node in nodes {
        transaction.push(Op::DefineKernelNode { node });
    }
    for (from, to, edge) in edges {
        transaction.push(Op::Connect { from, to, edge });
    }
    transaction.push(Op::DefineOntologyView {
        view: ModelView::new(model, members, []).unwrap().into(),
    });
    let mut store = InMemoryGraphStore::new();
    store.commit(transaction).unwrap();
    (
        eqiora::sem::KernelProgram::from_snapshot(&store.snapshot(), model).unwrap(),
        state.erase(),
        output.erase(),
    )
}

#[test]
fn memoryless_branch_crossing_requires_a_separately_declared_reset_event() {
    // Constant dx/dt=-1 is integrated exactly by backward Euler. Without an
    // event x=1-t. The explicit event resets x from0 to1 at t=1.
    let config = ReferenceConfig::new(1.5, 0.6)
        .unwrap()
        .with_nonlinear_tolerances(1e-12, 0.)
        .unwrap()
        .with_event_tolerances(1e-11, 1e-10)
        .unwrap();
    for (with_event, expected_x, expected_y) in [(false, -0.5, -0.25), (true, 0.5, 1.)] {
        let (program, state, output) = crossing_program(with_event);
        let trajectory = Interpreter::new().run(&program, config).unwrap();
        assert!((trajectory.last_value(state).unwrap().value() - expected_x).abs() < 2e-8);
        assert!((trajectory.last_value(output).unwrap().value() - expected_y).abs() < 4e-8);
        let samples = trajectory
            .samples()
            .iter()
            .filter(|sample| sample.field() == state)
            .collect::<Vec<_>>();
        let reset = samples
            .windows(2)
            .find(|pair| (pair[0].time() - pair[1].time()).abs() < 1e-13);
        if with_event {
            let pair = reset.expect("explicit event records pre/post samples");
            assert!((pair[0].time() - 1.).abs() < 2e-9);
            assert!(pair[0].value().value().abs() < 2e-9);
            assert_eq!(pair[1].value().value(), 1.);
        } else {
            assert!(
                reset.is_none(),
                "memoryless selection must not synthesize an event"
            );
        }
    }
}
