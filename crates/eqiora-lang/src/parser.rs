//! Recovering recursive-descent parser for the deliberately small v0 grammar.
use eqiora_core::diagnostic::codes;
use eqiora_core::{Diagnostic, Span};

mod comments;
mod compile_time;
mod dimension;
mod document;
mod domain;
mod expression;
mod formulation;
mod instance;
mod operator;
mod property;
mod relation;
mod signature;
mod value_type;

use crate::ast::{
    BinaryOp, BoundaryConnectionDecl, BoundaryDecl, BoundaryFamilyBinderSyntax,
    BoundaryPairingSyntax, BoundaryPortReferenceSyntax, BoundaryPortSelectorSyntax,
    BoundarySetBindingDecl, BoundarySetMemberSyntax, BoundarySideSyntax, ClockDecl, ComponentItem,
    ComponentParameterDecl, ComponentPortDecl, ComponentPortFamilyDecl, ConnectionDecl,
    ConnectionSyntax, ConnectorDecl, ConnectorQuantitySyntax, ConnectorSyntax, Document,
    DomainDecl, DomainSyntax, ExactIntegerSyntax, Expr, ExprKind, FieldBindingDecl, FieldDecl,
    FrameSyntax, InstanceDecl, Item, NamePath, ParameterBindingDecl, PortDecl, PortSyntax,
    PureOperatorBinaryOp, PureOperatorDecl, PureOperatorExpr, PureOperatorExprKind,
    PureOperatorFormal, PureValueClassSyntax, RationalSyntax, RepresentationDecl,
    RepresentationSyntax, SignalDirectionSyntax, SupportBindingDecl, SupportSlotDecl,
    SupportSlotSyntax, TextRange, UnaryOp, ValueShapeSyntax, VisibilitySyntax,
};
use crate::lexer::{Token, TokenKind, lex};
use relation::ParsedRelation;

/// Parsed document, lossless tokens, and accumulated source diagnostics.
#[derive(Debug, Clone, PartialEq)]
pub struct ParseResult {
    file: String,
    tokens: Vec<Token>,
    document: Option<Document>,
    diagnostics: Vec<Diagnostic>,
}

impl ParseResult {
    fn missing_declaration_diagnostic(&self, message: &'static str) -> Diagnostic {
        let range = self
            .tokens
            .last()
            .map_or(TextRange::new(0, 0), Token::range);
        Diagnostic::error(codes::SYNTAX_ERROR, message).with_span(Span {
            file: self.file.clone(),
            start: range.start(),
            end: range.end(),
        })
    }

    /// Lossless token stream, including trivia and EOF.
    #[must_use]
    pub fn tokens(&self) -> &[Token] {
        &self.tokens
    }

    /// Recovered syntax tree when at least one top-level declaration was parsed.
    #[must_use]
    pub const fn document(&self) -> Option<&Document> {
        self.document.as_ref()
    }

    /// Lexical and syntactic diagnostics in source order.
    #[must_use]
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    /// Consume the result only when no diagnostic was produced.
    ///
    /// # Errors
    /// Returns all accumulated diagnostics, including recoverable errors.
    pub fn into_document(self) -> Result<Document, Vec<Diagnostic>> {
        let missing =
            self.missing_declaration_diagnostic("source contains no top-level declaration");
        if self.diagnostics.is_empty() {
            self.document.ok_or_else(|| vec![missing])
        } else {
            Err(self.diagnostics)
        }
    }

    /// Consume the result only when it contains an executable Model and no
    /// diagnostic was produced.
    ///
    /// Declaration-only documents are valid library syntax through
    /// [`Self::into_document`], but are not executable compilation entries.
    ///
    /// # Errors
    /// Returns all accumulated diagnostics and reports a missing Model when
    /// the recovered document contains declarations only.
    pub fn into_compilation_document(self) -> Result<Document, Vec<Diagnostic>> {
        let missing =
            self.missing_declaration_diagnostic("source requires at least one `model` declaration");
        let mut diagnostics = self.diagnostics;
        let has_model = self
            .document
            .as_ref()
            .is_some_and(|document| !document.models.is_empty());
        if !has_model {
            diagnostics.push(missing);
        }
        if diagnostics.is_empty() {
            self.document.ok_or_else(|| {
                vec![Diagnostic::error(
                    codes::SYNTAX_ERROR,
                    "parser invariant violated: a Model-bearing document was not retained",
                )]
            })
        } else {
            Err(diagnostics)
        }
    }
}

/// Parse one Eqiora Language source file, including declaration-only package
/// library units.
#[must_use]
pub fn parse(file: impl Into<String>, source: &str) -> ParseResult {
    let file = file.into();
    let (tokens, mut diagnostics) = lex(file.clone(), source).into_parts();
    let mut parser = Parser {
        file: file.clone(),
        tokens: &tokens,
        cursor: 0,
        expression_recursion: 0,
        diagnostics: Vec::new(),
    };
    let mut document = parser.parse_document();
    diagnostics.extend(parser.diagnostics);
    if let Some(document) = &mut document {
        comments::attach(document, &tokens, source, &file, &mut diagnostics);
    }
    ParseResult {
        file,
        tokens,
        document,
        diagnostics,
    }
}

struct Parser<'a> {
    file: String,
    tokens: &'a [Token],
    cursor: usize,
    expression_recursion: usize,
    diagnostics: Vec<Diagnostic>,
}

enum ParsedComponentItem {
    Retained(Box<ComponentItem>),
    Discarded,
}

enum ParsedComponentPort {
    Ordinary(ComponentPortDecl),
    Family(ComponentPortFamilyDecl),
}

enum ParsedConnection {
    Ordinary(ConnectionDecl),
    Boundary(BoundaryConnectionDecl),
}

impl Parser<'_> {
    fn parse_connector(
        &mut self,
        start: u32,
        visibility: VisibilitySyntax,
    ) -> Option<ConnectorDecl> {
        self.expect_keyword("connector")?;
        let name = self.expect_identifier("Connector name")?.text().to_owned();
        self.expect(TokenKind::Equal, "`=` before Connector family")?;
        let syntax = if self.at_keyword("scalar_physical") {
            let (across_type, through_type) = self.parse_scalar_physical_types()?;
            ConnectorSyntax::ScalarPhysical {
                across_type,
                through_type,
            }
        } else if self.at_keyword("field_physical") {
            self.parse_field_physical_connector()?
        } else {
            self.error_here("expected `scalar_physical(...)` or `field_physical(...)`");
            return None;
        };
        let end = self
            .expect(TokenKind::Semicolon, "`;` after Connector")?
            .range()
            .end();
        Some(ConnectorDecl {
            comments: Default::default(),
            visibility,
            name,
            syntax,
            range: TextRange::new(start, end),
        })
    }

    fn parse_component_item(&mut self) -> Option<ParsedComponentItem> {
        let public = self.at_keyword("public");
        let start = if public {
            self.bump().range().start()
        } else {
            self.current().range().start()
        };
        let visibility = if public {
            VisibilitySyntax::Public
        } else {
            VisibilitySyntax::Private
        };

        if self.at_keyword("parameter") {
            return self
                .parse_component_parameter(start, visibility)
                .map(ComponentItem::Parameter)
                .map(Box::new)
                .map(ParsedComponentItem::Retained);
        }
        if self.at_keyword("port") {
            return self.parse_component_port(start, visibility).map(|port| {
                ParsedComponentItem::Retained(Box::new(match port {
                    ParsedComponentPort::Ordinary(port) => ComponentItem::Port(port),
                    ParsedComponentPort::Family(port) => ComponentItem::PortFamily(port),
                }))
            });
        }
        if public {
            let token = self.current().clone();
            self.error_token(
                &token,
                "only `parameter` and `port` body declarations may be public; support and unknown requirements belong in the signature",
            );
            if self.at_keyword("representation") {
                self.parse_representation()?;
            } else if self.at_keyword("variable") || self.at_keyword("state") {
                self.parse_field(true)?;
            } else if self.at_keyword("clock") {
                self.parse_clock()?;
            } else if self.at_keyword("relation") {
                self.parse_component_relation()?;
            } else if self.at_keyword("connect") {
                self.parse_connection(true)?;
            } else if self.at_keyword("instance") {
                self.parse_instance()?;
            } else {
                return None;
            }
            return Some(ParsedComponentItem::Discarded);
        }

        let item = if self.at_keyword("representation") {
            self.parse_representation()
                .map(ComponentItem::Representation)
        } else if self.at_keyword("variable") || self.at_keyword("state") {
            self.parse_field(true).map(ComponentItem::Field)
        } else if self.at_keyword("initial") {
            self.parse_initial().map(ComponentItem::Initial)
        } else if self.at_keyword("clock") {
            self.parse_clock().map(ComponentItem::Clock)
        } else if self.at_keyword("relation") {
            self.parse_component_relation()
                .map(|relation| match relation {
                    ParsedRelation::Ordinary(relation) => ComponentItem::Relation(relation),
                    ParsedRelation::Family(relation) => ComponentItem::RelationFamily(relation),
                })
        } else if self.at_keyword("connect") {
            self.parse_connection(true)
                .map(|connection| match connection {
                    ParsedConnection::Ordinary(connection) => ComponentItem::Connection(connection),
                    ParsedConnection::Boundary(connection) => {
                        ComponentItem::BoundaryConnection(connection)
                    }
                })
        } else if self.at_keyword("instance") {
            self.parse_instance().map(ComponentItem::Instance)
        } else {
            self.error_here(
                "expected parameter, port, support, representation, field, clock, relation, connect, or instance in component",
            );
            None
        }?;
        Some(ParsedComponentItem::Retained(Box::new(item)))
    }

    fn parse_item(&mut self) -> Option<Item> {
        if self.at_keyword("domain") {
            self.parse_domain().map(Item::Domain)
        } else if self.at_keyword("representation") {
            self.parse_representation().map(Item::Representation)
        } else if self.at_keyword("variable") || self.at_keyword("state") {
            self.parse_field(true).map(Item::Field)
        } else if self.at_keyword("initial") {
            self.parse_initial().map(Item::Initial)
        } else if self.at_keyword("parameter") {
            self.parse_parameter().map(Item::Parameter)
        } else if self.at_keyword("let") {
            self.parse_let().map(Item::Let)
        } else if self.at_keyword("port") {
            self.parse_port().map(Item::Port)
        } else if self.at_keyword("clock") {
            self.parse_clock().map(Item::Clock)
        } else if self.at_keyword("relation") {
            self.parse_relation().map(Item::Relation)
        } else if self.at_keyword("connect") {
            self.parse_connection(false)
                .map(|connection| match connection {
                    ParsedConnection::Ordinary(connection) => Item::Connection(connection),
                    ParsedConnection::Boundary(connection) => Item::BoundaryConnection(connection),
                })
        } else if self.at_keyword("boundary") {
            self.parse_boundary().map(Item::Boundary)
        } else if self.at_keyword("instance") {
            self.parse_instance().map(Item::Instance)
        } else {
            self.error_here(
                "expected domain, representation, field, parameter, let, port, clock, relation, connect, boundary, or instance",
            );
            None
        }
    }

    fn parse_scalar_physical_types(
        &mut self,
    ) -> Option<(crate::ValueTypeSyntax, crate::ValueTypeSyntax)> {
        self.expect_keyword("scalar_physical")?;
        self.expect(TokenKind::LeftParen, "`(` after `scalar_physical`")?;
        self.expect_keyword("across")?;
        self.expect(TokenKind::Equal, "`=` after `across`")?;
        let across_type = self.parse_value_type()?;
        self.expect(TokenKind::Comma, "`,` between physical types")?;
        self.expect_keyword("through")?;
        self.expect(TokenKind::Equal, "`=` after `through`")?;
        let through_type = self.parse_value_type()?;
        self.expect(TokenKind::RightParen, "`)` after physical types")?;
        Some((across_type, through_type))
    }

    fn parse_field_physical_connector(&mut self) -> Option<ConnectorSyntax> {
        self.expect_keyword("field_physical")?;
        self.expect(TokenKind::LeftParen, "`(` after `field_physical`")?;
        let mut trace = None;
        let mut flux = None;
        let mut shape = None;
        let mut frame = None;
        let mut pairing = None;

        while !self.at(TokenKind::RightParen) && !self.at(TokenKind::Eof) {
            let field = self
                .expect_identifier("field-physical Connector field")?
                .text()
                .to_owned();
            self.expect(TokenKind::Equal, "`=` after Connector field name")?;
            match field.as_str() {
                "trace" => {
                    if trace.is_some() {
                        self.error_previous("duplicate `trace` Connector field");
                        return None;
                    }
                    trace = Some(self.parse_connector_quantity("trace")?);
                }
                "flux" => {
                    if flux.is_some() {
                        self.error_previous("duplicate `flux` Connector field");
                        return None;
                    }
                    flux = Some(self.parse_connector_quantity("flux")?);
                }
                "shape" => {
                    if shape.is_some() {
                        self.error_previous("duplicate `shape` Connector field");
                        return None;
                    }
                    shape = Some(self.parse_value_shape()?);
                }
                "frame" => {
                    if frame.is_some() {
                        self.error_previous("duplicate `frame` Connector field");
                        return None;
                    }
                    frame = Some(if self.at_keyword("invariant") {
                        self.bump();
                        FrameSyntax::Invariant
                    } else if self.at_keyword("spatial") {
                        self.bump();
                        FrameSyntax::Spatial
                    } else {
                        self.error_here("expected `invariant` or `spatial` frame");
                        return None;
                    });
                }
                "pairing" => {
                    if pairing.is_some() {
                        self.error_previous("duplicate `pairing` Connector field");
                        return None;
                    }
                    self.expect_keyword("euclidean_boundary_duality")?;
                    pairing = Some(BoundaryPairingSyntax::EuclideanBoundaryDuality);
                }
                _ => {
                    self.error_previous(format!(
                        "unknown field-physical Connector field `{field}`"
                    ));
                    return None;
                }
            }
            if self.at(TokenKind::Comma) {
                self.bump();
                if self.at(TokenKind::RightParen) {
                    self.error_here("trailing comma is not admitted in `field_physical(...)`");
                    return None;
                }
            } else if !self.at(TokenKind::RightParen) {
                self.error_here("expected `,` or `)` after Connector field");
                return None;
            }
        }
        self.expect(TokenKind::RightParen, "`)` after field-physical Connector")?;

        Some(ConnectorSyntax::FieldPhysical {
            trace: self.require_connector_field(trace, "trace")?,
            flux: self.require_connector_field(flux, "flux")?,
            shape: self.require_connector_field(shape, "shape")?,
            frame: self.require_connector_field(frame, "frame")?,
            pairing: self.require_connector_field(pairing, "pairing")?,
        })
    }

    fn parse_connector_quantity(&mut self, role: &str) -> Option<ConnectorQuantitySyntax> {
        let name = self
            .expect_identifier(&format!("{role} quantity name"))?
            .text()
            .to_owned();
        self.expect(TokenKind::Colon, "`:` before quantity dimension")?;
        Some(ConnectorQuantitySyntax {
            name,
            dimension: self.parse_dimension_expression()?,
        })
    }

    fn require_connector_field<T>(&mut self, value: Option<T>, field: &str) -> Option<T> {
        value.or_else(|| {
            self.error_previous(format!(
                "field-physical Connector requires exactly one `{field}` field"
            ));
            None
        })
    }

    fn parse_value_shape(&mut self) -> Option<ValueShapeSyntax> {
        if self.at_keyword("scalar") {
            self.bump();
            return Some(ValueShapeSyntax::Scalar);
        }
        if self.at_keyword("spatial_vector") {
            self.bump();
            return Some(ValueShapeSyntax::SpatialVector);
        }
        self.expect(
            TokenKind::LeftBracket,
            "`scalar`, `spatial_vector`, or `[` for an exact value shape",
        )?;
        let mut extents = Vec::new();
        if !self.at(TokenKind::RightBracket) {
            loop {
                let extent = self.parse_u64("positive value-shape extent")?;
                if extent == 0 {
                    self.error_previous("value-shape extents must be positive");
                    return None;
                }
                let extent = u32::try_from(extent).ok().or_else(|| {
                    self.error_previous("value-shape extent exceeds the portable u32 range");
                    None
                })?;
                extents.push(extent);
                if !self.at(TokenKind::Comma) {
                    break;
                }
                self.bump();
            }
        }
        self.expect(TokenKind::RightBracket, "`]` after exact value shape")?;
        if extents.is_empty() {
            Some(ValueShapeSyntax::Scalar)
        } else {
            Some(ValueShapeSyntax::Exact(extents))
        }
    }

    fn parse_representation(&mut self) -> Option<RepresentationDecl> {
        let start = self.expect_keyword("representation")?.range().start();
        let name = self
            .expect_identifier("Representation name")?
            .text()
            .to_owned();
        self.expect(TokenKind::Equal, "`=` before Representation family")?;
        self.expect_keyword("continuum")?;
        let end = self
            .expect(TokenKind::Semicolon, "`;` after Representation")?
            .range()
            .end();
        Some(RepresentationDecl {
            comments: Default::default(),
            name,
            syntax: RepresentationSyntax::Continuum,
            range: TextRange::new(start, end),
        })
    }

    fn parse_field(&mut self, terminated: bool) -> Option<FieldDecl> {
        let role = if self.at_keyword("variable") {
            crate::ast::FieldRoleSyntax::Variable
        } else if self.at_keyword("state") {
            crate::ast::FieldRoleSyntax::State
        } else {
            self.error_here("expected `variable` or `state`");
            return None;
        };
        let start = self.bump().range().start();
        let name = self
            .expect_identifier("declaration name")?
            .text()
            .to_owned();
        self.expect(TokenKind::Colon, "`:` before mathematical type")?;
        let value_type = self.parse_value_type()?;
        let domain = if self.at_keyword("on") {
            self.bump();
            Some(self.expect_identifier("unknown support")?.text().to_owned())
        } else {
            None
        };
        let activation = if self.at_keyword("at") {
            self.bump();
            crate::ActivationSyntax::Periodic(
                self.expect_identifier("unknown clock")?.text().to_owned(),
            )
        } else {
            crate::ActivationSyntax::Continuous
        };
        let end = if terminated {
            self.expect(TokenKind::Semicolon, "`;` after declaration")?
                .range()
                .end()
        } else {
            self.previous_significant_range().end()
        };
        Some(FieldDecl {
            comments: Default::default(),
            name,
            domain,
            role,
            activation,
            value_type,
            range: TextRange::new(start, end),
        })
    }

    fn parse_component_parameter(
        &mut self,
        start: u32,
        visibility: VisibilitySyntax,
    ) -> Option<ComponentParameterDecl> {
        self.expect_keyword("parameter")?;
        let name = self
            .expect_identifier("component Parameter name")?
            .text()
            .to_owned();
        self.expect(TokenKind::Colon, "`:` before component Parameter type")?;
        let value_type = self.parse_value_type()?;
        let default = if self.at(TokenKind::Equal) {
            self.bump();
            Some(self.parse_expression(0)?)
        } else {
            None
        };
        let end = self
            .expect(TokenKind::Semicolon, "`;` after component Parameter")?
            .range()
            .end();
        Some(ComponentParameterDecl {
            comments: Default::default(),
            visibility,
            name,
            value_type,
            default,
            range: TextRange::new(start, end),
        })
    }

    fn parse_support_slot(
        &mut self,
        start: u32,
        visibility: VisibilitySyntax,
    ) -> Option<SupportSlotDecl> {
        self.expect_keyword("support")?;
        let name = self
            .expect_identifier("component support-slot name")?
            .text()
            .to_owned();
        self.expect(TokenKind::Colon, "`:` before support-slot contract")?;
        let syntax = if self.at_keyword("volume") {
            self.bump();
            self.expect(TokenKind::LeftParen, "`(` after `volume`")?;
            self.expect_keyword("ambient_dimension")?;
            self.expect(TokenKind::Equal, "`=` after `ambient_dimension`")?;
            let ambient_dimension = self.parse_u64("ambient dimension")?;
            let ambient_dimension = usize::try_from(ambient_dimension).ok().or_else(|| {
                self.error_previous("ambient dimension exceeds this platform's usize range");
                None
            })?;
            self.expect(TokenKind::RightParen, "`)` after volume support contract")?;
            SupportSlotSyntax::Volume { ambient_dimension }
        } else if self.at_keyword("boundary") {
            self.bump();
            self.expect(TokenKind::LeftParen, "`(` after `boundary`")?;
            self.expect_keyword("parent")?;
            self.expect(TokenKind::Equal, "`=` after boundary parent")?;
            let parent = self
                .expect_identifier("parent support-slot name")?
                .text()
                .to_owned();
            self.expect(TokenKind::RightParen, "`)` after boundary support contract")?;
            SupportSlotSyntax::Boundary { parent }
        } else if self.at_keyword("complete_exterior") {
            self.bump();
            self.expect(TokenKind::LeftParen, "`(` after `complete_exterior`")?;
            self.expect_keyword("parent")?;
            self.expect(TokenKind::Equal, "`=` after complete exterior parent")?;
            let parent = self
                .expect_identifier("parent support-slot name")?
                .text()
                .to_owned();
            self.expect(
                TokenKind::RightParen,
                "`)` after complete exterior support contract",
            )?;
            SupportSlotSyntax::CompleteExterior { parent }
        } else {
            self.error_here(
                "expected `volume(...)`, `boundary(...)`, or `complete_exterior(...)` support-slot contract",
            );
            return None;
        };
        let end = self.previous_significant_range().end();
        Some(SupportSlotDecl {
            comments: Default::default(),
            visibility,
            name,
            syntax,
            range: TextRange::new(start, end),
        })
    }

    fn parse_port(&mut self) -> Option<PortDecl> {
        let start = self.expect_keyword("port")?.range().start();
        let name = self.expect_identifier("Port name")?.text().to_owned();
        self.expect(TokenKind::Colon, "`:` before Port contract")?;
        let syntax = if self.at_keyword("signal") {
            self.parse_signal_port_syntax()?
        } else if self.at_keyword("conserving") {
            self.bump();
            if self.at_keyword("on") {
                self.bump();
                PortSyntax::ScalarPhysical {
                    domain: self
                        .expect_identifier("scalar physical Domain name")?
                        .text()
                        .to_owned(),
                }
            } else {
                let connector = self
                    .at(TokenKind::Identifier)
                    .then(|| self.parse_name_path("field-physical Connector name"))
                    .flatten();
                match connector {
                    Some(connector) if self.at_keyword("over") => {
                        self.bump();
                        PortSyntax::FieldPhysical {
                            connector,
                            support: self
                                .expect_identifier("boundary support name")?
                                .text()
                                .to_owned(),
                        }
                    }
                    _ => {
                        self.error_here(
                            "conserving Port requires a physical Domain or boundary Connector",
                        );
                        return None;
                    }
                }
            }
        } else {
            self.error_here("expected `signal` or `conserving` Port contract");
            return None;
        };
        let end = self
            .expect(TokenKind::Semicolon, "`;` after Port declaration")?
            .range()
            .end();
        Some(PortDecl {
            comments: Default::default(),
            name,
            syntax,
            range: TextRange::new(start, end),
        })
    }

    fn parse_component_port(
        &mut self,
        start: u32,
        visibility: VisibilitySyntax,
    ) -> Option<ParsedComponentPort> {
        self.expect_keyword("port")?;
        let name = self
            .expect_identifier("component Port name")?
            .text()
            .to_owned();
        let binder = self
            .at(TokenKind::LeftBracket)
            .then(|| self.parse_boundary_family_binder())
            .flatten();
        self.expect(TokenKind::Colon, "`:` before component Port contract")?;
        let syntax = if self.at_keyword("signal") {
            self.parse_signal_port_syntax()?
        } else if self.at_keyword("conserving") {
            self.bump();
            if self.at_keyword("on") {
                self.bump();
                PortSyntax::ScalarPhysicalConnector {
                    connector: self.parse_name_path("scalar physical Connector name")?,
                }
            } else {
                let connector = self.parse_name_path("field-physical Connector name")?;
                self.expect_keyword("over")?;
                PortSyntax::FieldPhysical {
                    connector,
                    support: self
                        .expect_identifier("boundary support-slot name")?
                        .text()
                        .to_owned(),
                }
            }
        } else {
            self.error_here("expected `signal` or `conserving on Connector` Port contract");
            return None;
        };
        let end = self
            .expect(TokenKind::Semicolon, "`;` after component Port")?
            .range()
            .end();
        let port = ComponentPortDecl {
            comments: Default::default(),
            visibility,
            name,
            syntax,
            range: TextRange::new(start, end),
        };
        let Some(binder) = binder else {
            return Some(ParsedComponentPort::Ordinary(port));
        };
        let PortSyntax::FieldPhysical { support, .. } = port.syntax() else {
            self.error_here("a boundary family binder requires a field-physical Port");
            return None;
        };
        if support != binder.member() {
            self.error_here(
                "a field-physical Port family must be declared over its bound boundary member",
            );
            return None;
        }
        Some(ParsedComponentPort::Family(ComponentPortFamilyDecl {
            port,
            binder,
        }))
    }

    fn parse_signal_port_syntax(&mut self) -> Option<PortSyntax> {
        self.expect_keyword("signal")?;
        let direction = if self.at_keyword("input") {
            self.bump();
            SignalDirectionSyntax::Input
        } else if self.at_keyword("output") {
            self.bump();
            SignalDirectionSyntax::Output
        } else {
            self.error_here("expected `input` or `output` after `signal`");
            return None;
        };
        Some(PortSyntax::Signal {
            direction,
            value_type: self.parse_value_type()?,
        })
    }

    fn parse_clock(&mut self) -> Option<ClockDecl> {
        let start = self.expect_keyword("clock")?.range().start();
        let name = self
            .expect_identifier("ClockDomain name")?
            .text()
            .to_owned();
        self.expect(TokenKind::Equal, "`=` before ClockDomain definition")?;
        self.expect_keyword("periodic")?;
        self.expect(TokenKind::LeftParen, "`(` after `periodic`")?;
        self.expect_keyword("period")?;
        self.expect(TokenKind::Equal, "`=` after `period`")?;
        let period = self.parse_rational()?;
        self.expect(TokenKind::Comma, "`,` between period and phase")?;
        self.expect_keyword("phase")?;
        self.expect(TokenKind::Equal, "`=` after `phase`")?;
        let phase = self.parse_rational()?;
        self.expect(TokenKind::RightParen, "`)` after periodic clock")?;
        let end = self
            .expect(TokenKind::Semicolon, "`;` after ClockDomain")?
            .range()
            .end();
        Some(ClockDecl {
            comments: Default::default(),
            name,
            period,
            phase,
            range: TextRange::new(start, end),
        })
    }

    fn parse_connection(&mut self, allow_family: bool) -> Option<ParsedConnection> {
        let start = self.expect_keyword("connect")?.range().start();
        if self.at_keyword("signal") {
            self.bump();
            let mut ports = vec![self.parse_name_path("signal output Port")?];
            self.expect(TokenKind::Arrow, "`->` after signal output")?;
            ports.extend(self.parse_name_path_list("signal input Port")?);
            if ports.len() < 2 {
                self.error_here("Connection requires at least two Ports");
            }
            let end = self
                .expect(TokenKind::Semicolon, "`;` after Connection")?
                .range()
                .end();
            return Some(ParsedConnection::Ordinary(ConnectionDecl {
                comments: Default::default(),
                syntax: ConnectionSyntax::Signal,
                ports,
                range: TextRange::new(start, end),
            }));
        }
        let syntax = if self.at_keyword("conserving") {
            ConnectionSyntax::Conserving
        } else if self.at_keyword("periodic") {
            ConnectionSyntax::SpatialPeriodic
        } else {
            self.error_here("expected `signal`, `conserving`, or `periodic` after `connect`");
            return None;
        };
        self.bump();
        if syntax == ConnectionSyntax::SpatialPeriodic && allow_family {
            self.error_here("spatial-periodic Connections are allowed only in closed Models");
            return None;
        }
        let binder = if self.at(TokenKind::LeftBracket) {
            if syntax == ConnectionSyntax::SpatialPeriodic {
                self.error_here("spatial-periodic Connections cannot declare a family binder");
                return None;
            }
            if !allow_family {
                self.error_here("boundary family binders are allowed only in Components");
                return None;
            }
            Some(self.parse_boundary_family_binder()?)
        } else {
            None
        };
        let ports = self.parse_boundary_port_reference_list("boundary Port")?;
        if syntax == ConnectionSyntax::SpatialPeriodic && ports.len() != 2 {
            self.error_here("spatial-periodic Connection requires exactly two Ports");
        } else if ports.len() < 2 {
            self.error_here("Connection requires at least two Ports");
        }
        let end = self
            .expect(TokenKind::Semicolon, "`;` after Connection")?
            .range()
            .end();
        if syntax == ConnectionSyntax::Conserving
            && binder.is_none()
            && ports.iter().all(|port| port.selector().is_none())
        {
            return Some(ParsedConnection::Ordinary(ConnectionDecl {
                comments: Default::default(),
                syntax,
                ports: ports.into_iter().map(|port| port.port).collect(),
                range: TextRange::new(start, end),
            }));
        }
        Some(ParsedConnection::Boundary(BoundaryConnectionDecl {
            comments: Default::default(),
            syntax,
            binder,
            ports,
            range: TextRange::new(start, end),
        }))
    }

    fn parse_boundary(&mut self) -> Option<BoundaryDecl> {
        let start = self.expect_keyword("boundary")?.range().start();
        let ports = self.parse_name_path_list("boundary Port")?;
        let end = self
            .expect(TokenKind::Semicolon, "`;` after boundary")?
            .range()
            .end();
        Some(BoundaryDecl {
            comments: Default::default(),
            ports,
            range: TextRange::new(start, end),
        })
    }

    fn parse_name_path_list(&mut self, expected: &str) -> Option<Vec<NamePath>> {
        let mut names = vec![self.parse_name_path(expected)?];
        while self.at(TokenKind::Comma) {
            self.bump();
            names.push(self.parse_name_path(expected)?);
        }
        Some(names)
    }

    fn parse_boundary_family_binder(&mut self) -> Option<BoundaryFamilyBinderSyntax> {
        let start = self
            .expect(TokenKind::LeftBracket, "`[` before boundary family binder")?
            .range()
            .start();
        let member = self
            .expect_identifier("boundary family member name")?
            .text()
            .to_owned();
        self.expect_keyword("in")?;
        let set = self
            .expect_identifier("complete exterior support-set name")?
            .text()
            .to_owned();
        let end = self
            .expect(TokenKind::RightBracket, "`]` after boundary family binder")?
            .range()
            .end();
        Some(BoundaryFamilyBinderSyntax {
            member,
            set,
            range: TextRange::new(start, end),
        })
    }

    fn parse_boundary_port_selector(&mut self) -> Option<BoundaryPortSelectorSyntax> {
        let start = self
            .expect(TokenKind::LeftBracket, "`[` before boundary Port selector")?
            .range()
            .start();
        let member = self
            .expect_identifier("boundary family member name")?
            .text()
            .to_owned();
        self.expect(TokenKind::Equal, "`=` in boundary Port selector")?;
        let target = self
            .expect_identifier("boundary selector target")?
            .text()
            .to_owned();
        let end = self
            .expect(TokenKind::RightBracket, "`]` after boundary Port selector")?
            .range()
            .end();
        Some(BoundaryPortSelectorSyntax {
            member,
            target,
            range: TextRange::new(start, end),
        })
    }

    fn parse_boundary_port_reference(
        &mut self,
        expected: &str,
    ) -> Option<BoundaryPortReferenceSyntax> {
        let port = self.parse_name_path(expected)?;
        let selector = self
            .at(TokenKind::LeftBracket)
            .then(|| self.parse_boundary_port_selector())
            .flatten();
        Some(BoundaryPortReferenceSyntax { port, selector })
    }

    fn parse_boundary_port_reference_list(
        &mut self,
        expected: &str,
    ) -> Option<Vec<BoundaryPortReferenceSyntax>> {
        let mut ports = vec![self.parse_boundary_port_reference(expected)?];
        while self.at(TokenKind::Comma) {
            self.bump();
            ports.push(self.parse_boundary_port_reference(expected)?);
        }
        Some(ports)
    }

    fn parse_name_path(&mut self, expected: &str) -> Option<NamePath> {
        let first = self.expect_identifier(expected)?;
        self.parse_name_path_from_first(first, expected)
    }

    fn parse_name_path_from_first(&mut self, first: Token, expected: &str) -> Option<NamePath> {
        let start = first.range().start();
        let mut end = first.range().end();
        let mut segments = vec![first.text().to_owned()];
        while self.at(TokenKind::Dot) {
            self.bump();
            let segment = self.expect_identifier(expected)?;
            end = segment.range().end();
            segments.push(segment.text().to_owned());
        }
        Some(NamePath::from_parsed_segments(
            segments,
            TextRange::new(start, end),
        ))
    }

    fn parse_rational(&mut self) -> Option<RationalSyntax> {
        let numerator = self.parse_u64("rational numerator")?;
        self.expect(TokenKind::Slash, "`/` in rational model time")?;
        let denominator = self.parse_u64("rational denominator")?;
        Some(RationalSyntax {
            numerator,
            denominator,
        })
    }

    fn parse_u64(&mut self, expected: &str) -> Option<u64> {
        let token = self.expect(TokenKind::Number, expected)?;
        token.text().parse::<u64>().ok().or_else(|| {
            self.error_token(&token, format!("{expected} must be an unsigned integer"));
            None
        })
    }

    fn parse_signed_number(&mut self) -> Option<f64> {
        let negative = self.at(TokenKind::Minus);
        if negative {
            self.bump();
        }
        let token = self.expect(TokenKind::Number, "numeric literal")?;
        self.parse_f64(&token)
            .map(|value| if negative { -value } else { value })
    }

    fn parse_f64(&mut self, token: &Token) -> Option<f64> {
        match token.text().parse::<f64>() {
            Ok(value)
                if value == 0.0
                    && token
                        .text()
                        .split(['e', 'E'])
                        .next()
                        .is_some_and(|significand| {
                            significand.bytes().any(|byte| matches!(byte, b'1'..=b'9'))
                        }) =>
            {
                self.error_token(token, "nonzero numeric literal underflows the f64 range");
                None
            }
            Ok(value) if value.is_finite() => Some(value),
            _ => {
                self.error_token(token, "numeric literal must be a finite f64 value");
                None
            }
        }
    }

    fn recover_item(&mut self) {
        while !self.at(TokenKind::Eof) && !self.at(TokenKind::RightBrace) {
            if self.at(TokenKind::Semicolon) {
                self.bump();
                return;
            }
            self.bump();
        }
    }

    fn recover_top_level(&mut self) {
        while !self.at(TokenKind::Eof)
            && !self.at_keyword("property")
            && !self.at_keyword("connector")
            && !self.at_keyword("component")
            && !self.at_keyword("pure")
            && !self.at_keyword("model")
        {
            self.bump();
        }
    }

    fn at_keyword(&mut self, keyword: &str) -> bool {
        self.current().kind() == TokenKind::Identifier && self.current().text() == keyword
    }

    fn expect_keyword(&mut self, keyword: &str) -> Option<Token> {
        if self.at_keyword(keyword) {
            Some(self.bump())
        } else {
            self.error_here(format!("expected `{keyword}`"));
            None
        }
    }

    fn expect_identifier(&mut self, expected: &str) -> Option<Token> {
        self.expect(TokenKind::Identifier, expected)
    }

    fn expect(&mut self, kind: TokenKind, expected: &str) -> Option<Token> {
        if self.at(kind) {
            Some(self.bump())
        } else {
            self.error_here(format!("expected {expected}"));
            None
        }
    }

    fn at(&mut self, kind: TokenKind) -> bool {
        self.current().kind() == kind
    }

    fn current(&mut self) -> &Token {
        self.skip_trivia();
        &self.tokens[self.cursor.min(self.tokens.len() - 1)]
    }

    fn bump(&mut self) -> Token {
        self.skip_trivia();
        let token = self.tokens[self.cursor.min(self.tokens.len() - 1)].clone();
        if token.kind() != TokenKind::Eof {
            self.cursor += 1;
        }
        token
    }

    fn skip_trivia(&mut self) {
        while self
            .tokens
            .get(self.cursor)
            .is_some_and(|token| token.kind().is_trivia())
        {
            self.cursor += 1;
        }
    }

    fn error_here(&mut self, message: impl Into<String>) {
        let token = self.current().clone();
        self.error_token(&token, message);
    }

    fn error_previous(&mut self, message: impl Into<String>) {
        let token = self.tokens[self.cursor.saturating_sub(1)].clone();
        self.error_token(&token, message);
    }

    fn error_token(&mut self, token: &Token, message: impl Into<String>) {
        self.diagnostics.push(
            Diagnostic::error(codes::SYNTAX_ERROR, message).with_span(Span {
                file: self.file.clone(),
                start: token.range().start(),
                end: token.range().end(),
            }),
        );
    }
}

#[cfg(test)]
mod tests;
