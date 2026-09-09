//! Retain independently typed nominal members through the ordinary expression lowerer.
use super::*;

pub(in crate::lower) fn lower_record(
    file: &str,
    members: &[LoweringExpression],
    bindings: &BTreeMap<String, Binding>,
) -> Result<ExprDag, Diagnostic> {
    let mut lowerer = ExpressionLowerer {
        file,
        bindings,
        builder: ExprDagBuilder::new(),
        dependencies: BTreeSet::new(),
        ports: BTreeSet::new(),
        cache: HashMap::new(),
        sampling: false,
        allow_discrete_symbols: true,
        activation: &ActivationSyntax::Continuous,
        initial: true,
    };
    let roots = members
        .iter()
        .map(|member| {
            expression_type(file, member, bindings, None)?;
            lowerer.lower(member).map(|value| value.id)
        })
        .collect::<Result<Vec<_>, Diagnostic>>()?;
    lowerer.builder.finish(roots)
}
