use super::*;

impl LoweringModel {
    pub(super) fn from_source(file: &str, model: &ModelDecl) -> Result<Self, Diagnostic> {
        let items = model
            .items()
            .iter()
            .enumerate()
            .map(|(item_index, item)| {
                Ok(match item {
                    Item::Domain(declaration) => LoweringItem::Domain {
                        name: declaration.name().to_owned(),
                        contract: LoweringDomainContract::Source(declaration.syntax().clone()),
                        range: declaration.range(),
                    },
                    Item::Field(declaration) => LoweringItem::Field {
                        name: declaration.name().to_owned(),
                        domain: declaration.domain().map(str::to_owned),
                        representation: declaration
                            .domain()
                            .map(|domain| format!("$continuum-{domain}")),
                        value_type: declaration.value_type().clone(),
                        role: declaration.role(),
                        activation: declaration.activation().clone(),
                        range: declaration.range(),
                    },
                    Item::Parameter(declaration) => LoweringItem::Parameter {
                        name: declaration.name().to_owned(),
                        value_type: declaration.value_type().clone(),
                        value: crate::units::parameter_literal(file, declaration)?,
                        range: declaration.range(),
                    },
                    Item::Port(declaration) => LoweringItem::Port {
                        name: declaration.name().to_owned(),
                        contract: LoweringPortContract::Source(declaration.syntax().clone()),
                        range: declaration.range(),
                    },
                    Item::Clock(declaration) => LoweringItem::Clock {
                        name: declaration.name().to_owned(),
                        period: declaration.period().clone(),
                        phase: declaration.phase().clone(),
                        range: declaration.range(),
                    },
                    Item::Initial(declaration) => LoweringItem::Relation {
                        name: format!("$initial{item_index}"),
                        activation: ActivationSyntax::Continuous,
                        domain: None,
                        equations: declaration
                            .equations()
                            .iter()
                            .map(LoweringEquation::from_source)
                            .collect(),
                        initial: true,
                        range: declaration.range(),
                    },
                    Item::Relation(declaration) => LoweringItem::Relation {
                        initial: false,
                        name: declaration.name().to_owned(),
                        activation: declaration.activation().clone(),
                        domain: declaration.domain().map(str::to_owned),
                        equations: declaration
                            .equations()
                            .iter()
                            .map(LoweringEquation::from_source)
                            .collect(),
                        range: declaration.range(),
                    },
                    Item::Connection(c) => LoweringItem::Connection {
                        syntax: c.syntax(),
                        ports: c.port_paths().iter().map(|p| p.as_str().into()).collect(),
                        range: c.range(),
                    },
                    _ => LoweringItem::Unsupported {
                        range: model.range(),
                    },
                })
            })
            .collect::<Result<_, Diagnostic>>()?;
        let mut items: Vec<LoweringItem> = items;
        items.push(LoweringItem::Boundary {
            ports: model
                .signature()
                .iter()
                .filter_map(|item| match item {
                    eqiora_lang::SignatureItem::Input(value)
                    | eqiora_lang::SignatureItem::Output(value) => Some(value.name().to_owned()),
                    eqiora_lang::SignatureItem::Port(value) => Some(value.name().to_owned()),
                    _ => None,
                })
                .collect(),
            range: model.range(),
        });
        let mut represented_supports = std::collections::BTreeSet::new();
        let representations: Vec<_> = model
            .items()
            .iter()
            .filter_map(|item| {
                let Item::Field(field) = item else {
                    return None;
                };
                let domain = field.domain()?;
                represented_supports
                    .insert(domain)
                    .then(|| LoweringItem::Representation {
                        name: format!("$continuum-{domain}"),
                        range: field.range(),
                    })
            })
            .collect();
        items.extend(representations);
        Ok(Self {
            name: model.name().to_owned(),
            range: model.range(),
            items,
        })
    }
}
