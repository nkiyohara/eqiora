//! A selected Model owns every exact structural Parameter, even when no runtime DAG uses it.
use eqiora_core::{
    DimExponents, Id, OntologyId, ScalarDomain, ValueLiteral, ValueType, entity::kinds,
};
use eqiora_graph::{EdgeKind, GraphStore, InMemoryGraphStore, Op, Transaction};
use eqiora_schema::kernel::{ActivationDef, ExprDagBuilder, RelationDef};
use eqiora_schema::{
    Model, ModelView,
    kernel::{IndexSetDef, ParameterDef},
};
use eqiora_sem::KernelProgram;

#[test]
fn structural_parameter_must_be_inside_the_selected_model_closure() {
    for include_parameter in [true, false] {
        let parameter = Id::<kinds::Parameter>::new();
        let set = Id::<kinds::IndexSet>::new();
        let model = OntologyId::<Model>::new();
        let relation = Id::<kinds::Relation>::new();
        let activation = Id::<kinds::Activation>::new();
        let mut dag = ExprDagBuilder::new();
        let zero = dag
            .constant(eqiora_core::DynQuantity::new(
                0.0,
                DimExponents::DIMENSIONLESS,
            ))
            .unwrap();
        let mut transaction = Transaction::new("exact structural closure");
        transaction.push(Op::DefineKernelNode {
            node: ParameterDef::new(
                parameter,
                ValueLiteral::from_integer(
                    ValueType::scalar(ScalarDomain::Integer, DimExponents::DIMENSIONLESS).unwrap(),
                    2,
                )
                .unwrap(),
            )
            .into(),
        });
        transaction.push(Op::DefineKernelNode {
            node: IndexSetDef::new(set, 2).unwrap().into(),
        });
        transaction.push(Op::Connect {
            from: set.erase(),
            to: parameter.erase(),
            edge: EdgeKind::StructurallyDependsOn,
        });
        transaction.push(Op::DefineKernelNode {
            node: RelationDef::new(relation, dag.finish([zero, zero]).unwrap())
                .unwrap()
                .into(),
        });
        transaction.push(Op::DefineKernelNode {
            node: ActivationDef::continuous(activation).into(),
        });
        transaction.push(Op::Connect {
            from: activation.erase(),
            to: relation.erase(),
            edge: EdgeKind::Activates,
        });
        let mut members = vec![set.erase(), relation.erase(), activation.erase()];
        if include_parameter {
            members.push(parameter.erase());
        }
        transaction.push(Op::DefineOntologyView {
            view: ModelView::new(model, members, []).unwrap().into(),
        });
        let mut store = InMemoryGraphStore::new();
        store.commit(transaction).unwrap();
        let result = KernelProgram::from_snapshot(&store.snapshot(), model);
        if include_parameter {
            result.unwrap();
        } else {
            assert!(
                result.is_err(),
                "structural dependency outside Model must fail closed"
            );
        }
    }
}
