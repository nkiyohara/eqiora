//! Selected authoring arguments enter the same dependency-ordered Parameter owner.
use super::*;
use crate::StaticBindingValue;
use eqiora_lang::SignatureItem;

pub(in crate::hierarchy) fn resolve_selected_parameters(
    file: &str,
    signature: &[SignatureItem],
    authored: &[(&str, StaticBindingValue<'_>)],
    frames: BTreeMap<String, SpatialSupport<String>>,
) -> Result<Vec<super::super::ExternalParameterBinding>, Vec<Diagnostic>> {
    let declarations = signature
        .iter()
        .filter_map(|item| match item {
            SignatureItem::Parameter(parameter) => Some((parameter.name().to_owned(), parameter)),
            _ => None,
        })
        .collect::<BTreeMap<_, _>>();
    let mut expressions = BTreeMap::new();
    let mut bound_values = BTreeMap::new();
    let mut clocks = BTreeMap::new();
    for &(name, value) in authored {
        match value {
            StaticBindingValue::Expression(value) if declarations.contains_key(name) => {
                expressions.insert(name.to_owned(), value.clone());
            }
            StaticBindingValue::Value(value) if declarations.contains_key(name) => {
                bound_values.insert(
                    name.to_owned(),
                    SymbolicParameterValue {
                        value: Some(value.clone()),
                        value_type: value.value_type().clone(),
                        expression: Some(LoweringExpression::literal(
                            value.clone(),
                            declarations[name].range(),
                        )),
                        lineage: Some(ParameterLineage::Constant),
                    },
                );
            }
            StaticBindingValue::Clock(clock) => {
                if let eqiora_schema::kernel::ClockKind::Periodic { period, .. } = clock.kind() {
                    clocks.insert(name.to_owned(), Some(period));
                }
            }
            _ => {}
        }
    }
    let resolver = SymbolicParameterResolver {
        declaration_file: file,
        declarations,
        bindings: Some(bindings::Bindings::closed(
            file,
            expressions,
            clocks.clone(),
            frames.clone(),
        )),
        bound_values,
        resolved: BTreeMap::new(),
        required_policy: RequiredParameterPolicy::RejectUnbound,
        frames,
    };
    let resolved = resolver.resolve_all(&mut |name| clocks.get(name).copied())?;
    authored
        .iter()
        .filter_map(|(name, _)| resolved.get(*name).map(|value| (*name, value)))
        .map(|(name, value)| {
            value
                .value
                .clone()
                .map(|value| super::super::ExternalParameterBinding::new(name, value))
                .ok_or_else(|| {
                    vec![hierarchy_error(format!(
                        "selected Parameter `{name}` remained symbolic"
                    ))]
                })
        })
        .collect()
}
