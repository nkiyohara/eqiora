use eqiora_compiler::compile;
use eqiora_graph::Op;
use eqiora_schema::kernel::{ExprNode, KernelNode};

#[test]
fn clocked_enum_state_uses_exact_declared_members_and_shared_selection() {
    let source = "enum Mode {Heating,Cooling,Fault} model M(){clock tick=periodic(1[s]);state mode:Mode at tick;initial{mode=Mode.Heating;}relation update at tick{next(mode)=case pre(mode){Mode.Heating=>Mode.Cooling,Mode.Cooling=>Mode.Fault,Mode.Fault=>Mode.Heating};}}";
    let models = compile("enum.eqi", source).unwrap_or_else(|e| panic!("{e:?}"));
    assert_eq!(
        models[0]
            .transaction()
            .ops()
            .iter()
            .filter(|op| matches!(
                op,
                Op::DefineKernelNode {
                    node: KernelNode::Enum(_)
                }
            ))
            .count(),
        1
    );
    assert!(models[0].transaction().ops().iter().any(|op|matches!(op,Op::DefineKernelNode{node:KernelNode::Relation(relation)} if relation.expression().nodes().iter().any(|node|matches!(node,ExprNode::Select{..})))));
}

#[test]
fn case_checks_every_exact_member_and_branch_type() {
    let source = "enum Mode {A,B} enum Other {A,B} model M(){parameter selected:Mode=Mode.A;variable y:integer;relation r{y=case selected{Mode.A=>9007199254740993,Mode.B=>2};}}";
    compile("case.eqi", source).unwrap_or_else(|errors| panic!("{errors:?}"));
    for invalid in [
        source.replace("Mode.B=>2", "Mode.A=>2"),
        source.replace(",Mode.B=>2", ""),
        source.replace("Mode.B=>2", "Other.B=>2"),
        source.replace("Mode.B=>2", "Mode.B=>2.5"),
        source.replace("selected:Mode=Mode.A", "selected:Mode=0"),
        source
            .replace(
                "variable y:integer",
                "parameter real_value:1=2;variable y:integer",
            )
            .replace("Mode.B=>2", "Mode.B=>real_value"),
    ] {
        assert!(compile("case.eqi", &invalid).is_err(), "{invalid}");
    }
}

#[test]
fn component_occurrences_share_enum_declaration_and_keep_parameter_dependencies() {
    let source = "enum Mode {A,B} component Cell(parameter selected:Mode){variable y:1;relation r{y=case selected{Mode.A=>1,Mode.B=>2};}} model M(){parameter selection:Mode=Mode.A;instance first:Cell(selected=selection);instance second:Cell(selected=Mode.B);}";
    let models = compile("occurrences.eqi", source).unwrap_or_else(|errors| panic!("{errors:?}"));
    let model = &models[0];
    let id = model.symbols().get("Mode").unwrap();
    let enums = model
        .transaction()
        .ops()
        .iter()
        .filter_map(|op| match op {
            Op::DefineKernelNode {
                node: KernelNode::Enum(value),
            } => Some(value),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(enums.len(), 1);
    assert_eq!(enums[0].id().erase(), id);
    let selection = model.symbols().get("selection").unwrap();
    assert!(model.transaction().ops().iter().any(|op|matches!(op, Op::DefineKernelNode{node:KernelNode::Relation(value)} if value.expression().nodes().iter().any(|node|matches!(node,ExprNode::Symbol(eqiora_schema::kernel::SymbolRef::Parameter(value)) if value.erase()==selection)))));
}

#[test]
fn selected_enum_binding_requires_exact_declaration_identity() {
    use eqiora_compiler::{CompiledModel, StaticBindingValue};
    let source = "enum Mode {A,B} model M(parameter choice:Mode=Mode.A){variable y:1;relation r{y=case choice{Mode.A=>1,Mode.B=>2};}}";
    let first = CompiledModel::compile_selected("binding.eqi", source, "M", &[]).unwrap();
    let definition = first
        .transaction()
        .ops()
        .iter()
        .find_map(|op| match op {
            Op::DefineKernelNode {
                node: KernelNode::Enum(value),
            } => Some(value),
            _ => None,
        })
        .unwrap();
    let value = definition.value(1).unwrap();
    let model = CompiledModel::compile_selected(
        "binding.eqi",
        source,
        "M",
        &[("choice", StaticBindingValue::Value(&value))],
    )
    .unwrap_or_else(|errors| panic!("{errors:?}"));
    assert!(model.transaction().ops().iter().any(|op|matches!(op,Op::DefineKernelNode{node:KernelNode::Parameter(parameter)} if parameter.value()==&value)));
    let foreign =
        eqiora_schema::kernel::EnumDef::new(eqiora_core::Id::new(), ["A".into(), "B".into()])
            .unwrap()
            .value(1)
            .unwrap();
    assert!(
        CompiledModel::compile_selected(
            "binding.eqi",
            source,
            "M",
            &[("choice", StaticBindingValue::Value(&foreign))]
        )
        .is_err()
    );
}

#[test]
fn native_enum_values_retain_registered_identity() {
    use eqiora_lang::{
        DraftDeclaration, DraftExpression, DraftParameter, DraftRelation, ModelDraft,
    };
    let definition =
        eqiora_schema::kernel::EnumDef::new(eqiora_core::Id::new(), ["A".into(), "B".into()])
            .unwrap();
    let value = definition.value(1).unwrap();
    let parameter = DraftParameter::new("choice", value.clone());
    let relation = DraftRelation::continuous(
        "same",
        [(
            parameter.expression(),
            DraftExpression::enum_value(value.clone()).unwrap(),
        )],
    );
    let draft = ModelDraft::new(
        "M",
        [
            DraftDeclaration::Enum {
                name: "Mode".into(),
                definition: definition.clone(),
            },
            parameter.into(),
            relation.into(),
        ],
    )
    .unwrap();
    let model = eqiora_compiler::lower_draft(&draft).unwrap_or_else(|errors| panic!("{errors:?}"));
    assert_eq!(model.symbols().get("Mode"), Some(definition.id().erase()));
    assert!(model.transaction().ops().iter().any(|op|matches!(op,Op::DefineKernelNode{node:KernelNode::Parameter(parameter)} if parameter.value()==&value)));
}

#[test]
fn imported_enum_aliases_share_identity_and_private_members_reject() {
    use eqiora_compiler::{
        CompilationNamespaceId, ResolvedHierarchyInput, ResolvedSourceUnit,
        analyze_resolved_hierarchy,
    };
    let owner = CompilationNamespaceId::new(["enum_test", "1", "fixture"]).unwrap();
    let source = "import enum_test.modes as left; import enum_test.modes as right; model M(){parameter choice:left.Mode=right.Mode.B;variable y:1;relation r{y=case choice{left.Mode.A=>1,right.Mode.B=>2};}}";
    let input = |declaration: &str| {
        ResolvedHierarchyInput::new(
            owner.clone(),
            vec![
                ResolvedSourceUnit::new(owner.clone(), "src/main.eqi", source).unwrap(),
                ResolvedSourceUnit::new(owner.clone(), "src/modes.eqi", declaration).unwrap(),
            ],
            vec![],
        )
    };
    let checked = analyze_resolved_hierarchy(input("public enum Mode {A,B}"))
        .unwrap()
        .validate_definitions()
        .unwrap();
    let model = checked
        .compile_root("M")
        .unwrap_or_else(|errors| panic!("{errors:?}"));
    assert_eq!(
        model.symbols().get("left.Mode"),
        model.symbols().get("right.Mode")
    );
    assert!(model.symbols().get("left.Mode").is_some());
    assert!(analyze_resolved_hierarchy(input("private enum Mode {A,B}")).is_err());
}
