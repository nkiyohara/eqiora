use super::*;
use eqiora_schema::kernel::ExprNode;

fn fixture(flux: &str) -> (KernelProgram, CompiledRegionForm, BTreeMap<String, RawId>) {
    let source = format!("model Stress() {{
        domain body = box(0, 1, 0, 1);
        domain left = boundary(body, axis=0, side=lower);
        domain right = boundary(body, axis=0, side=upper);
        parameter mu: 1 = 2;
        parameter lambda: 1 = 5;
        parameter p0: 1 = 3;
        parameter other0: 1 = 3;
        variable p: 1 on body;
        variable other: 1 on body;
        relation pressure on body {{ p = p0; }}
        relation other_pressure on body {{ other = other0; }}
        variable u: vector<m, 2> on body;
        relation balance on body {{
            -div(2*mu*symmetric_part(grad(u)) + lambda*isotropic_lift(div(u)) + isotropic_lift(p)) = 0;
        }}
        relation left_law on left {{ normal({flux}) = 0; }}
        relation right_law on right {{ normal({flux}) = 0; }}
    }}");
    let (transaction, model, symbols) =
        compile("flux.eqi", &source).unwrap().remove(0).into_parts();
    let mut store = InMemoryGraphStore::new();
    store.commit(transaction).unwrap();
    let program = KernelProgram::from_snapshot(&store.snapshot(), model).unwrap();
    let ids = ["body", "left", "right", "left_law", "right_law", "u"]
        .into_iter()
        .map(|name| (name.to_owned(), symbols.get(name).unwrap()))
        .collect::<BTreeMap<_, _>>();
    let form = CompiledRegionForm::derive(&program, ids["body"], 2).unwrap();
    (program, form, ids)
}

fn check(
    program: &KernelProgram,
    form: &CompiledRegionForm,
    ids: &BTreeMap<String, RawId>,
    side: &str,
) -> Result<(), Diagnostic> {
    let relation = ids[&format!("{side}_law")];
    let typed = typed_relation(program, relation).unwrap();
    let normal = typed.expression().roots()[0];
    assert!(matches!(
        typed.expression().node(normal),
        Some(ExprNode::NormalComponent(_))
    ));
    form.require_boundary_flux(program, ids[side], relation, ids["u"], normal)
}

#[test]
fn vector_stress_boundary_matches_complete_exact_operator_and_parameter_inventory() {
    let exact = "2*mu*symmetric_part(grad(u)) + lambda*isotropic_lift(div(u)) + isotropic_lift(p)";
    let (program, form, ids) = fixture(exact);
    check(&program, &form, &ids, "left").unwrap();
    check(&program, &form, &ids, "right").unwrap();
    // The uniform term is absent from the volume contraction, but remains a
    // mandatory member of the flux on both exact opposite parent boundaries.
    assert_eq!(form.rows[0].terms.len(), 2);
    assert_eq!(form.rows[0].flux.len(), 3);
    for changed in [
        "2*mu*symmetric_part(grad(u)) + lambda*isotropic_lift(div(u))",
        "2*mu*symmetric_part(grad(u)) + lambda*isotropic_lift(div(u)) + isotropic_lift(other)",
        "2*mu*grad(u) + lambda*isotropic_lift(div(u)) + isotropic_lift(p)",
    ] {
        let (program, form, ids) = fixture(changed);
        assert!(check(&program, &form, &ids, "left").is_err());
    }
    let typed = typed_relation(&program, ids["left_law"]).unwrap();
    assert!(
        form.require_boundary_flux(
            &program,
            ids["right"],
            ids["left_law"],
            ids["u"],
            typed.expression().roots()[0]
        )
        .is_err()
    );
}
