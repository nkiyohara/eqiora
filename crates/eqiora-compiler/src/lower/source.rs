use super::*;

impl LoweringModel {
    pub(super) fn from_source(model: &ModelDecl) -> Self {
        let items = model
            .items()
            .iter()
            .map(|item| match item {
                Item::Domain(declaration) => LoweringItem::Domain {
                    name: declaration.name().to_owned(),
                    contract: LoweringDomainContract::Source(declaration.syntax().clone()),
                    range: declaration.range(),
                },
                Item::Representation(declaration) => LoweringItem::Representation {
                    name: declaration.name().to_owned(),
                    syntax: declaration.syntax(),
                    range: declaration.range(),
                },
                Item::Field(declaration) => LoweringItem::Field {
                    name: declaration.name().to_owned(),
                    domain: declaration.domain().map(str::to_owned),
                    representation: declaration.representation().map(str::to_owned),
                    shape: declaration.shape().cloned(),
                    dimension: declaration.dimension().clone(),
                    initial: declaration.initial(),
                    range: declaration.range(),
                },
                Item::Parameter(declaration) => LoweringItem::Parameter {
                    name: declaration.name().to_owned(),
                    dimension: declaration.dimension().clone(),
                    value: declaration.initial(),
                    range: declaration.range(),
                },
                Item::Port(declaration) => LoweringItem::Port {
                    name: declaration.name().to_owned(),
                    contract: LoweringPortContract::Source(declaration.syntax().clone()),
                    range: declaration.range(),
                },
                Item::Clock(declaration) => LoweringItem::Clock {
                    name: declaration.name().to_owned(),
                    period: declaration.period(),
                    phase: declaration.phase(),
                    range: declaration.range(),
                },
                Item::Relation(declaration) => LoweringItem::Relation {
                    name: declaration.name().to_owned(),
                    activation: declaration.activation().clone(),
                    domain: declaration.domain().map(str::to_owned),
                    residuals: declaration
                        .residuals()
                        .iter()
                        .map(LoweringExpression::from_source)
                        .collect(),
                    range: declaration.range(),
                },
                Item::Connection(c) => LoweringItem::Connection {
                    syntax: c.syntax(),
                    ports: c.port_paths().iter().map(|p| p.as_str().into()).collect(),
                    range: c.range(),
                },
                Item::Boundary(declaration) => LoweringItem::Boundary {
                    ports: declaration.ports().map(str::to_owned).collect(),
                    range: declaration.range(),
                },
                _ => LoweringItem::Unsupported {
                    range: model.range(),
                },
            })
            .collect();
        Self {
            name: model.name().to_owned(),
            range: model.range(),
            items,
        }
    }
}
