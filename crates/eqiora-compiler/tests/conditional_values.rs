use eqiora_compiler::compile;
use eqiora_graph::Op;
use eqiora_schema::kernel::{ExprNode, KernelNode};

#[test]
fn source_selection_retains_both_arms_and_live_condition() {
    let models = compile("select.eqi", "model M(){parameter choose:bool=false;parameter x:V^2=-4;relation r{(if choose then math.sqrt(x) else 0[V])=0[V];}}").unwrap_or_else(|errors|panic!("{errors:?}"));
    let relation = models[0]
        .transaction()
        .ops()
        .iter()
        .find_map(|op| match op {
            Op::DefineKernelNode {
                node: KernelNode::Relation(value),
            } => Some(value),
            _ => None,
        })
        .unwrap();
    assert!(
        relation
            .expression()
            .nodes()
            .iter()
            .any(|node| matches!(node, ExprNode::Select { .. }))
    );
    assert!(
        relation
            .expression()
            .nodes()
            .iter()
            .any(|node| matches!(node, ExprNode::UnaryMath { .. }))
    );
}

#[test]
fn guarded_operator_and_dimensioned_sugar_use_one_checked_calculus() {
    for body in [
        "if x >= 0[V^2] then math.sqrt(x) else 0[V]",
        "math.sqrt(math.abs(x))",
        "math.sqrt(math.max(x, 0[V^2]))",
        "math.sqrt(math.clamp(x, 0[V^2], 4[V^2]))",
    ] {
        let source = format!(
            "operator guard(input x:V^2):V={body};model M(){{parameter x:V^2=-4;relation r{{guard(x=x)=0[V];}}}}"
        );
        compile("guard.eqi", &source).unwrap_or_else(|errors| panic!("{body}: {errors:?}"));
    }
}

#[test]
fn ordinary_nonsmooth_builtins_lower_to_select_and_clamp_requirement() {
    for expression in [
        "math.abs(x)",
        "math.min(x,2[V])",
        "math.max(x,2[V])",
        "math.clamp(x,0[V],2[V])",
    ] {
        let source = format!("model M(){{parameter x:V=-1;relation r{{{expression}=1[V];}}}}");
        compile("sugar.eqi", &source).unwrap_or_else(|errors| panic!("{expression}: {errors:?}"));
    }
    for expression in ["math.sign(x)", "math.step(x)"] {
        compile(
            "sign.eqi",
            &format!("model M(){{parameter x:V=-1;relation r{{{expression}=0;}}}}"),
        )
        .unwrap_or_else(|errors| panic!("{errors:?}"));
    }
}

#[test]
fn inactive_branches_still_require_valid_types_and_bindings() {
    for expression in [
        "if false then missing else 1",
        "if true then 1[m] else 1[s]",
        "if 1 then 1 else 2",
        "if false then math.sqrt(true) else 1",
        "math.min(1[m],2[s])",
        "math.sign(true)",
    ] {
        assert!(
            compile(
                "invalid-select.eqi",
                &format!("model M(){{relation r{{({expression})=1;}}}}")
            )
            .is_err(),
            "{expression}"
        );
    }
    for declaration in [
        "operator guard(input x:scalar):scalar=if x>=0 then x else -x;",
        "operator guard(input x:V^2):V=if x>=0[V^2] then math.sqrt(x) else 0[s];",
    ] {
        assert!(
            compile(
                "invalid-guard.eqi",
                &format!("{declaration} model M(){{relation r{{1=1;}}}}")
            )
            .is_err(),
            "{declaration}"
        );
    }
}
