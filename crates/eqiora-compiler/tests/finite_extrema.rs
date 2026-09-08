use eqiora_compiler::compile;
use eqiora_graph::Op;
use eqiora_schema::kernel::{ComparisonOp, ExprNode, KernelNode, SymbolRef};

#[test]
fn extrema_keep_exact_integer_operands_and_live_parameter_dependencies() {
    let models=compile("extrema.eqi", "model M(){parameter p:integer=9007199254740993;indexset I=range(3);relation r{min([p+1,p-2,p+1][ordinal(i)],over=(i in I))=p-2;max([p+1,p-2,p+1][ordinal(i)],over=(i in I))=p+1;}}").unwrap_or_else(|errors|panic!("{errors:?}"));
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
    let nodes = relation.expression().nodes();
    assert_eq!(
        nodes
            .iter()
            .filter(|node| matches!(node, ExprNode::Compare(ComparisonOp::LessEqual, ..)))
            .count(),
        2
    );
    assert_eq!(
        nodes
            .iter()
            .filter(|node| matches!(node, ExprNode::Compare(ComparisonOp::GreaterEqual, ..)))
            .count(),
        2
    );
    assert!(
        nodes
            .iter()
            .any(|node| matches!(node, ExprNode::Symbol(SymbolRef::Parameter(_))))
    );
    assert!(models[0].transaction().ops().iter().any(|op|matches!(op,Op::DefineKernelNode{node:KernelNode::Parameter(value)} if value.value().integer_scalar_value()==Some(9_007_199_254_740_993))));
}

#[test]
fn dimensioned_and_nested_extrema_preserve_scalar_types() {
    for expression in [
        "min([-2[m],3[m],3[m]][ordinal(i)],over=(i in I))",
        "max(min(to_real(ordinal(i)+ordinal(j))*1[m],over=(j in I)),over=(i in I))",
    ] {
        let source = format!("model M(){{indexset I=range(3);relation r{{{expression}=0[m];}}}}");
        compile("dimensioned-extrema.eqi", &source).unwrap_or_else(|errors| panic!("{errors:?}"));
    }
}

#[test]
fn invalid_singletons_capture_and_empty_extrema_fail_locally() {
    for body in [
        "indexset I=range(1);relation r{min(true,over=(i in I))=true;}",
        "indexset I=range(1);relation r{max(math.complex(1,2),over=(i in I))=math.complex(1,2);}",
        "indexset I=range(1);relation r{min([1,2],over=(i in I))=[1,2];}",
        "indexset I=range(1);relation r{min(index(I,0),over=(i in I))=index(I,0);}",
        "indexset I=range(0);relation r{min(1,over=(i in I))=1;}",
        "indexset I=range(2);relation r{max(min(ordinal(i),over=(i in I)),over=(i in I))=1;}",
        "indexset I=range(2);parameter p:1=min(1,over=(i in I));relation r{p=1;}",
    ] {
        let errors = compile("invalid-extrema.eqi", &format!("model M(){{{body}}}")).unwrap_err();
        assert!(
            errors.iter().all(|error| error.source_span().is_some()),
            "{errors:?}"
        );
    }
}
