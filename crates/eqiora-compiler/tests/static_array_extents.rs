use eqiora_compiler::{CompiledModel, StaticBindingValue, compile};

#[test]
fn exact_parameter_extents_and_immutable_slices_share_array_types() {
    compile("arrays.eqi", "model M(){parameter n:integer=3; parameter values:array<integer,n>=[2,3,5]; relation sum{values[0:2]=[2,3];}}").unwrap();
}

#[test]
fn child_static_family_contexts_and_selected_component_are_concrete() {
    let source = "public component C(parameter n:integer){indexset I=range(n); variable values:array<1,n>; relation zero[i in I]{values[ordinal(i)]=0;}} model M(){instance a:C(n=2);instance b:C(n=3);}";
    compile("children.eqi", source).unwrap();
    let three = eqiora_lang::SourceAstFactory::expression(
        eqiora_lang::ExprKind::Number(eqiora_lang::DecimalLiteral::parse("3").unwrap()),
        Default::default(),
    )
    .unwrap();
    CompiledModel::compile_selected(
        "selected.eqi",
        source,
        "C",
        &[("n", StaticBindingValue::Expression(&three))],
    )
    .unwrap();
    let component_only = source.split(" model M()").next().unwrap();
    CompiledModel::compile_selected(
        "standalone.eqi",
        component_only,
        "C",
        &[("n", StaticBindingValue::Expression(&three))],
    )
    .unwrap();
}

#[test]
fn array_parameter_bindings_resolve_types_in_dependency_order_without_parent_capture() {
    for bindings in ["n=3,data=[2,3,5]", "data=[2,3,5],n=3"] {
        compile("bindings.eqi", &format!("component C(parameter data:array<integer,n>,parameter n:integer=2){{relation check{{data[1]=3;}}}} model M(){{parameter n:integer=17;instance child:C({bindings});}}" )).unwrap();
    }
    compile("default.eqi", "component C(parameter data:array<integer,n>,parameter n:integer=3){relation check{data[1]=3;}} model M(){instance child:C(data=[2,3,5]);}").unwrap();
    // The argument named n comes from the parent; data's type uses the child's n.
    compile("shadow.eqi", "component C(parameter data:array<integer,n>,parameter n:integer){relation check{data[1]=3;}} model M(){parameter n:integer=3;instance child:C(data=[2,3,5],n=n);}").unwrap();
}

#[test]
fn selected_array_parameters_use_defaults_and_binding_order_independently() {
    let source = "public component Sized(parameter data:array<integer,n>,parameter n:integer=3){relation r{data[1]=3;}}";
    let document = eqiora_lang::parse(
        "arguments.eqi",
        "model Args(){parameter n:integer=3;parameter data:array<integer,3>=[2,3,5];}",
    )
    .into_document()
    .unwrap();
    let values = document.models()[0]
        .items()
        .iter()
        .filter_map(|item| match item {
            eqiora_lang::Item::Parameter(value) => {
                Some((value.name(), StaticBindingValue::Expression(value.value())))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    CompiledModel::compile_selected("sized.eqi", source, "Sized", &values).unwrap();
    CompiledModel::compile_selected(
        "sized.eqi",
        source,
        "Sized",
        &values.iter().copied().rev().collect::<Vec<_>>(),
    )
    .unwrap();
    CompiledModel::compile_selected("sized.eqi", source, "Sized", &values[1..]).unwrap();
}

#[test]
fn public_signal_and_borrowed_field_interfaces_use_each_actual_size() {
    compile("signals.eqi", "component C(parameter n:integer, input x:array<1,n>, output y:array<1,n>){relation copy{y=x;}} model M(input a:array<1,2>,input b:array<1,3>){instance c:C(n=2,x=a);instance d:C(n=3,x=b);}").unwrap();
    compile("fields.eqi", "component C(parameter n:integer, variable x:array<1,n>){relation zero{x[0]=0;}} model M(){variable a:array<1,2>;variable b:array<1,3>;instance c:C(n=2,x=a);instance d:C(n=3,x=b);}").unwrap();
}

#[test]
fn nested_families_specialize_member_bindings_before_type_and_footprint_checks() {
    let source = "component Leaf(parameter n:integer){indexset I=range(n);variable values:array<1,n>;relation r[i in I]{values[ordinal(i)]=0;}} component Group(parameter n:integer){indexset Members=range(n);instance child[i in Members]:Leaf(n=ordinal(i)+1);} model M(){parameter n:integer=3;instance group:Group(n=n);}";
    let compiled = compile("nested.eqi", source).unwrap().remove(0);
    for ordinal in 0..3 {
        assert!(
            compiled
                .symbols()
                .get(&format!("group.child[{ordinal}].values"))
                .is_some()
        );
    }
    assert!(compiled.symbols().get("group.child[3].values").is_none());
}

#[test]
fn structural_extent_and_folded_selectors_retain_exact_live_parameter_guards() {
    use eqiora_core::{DimExponents, ScalarDomain, ValueLiteral, ValueType};
    use eqiora_graph::{GraphStore, InMemoryGraphStore, Op, Transaction};
    for source in [
        "model M(){parameter n:integer=2;variable x:array<1,n>;relation r{x=[0,0];}}",
        "model M(){parameter n:integer=2;parameter x:array<integer,3>=[2,3,5];relation r{x[n]=5;}}",
        "model M(){parameter n:integer=2;parameter x:array<integer,3>=[2,3,5];relation r{x[0:n]=[2,3];}}",
        "model M(){parameter n:integer=2;parameter x:array<integer,3>=[2,3,5];parameter prefix:array<integer,2>=x[0:n];relation r{prefix=[2,3];}}",
        "component C(parameter n:integer,variable x:array<1,n>){relation r{x=[0,0];}} model M(){parameter n:integer=2;variable x:array<1,2>;instance c:C(n=n,x=x);}",
        "component C(parameter n:integer,parameter x:array<integer,n>){relation r{x=[2,3];}} component Wrap(parameter n:integer){instance c:C(n=n,x=[2,3]);} model M(){parameter n:integer=2;instance c:Wrap(n=n);}",
        "model M(){parameter n:integer=2;parameter x:array<integer,3>=[2,3,5];relation r{(if true then to_integer(0) else x[n])=0;}}",
        "model M(){parameter n:integer=2;parameter x:array<integer,3>=[2,3,5];parameter prefix:array<integer,2>=if true then [to_integer(2),to_integer(3)] else x[0:n];relation r{prefix=[2,3];}}",
        "operator ignore(input x:1):1=0*x+1;model M(){parameter n:integer=2;parameter x:array<1,3>=[2,3,5];relation r{ignore(x=x[n])=1;}}",
    ] {
        let compiled = compile("guards.eqi", source)
            .unwrap_or_else(|errors| panic!("{source}: {errors:?}"))
            .remove(0);
        let id = compiled.symbols().get("n").unwrap();
        let mut store = InMemoryGraphStore::new();
        store.commit(compiled.into_parts().0).unwrap();
        let before = store.snapshot();
        let mut edit = Transaction::new("structural value cannot change after compilation");
        edit.push(Op::SetValue {
            target: id,
            value: ValueLiteral::from_integer(
                ValueType::scalar(ScalarDomain::Integer, DimExponents::DIMENSIONLESS).unwrap(),
                3,
            )
            .unwrap(),
        });
        assert!(store.commit(edit).is_err(), "missing guard for {source}");
        assert_eq!(store.snapshot().revision(), before.revision());
    }
    // A numerical coefficient is not a structural dependency merely because it is a Parameter.
    let compiled = compile(
        "numeric.eqi",
        "model M(){parameter gain:1=2;variable x:1;relation r{x=gain*3;}}",
    )
    .unwrap()
    .remove(0);
    let gain = compiled.symbols().get("gain").unwrap();
    let mut store = InMemoryGraphStore::new();
    store.commit(compiled.into_parts().0).unwrap();
    let mut edit = Transaction::new("ordinary coefficient remains editable");
    edit.push(Op::SetValue {
        target: gain,
        value: ValueLiteral::from_real(
            ValueType::scalar(ScalarDomain::Real, DimExponents::DIMENSIONLESS).unwrap(),
            4.0,
        )
        .unwrap(),
    });
    store.commit(edit).unwrap();
}

#[test]
fn type_extent_conditions_keep_lexical_nominal_resolution_and_canonical_source() {
    for extent in [
        "(if true then 2 else 3)",
        "(case Mode.A{Mode.A=>2,Mode.B=>3})",
    ] {
        let source = format!(
            "enum Mode{{A,B}} model M(){{variable x:array<1,{extent}>;relation r{{x=[0,0];}}}}"
        );
        compile("type-scope.eqi", &source).unwrap();
        let parsed = eqiora_lang::parse("type-scope.eqi", &source)
            .into_document()
            .unwrap();
        let formatted = eqiora_lang::format(&parsed);
        let reparsed = eqiora_lang::parse("type-scope.eqi", &formatted)
            .into_document()
            .unwrap();
        assert_eq!(eqiora_lang::format(&reparsed), formatted);
        compile("type-scope.eqi", &formatted).unwrap();
    }
}

#[test]
fn member_dependent_unit_slices_charge_actual_width_not_maximum_array_width() {
    let values = (0..16)
        .map(|value| value.to_string())
        .collect::<Vec<_>>()
        .join(",");
    let source = format!(
        "model M(){{parameter data:array<integer,16>=[{values}];indexset I=range(16);relation r{{sum(data[ordinal(i):ordinal(i)+1][0],over=(i in I))=120;}}}}"
    );
    // Independently, sum of ordinals 0 through 15 is 16*15/2=120.
    compile("unit-slices.eqi", &source).unwrap();
}

#[test]
fn extent_and_slice_denials_are_local_and_do_not_broadcast_or_resize() {
    let valid = "model M(){parameter n:integer=3;parameter values:array<integer,n>=[2,3,5];relation r{values[0:2]=[2,3];}}";
    compile("valid.eqi", valid).unwrap();
    for changed in [
        valid.replace("n:integer=3", "n:integer=0"),
        valid.replace("n:integer=3", "n:integer=-1"),
        valid.replace("n:integer=3", "n:integer=1.5"),
        valid.replace("n:integer=3", "n:integer=4294967296"),
        valid.replace("n:integer=3", "n:integer=65537"),
        valid.replace("array<integer,n>", "array<integer,time>"),
        valid.replace("[0:2]", "[0:4]"),
        valid.replace("[0:2]", "[2:2]"),
        valid.replace("[0:2]", "[2:1]"),
        valid.replace("[0:2]", "[-1:2]"),
        valid.replace("[0:2]", "[0:2:1]"),
        valid.replace("[0:2]", "[:2]"),
        valid.replace("[0:2]", "[0:]"),
        valid.replace("[0:2]", "[false:2]"),
        valid.replace("[0:2]", "[0:time]"),
        valid.replace("[0:2]", "[0:2[s]]"),
        valid.replace("=[2,3,5]", "=2"),
    ] {
        let errors = compile("invalid-array.eqi", &changed).unwrap_err();
        assert!(
            errors.iter().any(|error| error.source_span().is_some()),
            "{changed}: {errors:?}"
        );
    }
}
