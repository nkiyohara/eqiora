use eqiora_compiler::{CompiledModel, QuantityRole, compile};
use eqiora_lang::NotationProfile;
use std::collections::{BTreeMap, BTreeSet};

fn model(source: &str) -> CompiledModel {
    compile("labels.eqi", source)
        .unwrap_or_else(|errors| panic!("{errors:?}\n{source}"))
        .remove(0)
}
fn labels(model: &CompiledModel, profile: NotationProfile) -> BTreeMap<String, String> {
    model
        .notation()
        .iter()
        .map(|entry| (entry.selector().to_owned(), entry.render(profile)))
        .collect()
}
fn unique(model: &CompiledModel) {
    for profile in [
        NotationProfile::Rich,
        NotationProfile::Plain,
        NotationProfile::Speech,
    ] {
        let labels = labels(model, profile);
        assert_eq!(labels.len(), model.notation().iter().len());
        assert_eq!(
            labels.values().collect::<BTreeSet<_>>().len(),
            labels.len(),
            "{labels:?}"
        );
    }
}

const RESISTORS: &str = r"connector Pin { across voltage: V; through current: A; }
component Resistor(parameter resistance @{R}: V/A, port p @{v}:Pin, port n @{v}:Pin) {
    relation law { p.voltage-n.voltage = resistance*p.current; p.current+n.current=0; }
}
model Circuit() { instance left:Resistor(resistance=2 [V/A]); instance right:Resistor(resistance=3 [V/A]); connect left.p,right.p; connect left.n,right.n; }";

#[test]
fn resistors_roles_source_trace_and_full_scope_views_share_one_resolver() {
    let compiled = model(RESISTORS);
    unique(&compiled);
    let rich = labels(&compiled, NotationProfile::Rich);
    assert_eq!(rich["left.resistance"], "R_{l e f t}");
    assert_eq!(rich["right.resistance"], "R_{r i g h t}");
    assert_eq!(compiled.notation().iter().len(), 10);
    let p = compiled
        .notation()
        .iter()
        .find(|entry| entry.selector() == "left.p.voltage")
        .unwrap();
    let i = compiled
        .notation()
        .iter()
        .find(|entry| entry.selector() == "left.p.current")
        .unwrap();
    assert_eq!(p.identity().occurrence(), i.identity().occurrence());
    assert_ne!(p.identity(), i.identity());
    assert_eq!(p.identity().role(), QuantityRole::Across);
    assert_eq!(p.graph_id(), i.graph_id());
    assert!(p.definition_span().is_some());
    assert!(p.instance_span().is_some());
    let view = compiled
        .notation()
        .view([i.identity(), p.identity(), p.identity()]);
    assert_eq!(view.len(), 2);
    for entry in view {
        assert_eq!(entry.render(NotationProfile::Rich), rich[entry.selector()]);
    }
    let reordered = model(&RESISTORS.replace(
        "instance left:Resistor(resistance=2 [V/A]); instance right:Resistor(resistance=3 [V/A]);",
        "instance right:Resistor(resistance=3 [V/A]); instance left:Resistor(resistance=2 [V/A]);",
    ));
    assert_eq!(rich, labels(&reordered, NotationProfile::Rich));
}

#[test]
fn existing_scripts_styles_glyphs_and_same_scope_are_disambiguated() {
    let compiled = model(
        r"model M() {
       parameter a @{\sigma_{ij}}:1=1; parameter b @{\sigma_{ij}}:1=2;
       parameter c @{x}:1=3; parameter d @{\mathbf{x}}:1=4;
       parameter e @{A}:1=5; parameter f @{\Alpha}:1=6;
       variable y:1; relation law {y=a+b+c+d+e+f;}
    }",
    );
    unique(&compiled);
    let rich = labels(&compiled, NotationProfile::Rich);
    assert_eq!(rich["a"], r"\sigma_{i j a v a l u e}");
    assert_eq!(rich["b"], r"\sigma_{i j b v a l u e}");
    assert!(!rich["c"].ends_with('x'));
    assert_eq!(rich["a"].matches("_{").count(), 1);
    assert_eq!(rich["b"].matches("_{").count(), 1);
}

#[test]
fn explicit_qualifiers_and_derived_suffixes_cannot_capture_each_other() {
    let source = r"component Cell(parameter value @{x}:1) {variable y:1; relation law {y=value;}} model M() {
       instance first @{r}:Cell(value=1); instance second @{r}:Cell(value=2);
       instance r:Cell(value=3); instance other:Cell(value=4);
    }";
    let compiled = model(source);
    unique(&compiled);
    let labels = labels(&compiled, NotationProfile::Rich);
    assert!(labels["first.value"].starts_with("x_{r "));
    assert!(labels["second.value"].starts_with("x_{r "));
}

#[test]
fn fluid_and_solid_properties_use_occurrences_not_contract_names() {
    let source = r"property contract Density():1 {derivatives value_only;}
    property release Measured implements Density {value=2;source_unit:1=1;validity=unconditional;citation=org.example.measurement;license=spdx.CC0_1_0;}
    component Material(property density @{\rho}:Density, output y:1) {relation law {y=density;}}
    model M() {instance fluid:Material(density=Measured);instance solid:Material(density=Measured);}";
    let compiled = model(source);
    unique(&compiled);
    let labels = labels(&compiled, NotationProfile::Rich);
    assert_eq!(labels["fluid.density"], r"\rho_{f l u i d}");
    assert_eq!(labels["solid.density"], r"\rho_{s o l i d}");
}

#[test]
fn bounded_generated_exhaustion_never_rejects_a_valid_physical_model() {
    let long = "a".repeat(200);
    let source = format!(
        "component Cell(parameter value @{{x}}:1) {{variable y:1; relation law {{y=value;}}}} model M() {{instance {long}:Cell(value=1);instance b:Cell(value=2);}}"
    );
    let compiled = model(&source);
    unique(&compiled);
    assert!(
        labels(&compiled, NotationProfile::Rich)
            .values()
            .all(|value| value.len() < 16384)
    );
}

#[test]
fn eliminated_public_exposures_keep_distinct_labels_and_source_origins() {
    let compiled = model(
        r"connector Pin {across potential:1;through flow:1;}
    component Leaf(port p @{v}:Pin) {relation owner {p.potential=0;}}
    component Wrapper(port p @{v}:Pin) {instance leaf:Leaf();connect p,leaf.p;}
    model Network() {instance left:Wrapper();instance right:Leaf();connect left.p,right.p;}",
    );
    unique(&compiled);
    let projection = compiled.physical_exposures().get("left.p").unwrap();
    let entries = compiled
        .notation()
        .iter()
        .filter(|entry| entry.identity().occurrence() == projection.exposure())
        .collect::<Vec<_>>();
    assert_eq!(entries.len(), 2);
    assert!(entries.iter().all(|entry| entry.graph_id().is_none()
        && entry.definition_span().is_some()
        && entry.instance_span().is_some()));
    assert!(
        compiled
            .notation()
            .iter()
            .any(|entry| entry.selector() == "left.leaf.p.potential" && entry.graph_id().is_some())
    );
}

#[test]
fn boundary_family_members_preserve_exact_selectors_and_roles() {
    let compiled = model(
        r"connector BoundaryScalar {trace value:1;flux flux:1;shape [];frame invariant;pairing euclidean_boundary_duality;orientation parent_outward;}
    component Wall(support body:volume(ambient_dimension=1),support exterior:complete_exterior(parent=body),port p @{u_{i}}[side in exterior]:BoundaryScalar over side) {
       relation law[side in exterior] on side {p[side=side].flux=0;}
    }
    component Terminal(support body:volume(ambient_dimension=1),support face:boundary(parent=body),port p:BoundaryScalar over face) {relation law on face {p.value=0;}}
    model M() {domain body=box(0,1);domain lo=boundary(body,axis=0,side=lower);domain hi=boundary(body,axis=0,side=upper);instance wall:Wall(body=body,exterior=boundaries(lo,hi));instance lower:Terminal(body=body,face=lo);instance upper:Terminal(body=body,face=hi);connect wall.p[side=lo],lower.p;connect wall.p[side=hi],upper.p;}",
    );
    unique(&compiled);
    let entries = compiled
        .notation()
        .iter()
        .filter(|entry| entry.identity().member().is_some())
        .collect::<Vec<_>>();
    assert_eq!(entries.len(), 4);
    assert_eq!(
        entries
            .iter()
            .map(|entry| entry.identity().member().unwrap())
            .collect::<BTreeSet<_>>()
            .len(),
        2
    );
    assert!(entries.iter().all(|entry| matches!(
        entry.identity().role(),
        QuantityRole::Trace | QuantityRole::Flux
    )));
    for entry in entries {
        assert_eq!(entry.render(NotationProfile::Rich).matches("_{").count(), 1);
    }
}

#[test]
fn forwarded_parameter_has_declaration_identity_and_shared_graph_target() {
    let compiled = model(
        r"component C(parameter p @{P}:1) {variable y:1;relation law {y=p;}}
    model M() {parameter value @{P}:1=3;instance a:C(p=value);instance b:C(p=value);}",
    );
    unique(&compiled);
    let by_name = compiled
        .notation()
        .iter()
        .map(|entry| (entry.selector(), entry))
        .collect::<BTreeMap<_, _>>();
    assert_ne!(by_name["a.p"].identity(), by_name["b.p"].identity());
    assert_eq!(by_name["a.p"].graph_id(), by_name["value"].graph_id());
    assert_eq!(by_name["b.p"].graph_id(), by_name["value"].graph_id());
    assert!(by_name["value"].graph_id().is_some());
}

#[test]
fn borrowed_fields_keep_occurrence_notation_while_sharing_the_exact_target() {
    let source = r"component Reader(state value @{v}:1) {relation law {value=0;}}
    model M() {state x @{v}:1;instance left:Reader(value=x);instance right:Reader(value=x);}";
    let compiled = model(source);
    unique(&compiled);
    let entries = compiled
        .notation()
        .iter()
        .map(|entry| (entry.selector(), entry))
        .collect::<BTreeMap<_, _>>();
    assert_eq!(entries.len(), 3);
    let target = entries["x"].graph_id().unwrap();
    assert_ne!(
        entries["left.value"].identity(),
        entries["right.value"].identity()
    );
    for selector in ["left.value", "right.value"] {
        let entry = entries[selector];
        assert_eq!(entry.graph_id(), Some(target));
        let definition = entry.definition_span().unwrap();
        assert!(source[definition.start as usize..definition.end as usize].contains("state value"));
        let instance = entry.instance_span().unwrap();
        assert!(source[instance.start as usize..instance.end as usize].contains("instance"));
    }
}

#[test]
fn deep_resolved_declarations_keep_physical_admission_and_exact_labels() {
    use eqiora_compiler::{
        CompilationNamespaceId, ResolvedDependency, ResolvedHierarchyInput, ResolvedSourceUnit,
        analyze_resolved_hierarchy,
    };
    let root = CompilationNamespaceId::new(["app"]).unwrap();
    let owner = CompilationNamespaceId::new(["vendor", "exact-revision"]).unwrap();
    // Current resolved admission uses 2 namespace markers + 2 owner segments
    // + 28 module segments: the exact existing 32-segment maximum.
    let modules = (0..28).map(|i| format!("m{i}")).collect::<Vec<_>>();
    let file = format!("src/{}.eqi", modules.join("/"));
    let long_name = "v".repeat(1024); // Admitted exact public identifier boundary.
    let declarations = [
        format!("state {long_name} @{{v}}:1"),
        "parameter constant @{v}:1".to_owned(),
        "parameter forwarded @{v}:1".to_owned(),
    ];
    let root_source = format!("import vendor.{} as lib;
        component Wrap(state x:1, parameter p:1) {{instance inner:lib.Reader({long_name}=x,constant=2,forwarded=p);}}
        model M() {{state x @{{v}}:1;parameter p:1=3;instance left:Wrap(x=x,p=p);instance right:Wrap(x=x,p=p);relation law {{x=0;}}}}", modules.join("."));
    let compile = |reverse: bool, annotated: bool| {
        let mut fields = declarations.clone();
        if reverse {
            fields.reverse();
        }
        let library = format!("public component Reader({}) {{}}", fields.join(","));
        let library = if annotated {
            library
        } else {
            library.replace(" @{v}", "")
        };
        let root_source = if annotated {
            root_source.clone()
        } else {
            root_source.replace(" @{v}", "")
        };
        let mut units = vec![
            ResolvedSourceUnit::new(root.clone(), "src/main.eqi", root_source).unwrap(),
            ResolvedSourceUnit::new(owner.clone(), &file, library).unwrap(),
        ];
        if reverse {
            units.reverse();
        }
        analyze_resolved_hierarchy(ResolvedHierarchyInput::new(
            root.clone(),
            units,
            vec![ResolvedDependency::new(root.clone(), owner.clone())],
        ))
        .unwrap()
        .validate_definitions()
        .unwrap()
        .compile_root("M")
        .unwrap()
    };
    let compiled = compile(false, true);
    unique(&compiled);
    let unannotated = compile(false, false);
    assert_eq!(compiled.model(), unannotated.model());
    assert_eq!(
        compiled.transaction().ops(),
        unannotated.transaction().ops()
    );
    let reordered = compile(true, true);
    assert_eq!(
        labels(&compiled, NotationProfile::Rich),
        labels(&reordered, NotationProfile::Rich)
    );
    let by_name = compiled
        .notation()
        .iter()
        .map(|entry| (entry.selector(), entry))
        .collect::<BTreeMap<_, _>>();
    let reordered_keys = reordered
        .notation()
        .iter()
        .map(|entry| (entry.selector(), entry.identity()))
        .collect::<BTreeMap<_, _>>();
    for side in ["left", "right"] {
        for name in [&long_name, "constant", "forwarded"] {
            let selector = format!("{side}.inner.{name}");
            let entry = by_name[selector.as_str()];
            assert_eq!(entry.identity(), reordered_keys[selector.as_str()]);
            assert!(entry.definition_span().unwrap().file.ends_with(&file));
            assert!(
                entry
                    .instance_span()
                    .unwrap()
                    .file
                    .ends_with("src/main.eqi")
            );
        }
        assert_eq!(
            by_name[format!("{side}.inner.{long_name}").as_str()].graph_id(),
            by_name["x"].graph_id()
        );
        assert_eq!(
            by_name[format!("{side}.inner.forwarded").as_str()].graph_id(),
            by_name["p"].graph_id()
        );
        assert!(
            by_name[format!("{side}.inner.constant").as_str()]
                .graph_id()
                .is_none()
        );
    }
    let too_deep = ResolvedHierarchyInput::new(
        root.clone(),
        vec![
            ResolvedSourceUnit::new(
                root.clone(),
                "src/main.eqi",
                "model M(){variable x:1;relation law{x=0;}}",
            )
            .unwrap(),
            ResolvedSourceUnit::new(
                owner.clone(),
                format!("src/extra/{}", &file[4..]),
                "public component C(state x:1){}",
            )
            .unwrap(),
        ],
        vec![ResolvedDependency::new(root, owner)],
    );
    assert!(
        analyze_resolved_hierarchy(too_deep)
            .unwrap_err()
            .iter()
            .any(|error| error
                .message()
                .contains("identity namespace exceeds the 32 segment limit"))
    );
    let name = "v".repeat(1025);
    let rejected = format!(
        "component C(state {name} @{{v}}:1){{}}model M(){{state x:1;instance r:C({name}=x);relation law{{x=0;}}}}"
    );
    assert!(
        eqiora_compiler::compile("too-long.eqi", &rejected)
            .unwrap_err()
            .iter()
            .any(|error| error
                .message()
                .contains("requires 1025 bytes, exceeding the 1024 byte limit"))
    );
}
