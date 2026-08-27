use eqiora::entity::kinds;
use eqiora::graph::{EdgeKind, GraphStore, InMemoryGraphStore, Op, Transaction};
use eqiora::kernel::{
    ActivationDef, ExprDagBuilder, FieldDef, KernelNode, ParameterDef, RelationDef, SymbolRef,
};
use eqiora::ontology::{Model, ModelView, OntologyId};
use eqiora::{DimExponents, DynQuantity, Id};

#[allow(dead_code)] // Integration-test crates consume disjoint shared fixtures.
pub(crate) mod exact_package;

#[allow(dead_code)] // Integration-test crates consume disjoint shared fixtures.
pub(crate) mod connection_set_conformance;

#[allow(dead_code)] // Integration-test crates consume disjoint shared fixtures.
pub(crate) mod common_scalar_plan;

#[allow(dead_code)] // Integration-test crates consume disjoint shared fixtures.
pub(crate) mod fixed_reference_fsi;

#[allow(dead_code)] // Integration-test crates consume disjoint shared fixtures.
pub(crate) struct CanonicalStateDependentMassDae {
    pub(crate) kernel: eqiora::sem::KernelProgram,
    pub(crate) relation: Id<kinds::Relation>,
    pub(crate) differential: Id<kinds::Field>,
    pub(crate) algebraic: Id<kinds::Field>,
    pub(crate) rate: Id<kinds::Parameter>,
}

#[allow(dead_code)] // Integration-test crates consume disjoint shared fixtures.
pub(crate) fn canonical_state_dependent_mass_dae() -> CanonicalStateDependentMassDae {
    let inverse_time = DimExponents {
        time: -1,
        ..DimExponents::DIMENSIONLESS
    };
    let differential = Id::<kinds::Field>::new();
    let algebraic = Id::<kinds::Field>::new();
    let rate = Id::<kinds::Parameter>::new();
    let relation = Id::<kinds::Relation>::new();
    let continuous = Id::<kinds::Activation>::new();
    let model = OntologyId::<Model>::new();

    let mut expression = ExprDagBuilder::new();
    let derivative = expression
        .symbol(SymbolRef::Derivative(differential))
        .unwrap();
    let differential_value = expression.symbol(SymbolRef::Field(differential)).unwrap();
    let algebraic_value = expression.symbol(SymbolRef::Field(algebraic)).unwrap();
    let rate_value = expression.symbol(SymbolRef::Parameter(rate)).unwrap();
    let one = expression
        .constant(DynQuantity::new(1.0, DimExponents::DIMENSIONLESS))
        .unwrap();
    let coefficient = expression.add(one, algebraic_value).unwrap();
    let decay = expression.mul(rate_value, differential_value).unwrap();
    let differential_sum = expression.add(derivative, decay).unwrap();
    let differential_residual = expression.mul(coefficient, differential_sum).unwrap();
    let square = expression
        .mul(differential_value, differential_value)
        .unwrap();
    let algebraic_residual = expression.sub(algebraic_value, square).unwrap();

    let nodes = [
        KernelNode::from(
            FieldDef::new(differential, DimExponents::DIMENSIONLESS)
                .with_initial(DynQuantity::new(1.0, DimExponents::DIMENSIONLESS))
                .unwrap(),
        ),
        KernelNode::from(
            FieldDef::new(algebraic, DimExponents::DIMENSIONLESS)
                .with_initial(DynQuantity::new(0.0, DimExponents::DIMENSIONLESS))
                .unwrap(),
        ),
        KernelNode::from(ParameterDef::new(rate, DynQuantity::new(1.0, inverse_time))),
        KernelNode::from(RelationDef::new(
            relation,
            expression
                .finish([differential_residual, algebraic_residual])
                .unwrap(),
        )),
        KernelNode::from(ActivationDef::continuous(continuous)),
    ];
    let members = nodes.iter().map(KernelNode::id).collect::<Vec<_>>();
    let mut transaction = Transaction::new("canonical state-dependent-mass index-one DAE");
    for node in nodes {
        transaction.push(Op::DefineKernelNode { node });
    }
    for dependency in [differential.erase(), algebraic.erase(), rate.erase()] {
        transaction.push(Op::Connect {
            from: relation.erase(),
            to: dependency,
            edge: EdgeKind::DependsOn,
        });
    }
    transaction
        .push(Op::Connect {
            from: continuous.erase(),
            to: relation.erase(),
            edge: EdgeKind::Activates,
        })
        .push(Op::DefineOntologyView {
            view: ModelView::new(model, members, []).unwrap().into(),
        });

    let mut store = InMemoryGraphStore::new();
    store.commit(transaction).unwrap();
    CanonicalStateDependentMassDae {
        kernel: eqiora::sem::KernelProgram::from_snapshot(&store.snapshot(), model).unwrap(),
        relation,
        differential,
        algebraic,
        rate,
    }
}
