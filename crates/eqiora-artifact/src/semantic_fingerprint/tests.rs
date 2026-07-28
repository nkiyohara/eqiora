use super::*;
use eqiora_compiler::compile;
use eqiora_core::entity::kinds;
use eqiora_core::{Id, OntologyId};
use eqiora_graph::{EdgeKind, GraphStore, InMemoryGraphStore, Op, Transaction};
use eqiora_schema::kernel::{
    ActivationDef, ExprDagBuilder, FieldDef, PortDef, RelationDef, SignalDirection, SymbolRef,
};
use eqiora_schema::{Model, ModelView};

const DECAY: &str = r#"
model decay {
  field x: 1 = 1;
  parameter rate: 1 / s = 1;
  relation flow continuous { derivative(x) + rate * x = 0; }
}
"#;

fn program(source: &str) -> KernelProgram {
    let compiled = compile("fingerprint.eqi", source)
        .expect("valid source")
        .remove(0);
    let (transaction, model, _) = compiled.into_parts();
    let mut store = InMemoryGraphStore::new();
    store.commit(transaction).expect("valid graph transaction");
    KernelProgram::from_snapshot(&store.snapshot(), model).expect("valid kernel program")
}

#[test]
fn independent_occurrence_ids_and_formatting_do_not_change_the_projection() {
    let first = program(DECAY);
    let second = program(
        "model renamed { parameter r: 1/s = 1; field state: 1=1;\nrelation balance continuous { derivative(state)+r*state=0; } }",
    );

    assert_ne!(first.model(), second.model());
    assert_eq!(
        StructuralSemanticFingerprint::from_program(&first).unwrap(),
        StructuralSemanticFingerprint::from_program(&second).unwrap()
    );
    assert!(structurally_equivalent(&first, &second).unwrap());
}

#[test]
fn symmetric_graphs_choose_the_same_exact_label_across_fresh_ids_and_order() {
    let first = program(
        "model first { parameter a: 1 = 1; parameter b: 1 = 1; relation r continuous { 0 = 0; } }",
    );
    let second = program(
        "model second { relation balance continuous { 0 = 0; } parameter y: 1 = 1; parameter x: 1 = 1; }",
    );
    assert!(structurally_equivalent(&first, &second).unwrap());
    assert_eq!(
        StructuralSemanticFingerprint::from_program(&first).unwrap(),
        StructuralSemanticFingerprint::from_program(&second).unwrap()
    );
}

#[test]
fn expression_arena_allocation_is_alpha_normalized_too() {
    let first = manually_allocated_expression(false, false);
    let reversed = manually_allocated_expression(true, false);
    assert!(structurally_equivalent(&first, &reversed).unwrap());
}

#[test]
fn model_boundary_membership_is_not_alpha_normalized_away() {
    let internal = manually_allocated_expression(false, false);
    let exposed = manually_allocated_expression(false, true);
    assert!(!structurally_equivalent(&internal, &exposed).unwrap());
}

#[test]
fn value_operator_and_rewiring_changes_are_not_alpha_normalized_away() {
    let baseline = program(DECAY);
    let changed_value = program(&DECAY.replace("= 1;\n  relation", "= 2;\n  relation"));
    let changed_operator = program(&DECAY.replace("rate * x", "rate / x"));
    assert!(!structurally_equivalent(&baseline, &changed_value).unwrap());
    assert!(!structurally_equivalent(&baseline, &changed_operator).unwrap());

    let separate = program(
        "model p { parameter a: 1 = 2; parameter b: 1 = 2; relation r continuous { a-b=0; } }",
    );
    let aliased = program(
        "model p { parameter a: 1 = 2; parameter b: 1 = 2; relation r continuous { a-a=0; } }",
    );
    assert!(!structurally_equivalent(&separate, &aliased).unwrap());
}

#[test]
fn nominally_distinct_equal_domains_remain_distinct_vertices() {
    let distinct = program(
        r#"
model network {
  domain a = scalar_physical(across = 1, through = 1);
  domain b = scalar_physical(across = 1, through = 1);
  port a1: conserving on a;
  port a2: conserving on a;
  port b1: conserving on b;
  port b2: conserving on b;
  relation ra continuous { across(a1) - across(a2) = 0; through(a1) + through(a2) = 0; }
  relation rb continuous { across(b1) - across(b2) = 0; through(b1) + through(b2) = 0; }
  connect conserving a1, a2;
  connect conserving b1, b2;
}
"#,
    );
    let shared = program(
        r#"
model network {
  domain a = scalar_physical(across = 1, through = 1);
  domain b = scalar_physical(across = 1, through = 1);
  port a1: conserving on a;
  port a2: conserving on a;
  port b1: conserving on a;
  port b2: conserving on a;
  relation ra continuous { across(a1) - across(a2) = 0; through(a1) + through(a2) = 0; }
  relation rb continuous { across(b1) - across(b2) = 0; through(b1) + through(b2) = 0; }
  connect conserving a1, a2;
  connect conserving b1, b2;
}
"#,
    );
    assert!(!structurally_equivalent(&distinct, &shared).unwrap());
}

#[test]
fn exact_canonicalization_fails_instead_of_using_occurrence_order() {
    let value = program(DECAY);
    let limits = SemanticFingerprintLimits {
        max_search_states: 1,
        ..SemanticFingerprintLimits::default()
    };
    let result = StructuralSemanticFingerprint::from_program_with_limits(&value, limits);
    assert!(
        result.is_ok(),
        "ordinary asymmetric models need no search branch"
    );

    let symmetric = program(
        "model symmetric { parameter a: 1 = 1; parameter b: 1 = 1; relation r continuous { 0 = 0; } }",
    );
    let error = StructuralSemanticFingerprint::from_program_with_limits(&symmetric, limits)
        .expect_err("ambiguous exact labeling must respect the state limit");
    assert!(error.message().contains("search-state limit"));
}

fn manually_allocated_expression(reverse: bool, expose_port: bool) -> KernelProgram {
    let left = Id::<kinds::Field>::new();
    let right = Id::<kinds::Field>::new();
    let relation = Id::<kinds::Relation>::new();
    let activation = Id::<kinds::Activation>::new();
    let port = Id::<kinds::Port>::new();
    let model = OntologyId::<Model>::new();
    let mut expression = ExprDagBuilder::new();
    let (left_value, right_value) = if reverse {
        let right_value = expression.symbol(SymbolRef::Field(right)).unwrap();
        let left_value = expression.symbol(SymbolRef::Field(left)).unwrap();
        (left_value, right_value)
    } else {
        let left_value = expression.symbol(SymbolRef::Field(left)).unwrap();
        let right_value = expression.symbol(SymbolRef::Field(right)).unwrap();
        (left_value, right_value)
    };
    let root = expression.add(left_value, right_value).unwrap();
    let expression = expression.finish([root]).unwrap();
    let members = [
        left.erase(),
        right.erase(),
        relation.erase(),
        activation.erase(),
        port.erase(),
    ];
    let boundary = expose_port.then_some(port.erase());
    let view = ModelView::new(model, members, boundary).unwrap();
    let mut transaction = Transaction::new("manual expression allocation");
    transaction
        .push(Op::DefineKernelNode {
            node: FieldDef::new(left, DimExponents::DIMENSIONLESS)
                .with_initial(DynQuantity::new(1.0, DimExponents::DIMENSIONLESS))
                .unwrap()
                .into(),
        })
        .push(Op::DefineKernelNode {
            node: FieldDef::new(right, DimExponents::DIMENSIONLESS)
                .with_initial(DynQuantity::new(2.0, DimExponents::DIMENSIONLESS))
                .unwrap()
                .into(),
        })
        .push(Op::DefineKernelNode {
            node: RelationDef::new(relation, expression).into(),
        })
        .push(Op::DefineKernelNode {
            node: ActivationDef::continuous(activation).into(),
        })
        .push(Op::DefineKernelNode {
            node: PortDef::signal(port, SignalDirection::Input, DimExponents::DIMENSIONLESS).into(),
        })
        .push(Op::Connect {
            from: relation.erase(),
            to: left.erase(),
            edge: EdgeKind::DependsOn,
        })
        .push(Op::Connect {
            from: relation.erase(),
            to: right.erase(),
            edge: EdgeKind::DependsOn,
        })
        .push(Op::Connect {
            from: activation.erase(),
            to: relation.erase(),
            edge: EdgeKind::Activates,
        })
        .push(Op::DefineOntologyView { view: view.into() });
    let mut store = InMemoryGraphStore::new();
    store.commit(transaction).unwrap();
    KernelProgram::from_snapshot(&store.snapshot(), model).unwrap()
}
