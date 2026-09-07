//! Projection into the shared source AST, with synthetic ranges for diagnostics.

use std::collections::HashMap;

use eqiora_core::GraphPath;

use super::{DraftDeclaration, DraftPortReference, NativeModelAst, connection_path, value_type};
use crate::ast::{
    ActivationSyntax, ConnectionDecl, ConnectionSyntax, DomainDecl, DomainSyntax, Equation, Expr,
    ExprKind, FieldDecl, Item, ModelDecl, NamePath, ParameterDecl, PortDecl, PortSyntax,
    RelationDecl, TextRange, VisibilitySyntax,
};

impl super::ModelDraft {
    /// Build the private compiler bridge without formatting or parsing source.
    #[doc(hidden)]
    #[must_use]
    pub fn native_ast(&self) -> NativeModelAst {
        let mut ranges = RangeAllocator::default();
        let mut paths = HashMap::new();
        let mut items = Vec::with_capacity(self.declarations.len());

        for declaration in &self.declarations {
            let declaration_path =
                declaration
                    .name()
                    .map(str::to_owned)
                    .unwrap_or_else(|| match declaration {
                        DraftDeclaration::ConservingConnection(connection) => {
                            connection_path(connection)
                        }
                        DraftDeclaration::Initial(_) => "initial".to_owned(),
                        _ => unreachable!("named declaration"),
                    });
            let path = GraphPath::new([self.name.clone(), declaration_path]);
            let range = ranges.allocate(&path, &mut paths);
            let item = match declaration {
                DraftDeclaration::SpatialDomain(domain) => Item::Domain(DomainDecl {
                    comments: Default::default(),
                    name: domain.name().to_owned(),
                    syntax: domain.syntax(),
                    range,
                }),
                DraftDeclaration::PhysicalDomain(domain) => Item::Domain(DomainDecl {
                    comments: Default::default(),
                    name: domain.name.clone(),
                    syntax: DomainSyntax::ScalarPhysical {
                        across_type: value_type::project(
                            &domain.across_type,
                            &path,
                            &mut ranges,
                            &mut paths,
                        ),
                        through_type: value_type::project(
                            &domain.through_type,
                            &path,
                            &mut ranges,
                            &mut paths,
                        ),
                    },
                    range,
                }),
                DraftDeclaration::Field(field) => Item::Field(FieldDecl {
                    comments: Default::default(),
                    name: field.name.clone(),
                    domain: field
                        .spatial_scope
                        .as_ref()
                        .map(|scope| scope.domain.name().to_owned()),
                    role: field.role,
                    activation: ActivationSyntax::Continuous,
                    value_type: value_type::project(
                        &field.value_type,
                        &path,
                        &mut ranges,
                        &mut paths,
                    ),
                    range,
                }),
                DraftDeclaration::Parameter(parameter) => Item::Parameter(ParameterDecl {
                    comments: Default::default(),
                    name: parameter.name.clone(),
                    value_type: value_type::project(
                        &parameter.value_type,
                        &path,
                        &mut ranges,
                        &mut paths,
                    ),
                    value: Expr {
                        kind: ExprKind::Number(parameter.value),
                        range,
                    },
                    range,
                }),
                DraftDeclaration::ConservingPort(port) => Item::Port(PortDecl {
                    comments: Default::default(),
                    name: port.name.clone(),
                    syntax: PortSyntax::ScalarPhysical {
                        domain: port.domain.name.clone(),
                    },
                    range,
                }),
                DraftDeclaration::Relation(relation) => Item::Relation(RelationDecl {
                    comments: Default::default(),
                    name: relation.name.clone(),
                    activation: ActivationSyntax::Continuous,
                    domain: relation
                        .domain
                        .as_ref()
                        .map(|domain| domain.name().to_owned()),
                    equations: relation
                        .residuals
                        .iter()
                        .map(|expression| {
                            let left = expression.ast(&path, &mut ranges, &mut paths);
                            let range = left.range();
                            Equation {
                                left,
                                right: Expr {
                                    kind: ExprKind::Number(0.0),
                                    range,
                                },
                                range,
                            }
                        })
                        .collect(),
                    range,
                }),
                DraftDeclaration::Initial(residuals) => Item::Initial(crate::ast::InitialDecl {
                    comments: Default::default(),
                    equations: residuals
                        .iter()
                        .map(|expression| {
                            let left = expression.ast(&path, &mut ranges, &mut paths);
                            let range = left.range();
                            Equation {
                                left,
                                right: Expr {
                                    kind: ExprKind::Number(0.0),
                                    range,
                                },
                                range,
                            }
                        })
                        .collect(),
                    range,
                }),
                DraftDeclaration::ConservingConnection(connection) => {
                    Item::Connection(ConnectionDecl {
                        comments: Default::default(),
                        syntax: ConnectionSyntax::Conserving,
                        ports: connection
                            .ports
                            .iter()
                            .map(|port| NamePath::single(port.name.clone(), range))
                            .collect(),
                        range,
                    })
                }
            };
            items.push(item);
        }

        let model_path = GraphPath::new([self.name.clone()]);
        let range = ranges.allocate(&model_path, &mut paths);
        NativeModelAst {
            model: ModelDecl {
                comments: Default::default(),
                visibility: VisibilitySyntax::Private,
                name: self.name.clone(),
                items,
                range,
            },
            paths,
        }
    }
}

#[derive(Debug, Default)]
pub(super) struct RangeAllocator {
    next: u32,
}

impl RangeAllocator {
    pub(super) fn allocate(
        &mut self,
        path: &GraphPath,
        paths: &mut HashMap<TextRange, GraphPath>,
    ) -> TextRange {
        let start = self.next;
        self.next = self.next.saturating_add(1);
        let range = TextRange::new(start, self.next);
        paths.insert(range, path.clone());
        range
    }
}

pub(super) fn physical_accessor_ast(
    callee: &str,
    reference: &DraftPortReference,
    path: &GraphPath,
    ranges: &mut RangeAllocator,
    paths: &mut HashMap<TextRange, GraphPath>,
) -> ExprKind {
    ExprKind::Call {
        callee: NamePath::single(callee.to_owned(), ranges.allocate(path, paths)),
        arguments: vec![Expr {
            kind: ExprKind::Name(reference.name.clone()),
            range: ranges.allocate(path, paths),
        }],
    }
}

impl NativeModelAst {
    /// Source-shaped model consumed by the shared compiler lowerer.
    #[must_use]
    pub const fn model(&self) -> &ModelDecl {
        &self.model
    }

    /// Native declaration path associated with one synthetic range.
    #[must_use]
    pub fn graph_path(&self, range: TextRange) -> Option<&GraphPath> {
        self.paths.get(&range)
    }
}
