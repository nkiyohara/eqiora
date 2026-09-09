//! Native table admission, shared execution and ordinary Model artifact replay.
use eqiora::artifact::{ModelEnvelope, ResolvedArrayV1, decode_real_table};
use eqiora::entity::kinds;
use eqiora::graph::{EdgeKind, GraphStore, InMemoryGraphStore, Op, Transaction};
use eqiora::kernel::pure_operator::{ExactRational, PureOperatorDefinition};
use eqiora::kernel::{
    ActivationDef, ExprDagBuilder, FieldDef, FieldRole, KernelNode, ParameterDef, RelationDef,
    SymbolRef,
};
use eqiora::ontology::{Model, ModelView, OntologyId};
use eqiora::runtime::{CpuExecutor, CpuProgram};
use eqiora::sem::{Interpreter, KernelProgram, ReferenceConfig};
use eqiora::{DimExponents, Id, RawId, ScalarDomain, ValueLiteral, ValueType};
use eqiora_schema::property_table::{AcceptedRealTable, RealTableProfile};
const D: DimExponents = DimExponents::DIMENSIONLESS;
fn table(values: Vec<f64>) -> AcceptedRealTable {
    let array = ResolvedArrayV1::from_f64(vec![3, 2], values).unwrap();
    decode_real_table(
        &array.digest().unwrap(),
        Some(&array.canonical_json().unwrap()),
        Default::default(),
        D,
        D,
        RealTableProfile::PiecewiseAffineOpenIntervalsV1,
        [ExactRational::integer(0), ExactRational::integer(5)],
    )
    .unwrap()
}
fn program(definition: &PureOperatorDefinition, inputs: [f64; 2]) -> (KernelProgram, [RawId; 2]) {
    let value_type = ValueType::scalar(ScalarDomain::Real, D).unwrap();
    let activation = Id::<kinds::Activation>::new();
    let fields = [Id::<kinds::Field>::new(), Id::new()];
    let mut nodes = vec![KernelNode::from(ActivationDef::continuous(activation))];
    let mut transaction = Transaction::new("two native exact table consumers");
    for (field, input) in fields.into_iter().zip(inputs) {
        let parameter = Id::<kinds::Parameter>::new();
        let relation = Id::<kinds::Relation>::new();
        let mut builder = ExprDagBuilder::new();
        let lhs = builder.symbol(SymbolRef::Field(field)).unwrap();
        let argument = builder.symbol(SymbolRef::Parameter(parameter)).unwrap();
        let rhs = builder.pure_operator(definition, [argument]).unwrap();
        nodes.extend([
            FieldDef::new(field, value_type.clone(), FieldRole::Variable).into(),
            ParameterDef::new(
                parameter,
                ValueLiteral::from_real(value_type.clone(), input).unwrap(),
            )
            .into(),
            RelationDef::new(relation, builder.finish([lhs, rhs]).unwrap())
                .unwrap()
                .into(),
        ]);
        for dependency in [field.erase(), parameter.erase()] {
            transaction.push(Op::Connect {
                from: relation.erase(),
                to: dependency,
                edge: EdgeKind::DependsOn,
            });
        }
        transaction.push(Op::Connect {
            from: activation.erase(),
            to: relation.erase(),
            edge: EdgeKind::Activates,
        });
    }
    let members = nodes.iter().map(KernelNode::id).collect::<Vec<_>>();
    // Definitions precede connections in the submitted ordinary transaction.
    let mut accepted = Transaction::new("native exact table model");
    for node in nodes {
        accepted.push(Op::DefineKernelNode { node });
    }
    for op in transaction.ops() {
        accepted.push(op.clone());
    }
    let model = OntologyId::<Model>::new();
    accepted.push(Op::DefineOntologyView {
        view: ModelView::new(model, members, []).unwrap().into(),
    });
    let mut store = InMemoryGraphStore::new();
    store.commit(accepted).unwrap();
    (
        KernelProgram::from_snapshot(&store.snapshot(), model).unwrap(),
        fields.map(Id::erase),
    )
}
fn config() -> ReferenceConfig {
    ReferenceConfig::new(0., 1.)
        .unwrap()
        .with_nonlinear_tolerances(1e-13, 0.)
        .unwrap()
}
fn check(definition: &PureOperatorDefinition, inputs: [f64; 2], expected: [f64; 2]) {
    let (program, fields) = program(definition, inputs);
    let bytes = ModelEnvelope::from_program(&program)
        .unwrap()
        .canonical_json()
        .unwrap();
    let envelope = ModelEnvelope::from_json(&bytes, Default::default()).unwrap();
    assert_eq!(envelope.canonical_json().unwrap(), bytes);
    let reopened = envelope.to_program().unwrap();
    assert_eq!(reopened, program);
    for p in [&program, &reopened] {
        for trajectory in [
            Interpreter::new().run(p, config()).unwrap(),
            CpuExecutor::new()
                .run(&CpuProgram::lower(p).unwrap(), config())
                .unwrap(),
        ] {
            for (field, expected) in fields.into_iter().zip(expected) {
                let actual = trajectory.last_value(field).unwrap();
                assert!(
                    (actual.value() - expected).abs() <= 2e-12,
                    "{} != {expected}",
                    actual.value()
                );
                assert_eq!(actual.dim(), D);
            }
        }
    }
}
fn rejects(definition: &PureOperatorDefinition, input: f64) {
    let (program, _) = program(definition, [3., input]);
    assert!(Interpreter::new().run(&program, config()).is_err());
    assert!(
        CpuExecutor::new()
            .run(&CpuProgram::lower(&program).unwrap(), config())
            .is_err()
    );
}
#[test]
fn two_consumers_execute_exact_secants_endpoints_and_reopened_artifacts() {
    // Secants (5-1)/(2-0)=2, (2-5)/(5-2)=-1.
    let table = table(vec![0., 1., 2., 5., 5., 2.]);
    check(table.value(), [0., 5.], [1., 2.]);
    check(table.value(), [1., 3.], [3., 4.]);
    check(table.value(), [2., 4.5], [5., 2.5]);
    check(table.derivative(), [1., 4.], [2., -1.]);
    check(table.derivative(), [1., 3.], [2., -1.]);
}
#[test]
fn nonsmooth_knots_and_outside_domain_reject_without_provider_defaults() {
    let table = table(vec![0., 1., 2., 5., 5., 2.]);
    rejects(table.derivative(), 2.);
    for input in [-0.5, 5.5] {
        rejects(table.value(), input);
        rejects(table.derivative(), input);
    }
    let smooth = self::table(vec![0., 1., 2., 5., 5., 11.]);
    check(smooth.derivative(), [1., 4.], [2., 2.]);
    for input in [0., 2., 5.] {
        rejects(smooth.derivative(), input);
        rejects(table.derivative(), input);
    }
}

#[test]
fn declared_validity_is_contained_by_data_and_guards_both_calculi() {
    let array = ResolvedArrayV1::from_f64(vec![3, 2], vec![0., 1., 2., 5., 5., 2.]).unwrap();
    let admit = |low, high| {
        decode_real_table(
            &array.digest().unwrap(),
            Some(&array.canonical_json().unwrap()),
            Default::default(),
            D,
            D,
            RealTableProfile::PiecewiseAffineOpenIntervalsV1,
            [ExactRational::integer(low), ExactRational::integer(high)],
        )
    };
    let subset = admit(1, 4).unwrap();
    assert_eq!(subset.array().values(), array.f64_values().unwrap());
    check(subset.value(), [1., 4.], [3., 3.]);
    check(subset.derivative(), [1.5, 3.], [2., -1.]);
    for x in [1., 2., 4.] {
        rejects(subset.derivative(), x);
    }
    for x in [0.5, 4.5] {
        rejects(subset.value(), x);
        rejects(subset.derivative(), x);
    }
    for (low, high) in [(-1, 4), (1, 6), (3, 3), (4, 1)] {
        assert!(admit(low, high).is_err());
    }
}
