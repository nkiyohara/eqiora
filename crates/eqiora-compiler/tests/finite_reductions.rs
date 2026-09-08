use eqiora_compiler::compile;
use eqiora_compiler::source_identity::LocalSourceIdentity;
use eqiora_graph::Op;
use eqiora_schema::kernel::{ExprNode, KernelNode, SymbolRef};

#[test]
fn finite_polynomial_and_dimensioned_product_lower_to_ordinary_equations() {
    let models=compile("fold.eqi","model M(){indexset I=range(3);relation polynomial{sum((ordinal(i)+1)*(ordinal(i)+1),over=(i in I))=14;}relation length{product(2[m],over=(i in I))=8[m^3];}}").unwrap();
    let relations = models[0]
        .transaction()
        .ops()
        .iter()
        .filter_map(|op| match op {
            Op::DefineKernelNode {
                node: KernelNode::Relation(value),
            } => Some(value),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(relations.len(), 2);
    assert!(relations.iter().any(|relation|relation.expression().nodes().iter().any(|node|matches!(node,ExprNode::Constant(value) if value.integer_scalar_value()==Some(14)))));
    assert!(relations.iter().all(|relation| {
        relation
            .expression()
            .nodes()
            .iter()
            .any(|node| matches!(node, ExprNode::Mul(..)))
    }));
}

#[test]
fn nested_fresh_binders_and_live_parameters_keep_dependencies() {
    let models=compile("nested.eqi","model M(){parameter gain:1=2;indexset I=range(3);indexset J=range(2);let total=sum(sum(gain*to_real(ordinal(i)+ordinal(j)),over=(j in J)),over=(i in I));relation r{total=18;}}").unwrap();
    assert!(models[0].transaction().ops().iter().any(|op|matches!(op,Op::DefineKernelNode{node:KernelNode::Relation(value)} if value.expression().nodes().iter().any(|node|matches!(node,ExprNode::Symbol(SymbolRef::Parameter(_)))))));
}

#[test]
fn reduction_names_alpha_normalize_without_erasing_operation_or_set() {
    let identity = |body: &str| {
        let source =
            format!("model M(){{indexset I=range(3);indexset J=range(3);relation r{{{body}=0;}}}}");
        let document = eqiora_lang::parse("identity.eqi", &source)
            .into_document()
            .unwrap();
        LocalSourceIdentity::from_document(&document).unwrap()
    };
    let first = identity("sum(to_real(ordinal(i)),over=(i in I))");
    assert_eq!(first, identity("sum(to_real(ordinal(k)),over=(k in I))"));
    assert_ne!(first, identity("sum(to_real(ordinal(i)),over=(i in J))"));
    assert_ne!(
        first,
        identity("product(to_real(ordinal(i)),over=(i in I))")
    );
}

#[test]
fn invalid_reduction_domains_capture_and_expansion_fail_locally() {
    for body in [
        "indexset I=range(0);relation r{sum(1,over=(i in I))=0;}",
        "indexset I=range(1000000000);relation r{sum(1,over=(i in I))=0;}",
        "indexset I=range(3);parameter i:integer=1;relation r{sum(ordinal(i),over=(i in I))=0;}",
        "indexset I=range(3);relation r{sum(sum(1,over=(i in I)),over=(i in I))=0;}",
        "indexset I=range(3);relation r{sum(true,over=(i in I))=true;}",
        "indexset I=range(3);relation r{sum([1,2],over=(i in I))=[3,6];}",
        "indexset I=range(3);parameter p:1=sum(1,over=(i in I));relation r{p=3;}",
    ] {
        let errors = compile("invalid.eqi", &format!("model M(){{{body}}}")).unwrap_err();
        assert!(
            errors.iter().all(|error| error.source_span().is_some()),
            "{body}: {errors:?}"
        );
    }
}

#[test]
fn selected_static_extent_is_bound_before_reduction_expansion() {
    let value = eqiora_lang::SourceAstFactory::expression(
        eqiora_lang::ExprKind::Number(eqiora_lang::DecimalLiteral::parse("3").unwrap()),
        Default::default(),
    )
    .unwrap();
    eqiora_compiler::CompiledModel::compile_selected("selected.eqi","model M(parameter n:integer){indexset I=range(n);relation r{sum(to_real(ordinal(i)),over=(i in I))=3;}}","M",&[("n",eqiora_compiler::StaticBindingValue::Expression(&value))]).unwrap();
}

#[test]
fn selected_extent_checks_product_units_and_unselected_definitions() {
    let extent = |n: &str| {
        eqiora_lang::SourceAstFactory::expression(
            eqiora_lang::ExprKind::Number(eqiora_lang::DecimalLiteral::parse(n).unwrap()),
            Default::default(),
        )
        .unwrap()
    };
    let source = "model M(parameter n:integer){indexset I=range(n);relation r{product(2[m],over=(i in I))=8[m^3];}}";
    let three = extent("3");
    eqiora_compiler::CompiledModel::compile_selected(
        "product.eqi",
        source,
        "M",
        &[("n", eqiora_compiler::StaticBindingValue::Expression(&three))],
    )
    .unwrap();
    for n in ["0", "2"] {
        let value = extent(n);
        assert!(
            eqiora_compiler::CompiledModel::compile_selected(
                "product.eqi",
                source,
                "M",
                &[("n", eqiora_compiler::StaticBindingValue::Expression(&value))]
            )
            .is_err()
        );
    }
    let invalid = format!("{source} model Unselected(){{relation wrong{{1[m]=1[s];}}}}");
    assert!(
        eqiora_compiler::CompiledModel::compile_selected(
            "product.eqi",
            &invalid,
            "M",
            &[("n", eqiora_compiler::StaticBindingValue::Expression(&three))]
        )
        .is_err()
    );
}
