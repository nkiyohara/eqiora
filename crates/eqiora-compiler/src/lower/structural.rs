//! Folded structural inputs remain exact graph dependencies, separate from ExprDag inputs.
use super::*;

pub(super) fn connect_relation(
    file: &str,
    range: TextRange,
    relation: eqiora_core::RawId,
    equations: &[LoweringEquation],
    bindings: &BTreeMap<String, Binding>,
    edges: &mut Vec<(eqiora_core::RawId, eqiora_core::RawId, EdgeKind)>,
) -> Result<(), Diagnostic> {
    let structural = equations
        .iter()
        .flat_map(|equation| [&equation.left, &equation.right])
        .flat_map(LoweringExpression::structural_parameters)
        .collect::<BTreeSet<_>>();
    for name in structural {
        let Some(Binding::Parameter(parameter, _)) = bindings.get(&name) else {
            return Err(unresolved(file, range, &name, "structural Parameter"));
        };
        edges.push((relation, parameter.erase(), EdgeKind::StructurallyDependsOn));
    }
    Ok(())
}

pub(super) fn connect_declarations(
    file: &str,
    model: &LoweringModel,
    bindings: &BTreeMap<String, Binding>,
    edges: &mut Vec<(eqiora_core::RawId, eqiora_core::RawId, EdgeKind)>,
) -> Result<(), Vec<Diagnostic>> {
    if model.structural_dependencies.is_empty() {
        return Ok(());
    }
    let nominals = model
        .items
        .iter()
        .filter_map(|item| match item {
            LoweringItem::Nominal { name, definition } => Some((name.as_str(), definition.id())),
            _ => None,
        })
        .collect::<BTreeMap<_, _>>();
    for (owner, dependencies) in &model.structural_dependencies {
        let owner = bindings
            .get(owner)
            .map(Binding::primary_id)
            .or_else(|| nominals.get(owner.as_str()).copied())
            .ok_or_else(|| {
                vec![unresolved(
                    file,
                    model.range,
                    owner,
                    "structural declaration",
                )]
            })?;
        for name in dependencies {
            let Some(Binding::Parameter(parameter, _)) = bindings.get(name) else {
                return Err(vec![unresolved(
                    file,
                    model.range,
                    name,
                    "structural Parameter",
                )]);
            };
            edges.push((owner, parameter.erase(), EdgeKind::StructurallyDependsOn));
        }
    }
    Ok(())
}
