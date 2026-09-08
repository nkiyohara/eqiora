//! Projection into the shared source AST, with synthetic ranges for diagnostics.

use std::collections::HashMap;

use eqiora_core::GraphPath;

use super::{DraftDeclaration, DraftPortReference, connection_path, value_type};
use crate::ast::{
    ActivationSyntax, ConnectionDecl, ConnectionSyntax, DomainDecl, DomainSyntax, Equation, Expr,
    ExprKind, FieldDecl, Item, ModelDecl, NamePath, ParameterDecl, PortDecl, PortSyntax,
    RelationDecl, TextRange, VisibilitySyntax,
};

/// Synthetic AST plus paths that recover native declaration context.
#[doc(hidden)]
#[derive(Debug)]
pub struct NativeModelAst {
    document: crate::Document,
    nominal_ids: HashMap<String, eqiora_core::RawId>,
    paths: HashMap<TextRange, GraphPath>,
}

impl super::ModelDraft {
    /// Build the private compiler bridge without formatting or parsing source.
    #[doc(hidden)]
    #[must_use]
    pub fn native_ast(&self) -> NativeModelAst {
        let mut ranges = RangeAllocator::default();
        let mut paths = HashMap::new();
        let mut items = Vec::with_capacity(self.declarations.len());
        let mut finite_spaces = Vec::new();
        let mut enumerations = Vec::new();
        let mut nominal_ids = HashMap::new();

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
                DraftDeclaration::Enum { name, definition } => {
                    let tags = definition
                        .members()
                        .iter()
                        .map(|tag| {
                            NamePath::from_segments([tag.as_str()], range)
                                .expect("checked enum tag")
                        })
                        .collect();
                    enumerations.push(
                        crate::SourceAstFactory::enumeration(
                            VisibilitySyntax::Private,
                            name.clone(),
                            tags,
                            range,
                        )
                        .expect("checked enum declaration"),
                    );
                    nominal_ids.insert(name.clone(), definition.id().erase());
                    continue;
                }
                DraftDeclaration::FiniteSpace { name, definition } => {
                    finite_spaces.push(
                        crate::SourceAstFactory::finite_space(
                            VisibilitySyntax::Private,
                            name.clone(),
                            definition.labels().to_vec(),
                            range,
                        )
                        .expect("checked finite space"),
                    );
                    nominal_ids.insert(name.clone(), definition.id().erase());
                    continue;
                }
                DraftDeclaration::IndexSet { name, definition } => {
                    nominal_ids.insert(name.clone(), definition.id().erase());
                    let extent = Expr {
                        resolved_enum: None,
                        resolved_nominal: None,
                        kind: ExprKind::Number(
                            crate::DecimalLiteral::parse(&definition.extent().to_string())
                                .expect("bounded extent"),
                        ),
                        range,
                    };
                    Item::IndexSet(
                        crate::SourceAstFactory::index_set(name.clone(), extent, range)
                            .expect("checked index set"),
                    )
                }
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
                            &mut |id| self.nominal_name(id),
                        ),
                        through_type: value_type::project(
                            &domain.through_type,
                            &path,
                            &mut ranges,
                            &mut paths,
                            &mut |id| self.nominal_name(id),
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
                        &mut |id| self.nominal_name(id),
                    ),
                    range,
                }),
                DraftDeclaration::Parameter(parameter) => Item::Parameter(ParameterDecl {
                    comments: Default::default(),
                    name: parameter.name.clone(),
                    value_type: value_type::project(
                        parameter.value_type(),
                        &path,
                        &mut ranges,
                        &mut paths,
                        &mut |id| self.nominal_name(id),
                    ),
                    value: crate::SourceAstFactory::value_literal(
                        parameter.value(),
                        parameter.frame_name(range),
                        range,
                        |id| self.nominal_name(id),
                        |id| self.enum_definition(id),
                    )
                    .expect("validated native Parameter projection"),
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
                        .equations
                        .iter()
                        .map(|(left, right)| {
                            let left = left
                                .ast(
                                    &path,
                                    &mut ranges,
                                    &mut paths,
                                    &mut |id| self.nominal_name(id),
                                    &mut |id| self.enum_definition(id),
                                )
                                .expect("validated native expression scope");
                            let right = right
                                .ast(
                                    &path,
                                    &mut ranges,
                                    &mut paths,
                                    &mut |id| self.nominal_name(id),
                                    &mut |id| self.enum_definition(id),
                                )
                                .expect("validated native expression scope");
                            let range = left.range();
                            Equation { left, right, range }
                        })
                        .collect(),
                    range,
                }),
                DraftDeclaration::Initial(residuals) => Item::Initial(crate::ast::InitialDecl {
                    comments: Default::default(),
                    equations: residuals
                        .iter()
                        .map(|(left, right)| {
                            let left = left
                                .ast(
                                    &path,
                                    &mut ranges,
                                    &mut paths,
                                    &mut |id| self.nominal_name(id),
                                    &mut |id| self.enum_definition(id),
                                )
                                .expect("validated native expression scope");
                            let right = right
                                .ast(
                                    &path,
                                    &mut ranges,
                                    &mut paths,
                                    &mut |id| self.nominal_name(id),
                                    &mut |id| self.enum_definition(id),
                                )
                                .expect("validated native expression scope");
                            let range = left.range();
                            Equation { left, right, range }
                        })
                        .collect(),
                    range,
                }),
                DraftDeclaration::ConservingConnection(connection) => {
                    Item::Connection(ConnectionDecl {
                        binder: None,
                        comments: Default::default(),
                        syntax: ConnectionSyntax::Conserving,
                        ports: connection
                            .ports
                            .iter()
                            .map(|port| Expr {
                                resolved_enum: None,
                                resolved_nominal: None,
                                kind: ExprKind::Name(port.name.clone()),
                                range,
                            })
                            .collect(),
                        range,
                    })
                }
            };
            items.push(item);
        }

        let model_path = GraphPath::new([self.name.clone()]);
        let range = ranges.allocate(&model_path, &mut paths);
        let model = ModelDecl {
            signature: Vec::new(),
            comments: Default::default(),
            visibility: VisibilitySyntax::Private,
            name: self.name.clone(),
            items,
            range,
        };
        let mut document =
            crate::SourceAstFactory::document(enumerations, vec![], vec![], vec![model])
                .expect("native model document");
        document.finite_spaces = finite_spaces;
        NativeModelAst {
            document,
            nominal_ids,
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
        arguments: crate::CallArguments::Positional(vec![Expr {
            resolved_enum: None,
            resolved_nominal: None,
            kind: ExprKind::Name(reference.name.clone()),
            range: ranges.allocate(path, paths),
        }]),
    }
}

impl NativeModelAst {
    /// Source-shaped model consumed by the shared compiler lowerer.
    #[must_use]
    pub fn model(&self) -> &ModelDecl {
        &self.document.models[0]
    }

    /// Complete source unit including nominal declarations.
    pub fn document(&self) -> &crate::Document {
        &self.document
    }

    /// Exact identity supplied by a native nominal declaration.
    pub fn nominal_identity(&self, name: &str) -> Option<eqiora_core::RawId> {
        self.nominal_ids.get(name).copied()
    }

    /// Native declaration path associated with one synthetic range.
    #[must_use]
    pub fn graph_path(&self, range: TextRange) -> Option<&GraphPath> {
        self.paths.get(&range)
    }
}
