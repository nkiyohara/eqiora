use super::*;
use eqiora_meshing::QuadratureRule;

#[test]
fn scalar_q1_uses_the_same_value_and_gradient_contractions() {
    let source = "model Scalar {
        domain body = box(0, 2, 0, 3);
        domain left = boundary(body, axis = 0, side = lower);
        domain right = boundary(body, axis = 0, side = upper);
        domain bottom = boundary(body, axis = 1, side = lower);
        domain top = boundary(body, axis = 1, side = upper);
        representation space = continuum;
        parameter reaction: 1 / m ^ 2 = 3;
        parameter load: 1 / m ^ 2 = 5;
        field u on body as space: 1;
        relation balance continuous on body { -div(2 * grad(u)) + reaction * u - load = 0; }
        relation bc0 continuous on left { trace(u) = 0; }
        relation bc1 continuous on right { trace(u) = 0; }
        relation bc2 continuous on bottom { trace(u) = 0; }
        relation bc3 continuous on top { trace(u) = 0; }
    }";
    let (transaction, model, _) = compile("shared-q1.eqi", source)
        .unwrap()
        .remove(0)
        .into_parts();
    let mut store = InMemoryGraphStore::new();
    store.commit(transaction).unwrap();
    let program = KernelProgram::from_snapshot(&store.snapshot(), model).unwrap();
    let domain = program
        .nodes()
        .find_map(|node| match node {
            KernelNode::Domain(domain)
                if matches!(
                    domain.kind(),
                    eqiora_schema::kernel::DomainKind::CartesianBox { .. }
                ) =>
            {
                Some(domain.id().erase())
            }
            _ => None,
        })
        .unwrap();
    let form = CompiledRegionForm::derive(&program, domain, 2).unwrap();
    let reference = ReferenceCell::hypercube(2).unwrap();
    let fields = form
        .fields()
        .map(|(field, value_type)| RegionFieldBinding {
            field,
            space: p1(),
            scale: DynQuantity::new(1.0, value_type.dimension()),
        })
        .collect::<Vec<_>>();
    let rows = form
        .rows()
        .map(|(relation, _, _)| (relation, DynQuantity::new(1.0, DimExponents::DIMENSIONLESS)))
        .collect();
    let bound = form.bind(reference, &fields, &rows, None).unwrap();
    let geometry =
        AffineGeometryMap::new(reference, 2, vec![1.0, 1.5], vec![1.0, 0.0, 0.0, 1.5]).unwrap();
    let quadrature = QuadratureRule::tensor_product_gauss_legendre(2, 2).unwrap();
    let local = bound
        .evaluate(&geometry, &quadrature, &BTreeMap::new())
        .unwrap();
    let scalar = crate::form_compiler::linear::CompiledLinearBlockForm::derive(&program, domain, 2)
        .unwrap()
        .evaluate(&geometry, &quadrature)
        .unwrap();
    for (a, b) in local.matrix().iter().zip(scalar.matrix()) {
        close(*a, *b);
    }
    for (a, b) in local.rhs().iter().zip(scalar.rhs()) {
        close(*a, *b);
    }
    // Constant forcing integrates to load * area / 4 at each Q1 vertex.
    for value in local.rhs() {
        close(*value, 5.0 * 6.0 / 4.0);
    }
}
