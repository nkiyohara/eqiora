//! Recovering recursive-descent parser for the deliberately small v0 grammar.
use eqiora_core::diagnostic::codes;
use eqiora_core::{Diagnostic, Span};

mod compile_time;
mod document;
mod domain;
mod formulation;
mod instance;
mod property;
mod relation;

use crate::ast::{
    BinaryOp, BoundaryConnectionDecl, BoundaryDecl, BoundaryFamilyBinderSyntax,
    BoundaryPairingSyntax, BoundaryPortReferenceSyntax, BoundaryPortSelectorSyntax,
    BoundarySetBindingDecl, BoundarySetMemberSyntax, BoundarySideSyntax, ClockDecl, ComponentItem,
    ComponentParameterDecl, ComponentPortDecl, ComponentPortFamilyDecl, ConnectionDecl,
    ConnectionSyntax, ConnectorDecl, ConnectorQuantitySyntax, ConnectorSyntax, Document,
    DomainDecl, DomainSyntax, ExactIntegerSyntax, Expr, ExprKind, FieldBindingDecl, FieldDecl,
    FieldSlotDecl, FrameSyntax, InstanceDecl, Item, NamePath, ParameterBindingDecl, PortDecl,
    PortSyntax, PureOperatorBinaryOp, PureOperatorDecl, PureOperatorExpr, PureOperatorExprKind,
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
        diagnostics: Vec::new(),
    };
    let document = parser.parse_document();
    diagnostics.extend(parser.diagnostics);
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
    fn parse_pure_operator(
        &mut self,
        start: u32,
        visibility: VisibilitySyntax,
    ) -> Option<PureOperatorDecl> {
        self.expect_keyword("pure")?;
        self.expect_keyword("operator")?;
        let name = self
            .expect_identifier("pure operator name")?
            .text()
            .to_owned();
        self.expect(TokenKind::LeftParen, "`(` after pure operator name")?;
        if self.at(TokenKind::RightParen) {
            self.error_here("pure operator requires at least one formal");
            return None;
        }
        let mut formals = vec![self.parse_pure_operator_formal()?];
        while self.at(TokenKind::Comma) {
            self.bump();
            formals.push(self.parse_pure_operator_formal()?);
        }
        self.expect(TokenKind::RightParen, "`)` after pure operator formals")?;
        self.expect(TokenKind::Arrow, "`->` before pure operator result")?;
        let result = self.parse_pure_value_class()?;
        self.expect(TokenKind::Equal, "`=` before pure operator body")?;
        let body = self.parse_pure_operator_expression(0)?;
        let end = self
            .expect(TokenKind::Semicolon, "`;` after pure operator")?
            .range()
            .end();
        Some(PureOperatorDecl {
            visibility,
            name,
            formals,
            result,
            body,
            range: TextRange::new(start, end),
        })
    }

    fn parse_pure_operator_formal(&mut self) -> Option<PureOperatorFormal> {
        let name = self.expect_identifier("pure operator formal")?;
        let start = name.range().start();
        self.expect(TokenKind::Colon, "`:` after pure operator formal")?;
        let value_class = self.parse_pure_value_class()?;
        let end = self.previous_significant_range().end();
        Some(PureOperatorFormal {
            name: name.text().to_owned(),
            value_class,
            range: TextRange::new(start, end),
        })
    }

    fn parse_pure_value_class(&mut self) -> Option<PureValueClassSyntax> {
        if self.at_keyword("scalar") {
            self.bump();
            Some(PureValueClassSyntax::Scalar)
        } else if self.at_keyword("spatial") {
            self.bump();
            self.expect(TokenKind::LeftBracket, "`[` after `spatial`")?;
            let rank = self.parse_exact_integer("spatial rank")?;
            self.expect(TokenKind::RightBracket, "`]` after spatial rank")?;
            Some(PureValueClassSyntax::Spatial { rank })
        } else {
            self.error_here("expected `scalar` or `spatial[rank]`");
            None
        }
    }

    fn parse_pure_operator_expression(
        &mut self,
        minimum_binding_power: u8,
    ) -> Option<PureOperatorExpr> {
        let mut left = if self.at(TokenKind::Minus) {
            let start = self.bump().range().start();
            let value = self.parse_pure_operator_expression(5)?;
            PureOperatorExpr {
                range: TextRange::new(start, value.range.end()),
                kind: PureOperatorExprKind::Neg(Box::new(value)),
            }
        } else if self.at_keyword("rational") {
            let start = self.bump().range().start();
            self.expect(TokenKind::LeftParen, "`(` after `rational`")?;
            let numerator = self.parse_exact_integer("rational numerator")?;
            self.expect(TokenKind::Comma, "`,` between rational integers")?;
            let denominator = self.parse_exact_integer("rational denominator")?;
            if denominator.value == 0 {
                let token = self.tokens[self.cursor.saturating_sub(1)].clone();
                self.error_token(&token, "rational denominator must be nonzero");
                return None;
            }
            let end = self
                .expect(TokenKind::RightParen, "`)` after rational literal")?
                .range()
                .end();
            PureOperatorExpr {
                kind: PureOperatorExprKind::Rational {
                    numerator,
                    denominator,
                },
                range: TextRange::new(start, end),
            }
        } else if self.at_keyword("component") {
            let start = self.bump().range().start();
            self.expect(TokenKind::LeftParen, "`(` after `component`")?;
            let formal = self.expect_identifier("component formal")?;
            let mut result_axes = Vec::new();
            while self.at(TokenKind::Comma) {
                self.bump();
                result_axes.push(self.parse_exact_integer("component result axis")?);
            }
            let end = self
                .expect(TokenKind::RightParen, "`)` after component selection")?
                .range()
                .end();
            PureOperatorExpr {
                kind: PureOperatorExprKind::Component {
                    formal: formal.text().to_owned(),
                    formal_range: formal.range(),
                    result_axes,
                },
                range: TextRange::new(start, end),
            }
        } else if self.at_keyword("delta") {
            let start = self.bump().range().start();
            self.expect(TokenKind::LeftParen, "`(` after `delta`")?;
            let left_axis = self.parse_exact_integer("delta left axis")?;
            self.expect(TokenKind::Comma, "`,` between delta axes")?;
            let right_axis = self.parse_exact_integer("delta right axis")?;
            let end = self
                .expect(TokenKind::RightParen, "`)` after delta axes")?
                .range()
                .end();
            PureOperatorExpr {
                kind: PureOperatorExprKind::Delta {
                    left_axis,
                    right_axis,
                },
                range: TextRange::new(start, end),
            }
        } else if self.at(TokenKind::LeftParen) {
            let start = self.bump().range().start();
            let mut expression = self.parse_pure_operator_expression(0)?;
            let end = self
                .expect(TokenKind::RightParen, "`)` after pure operator expression")?
                .range()
                .end();
            expression.range = TextRange::new(start, end);
            expression
        } else {
            self.error_here(
                "expected `rational`, `component`, `delta`, or parenthesized pure operator expression",
            );
            return None;
        };

        loop {
            let (operator, left_power, right_power) = match self.current().kind() {
                TokenKind::Plus => (PureOperatorBinaryOp::Add, 1, 2),
                TokenKind::Minus => (PureOperatorBinaryOp::Sub, 1, 2),
                TokenKind::Star => (PureOperatorBinaryOp::Mul, 3, 4),
                _ => break,
            };
            if left_power < minimum_binding_power {
                break;
            }
            self.bump();
            let right = self.parse_pure_operator_expression(right_power)?;
            let range = TextRange::new(left.range.start(), right.range.end());
            left = PureOperatorExpr {
                kind: PureOperatorExprKind::Binary {
                    op: operator,
                    left: Box::new(left),
                    right: Box::new(right),
                },
                range,
            };
        }
        Some(left)
    }

    fn parse_exact_integer(&mut self, expected: &str) -> Option<ExactIntegerSyntax> {
        let token = self.expect(TokenKind::Number, expected)?;
        match token.text().parse::<u64>() {
            Ok(value) => Some(ExactIntegerSyntax {
                spelling: token.text().to_owned(),
                value,
                range: token.range(),
            }),
            Err(_) => {
                self.error_token(
                    &token,
                    format!("{expected} must be an exact unsigned integer"),
                );
                None
            }
        }
    }

    fn parse_connector(
        &mut self,
        start: u32,
        visibility: VisibilitySyntax,
    ) -> Option<ConnectorDecl> {
        self.expect_keyword("connector")?;
        let name = self.expect_identifier("Connector name")?.text().to_owned();
        self.expect(TokenKind::Equal, "`=` before Connector family")?;
        let syntax = if self.at_keyword("scalar_physical") {
            let (across_dimension, through_dimension) = self.parse_scalar_physical_dimensions()?;
            ConnectorSyntax::ScalarPhysical {
                across_dimension,
                through_dimension,
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
        if self.at_keyword("support") {
            return self
                .parse_support_slot(start, visibility)
                .map(ComponentItem::Support)
                .map(Box::new)
                .map(ParsedComponentItem::Retained);
        }
        if public && self.at_field_slot_declaration() {
            return self
                .parse_field_slot(start)
                .map(ComponentItem::FieldSlot)
                .map(Box::new)
                .map(ParsedComponentItem::Retained);
        }

        if !public && self.at_field_slot_declaration() {
            let token = self.current().clone();
            self.error_token(&token, "`field slot` declarations must be public in v1");
            self.parse_field_slot(start)?;
            return Some(ParsedComponentItem::Discarded);
        }

        if public {
            let token = self.current().clone();
            self.error_token(
                &token,
                "only scalar `parameter`, `port`, `support`, and `field slot` declarations may be public",
            );
            if self.at_keyword("representation") {
                self.parse_representation()?;
            } else if self.at_keyword("field") {
                self.parse_field()?;
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
        } else if self.at_keyword("field") {
            self.parse_field().map(ComponentItem::Field)
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
        } else if self.at_keyword("field") {
            self.parse_field().map(Item::Field)
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

    fn parse_scalar_physical_dimensions(&mut self) -> Option<(Expr, Expr)> {
        self.expect_keyword("scalar_physical")?;
        self.expect(TokenKind::LeftParen, "`(` after `scalar_physical`")?;
        self.expect_keyword("across")?;
        self.expect(TokenKind::Equal, "`=` after `across`")?;
        let across_dimension = self.parse_expression(0)?;
        self.expect(TokenKind::Comma, "`,` between physical dimensions")?;
        self.expect_keyword("through")?;
        self.expect(TokenKind::Equal, "`=` after `through`")?;
        let through_dimension = self.parse_expression(0)?;
        self.expect(TokenKind::RightParen, "`)` after physical dimensions")?;
        Some((across_dimension, through_dimension))
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
            dimension: self.parse_expression(0)?,
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
            name,
            syntax: RepresentationSyntax::Continuum,
            range: TextRange::new(start, end),
        })
    }

    fn parse_field(&mut self) -> Option<FieldDecl> {
        let start = self.expect_keyword("field")?.range().start();
        let name = self
            .expect_identifier("declaration name")?
            .text()
            .to_owned();
        let (domain, representation) = if self.at_keyword("on") {
            self.bump();
            let domain = self.expect_identifier("Field Domain")?.text().to_owned();
            self.expect_keyword("as")?;
            let representation = self
                .expect_identifier("Field Representation")?
                .text()
                .to_owned();
            (Some(domain), Some(representation))
        } else {
            (None, None)
        };
        self.expect(TokenKind::Colon, "`:` before dimension")?;
        let dimension = self.parse_expression(0)?;
        let shape = if self.at_keyword("shape") {
            self.bump();
            Some(self.parse_value_shape()?)
        } else {
            None
        };
        let scalar = shape.as_ref().is_none_or(|shape| {
            matches!(shape, ValueShapeSyntax::Scalar)
                || matches!(shape, ValueShapeSyntax::Exact(extents) if extents.is_empty())
        });
        let initial = if scalar && self.at(TokenKind::Equal) {
            self.bump();
            Some(self.parse_signed_number()?)
        } else if self.at(TokenKind::Equal) {
            self.error_here(
                "non-scalar Field cannot have a scalar initial value; omit `=` until shaped values are supported",
            );
            self.bump();
            let _ = self.parse_signed_number()?;
            None
        } else {
            None
        };
        let end = self
            .expect(TokenKind::Semicolon, "`;` after declaration")?
            .range()
            .end();
        Some(FieldDecl {
            name,
            domain,
            representation,
            shape,
            dimension,
            initial,
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
        self.expect(TokenKind::Colon, "`:` before component Parameter dimension")?;
        let dimension = self.parse_expression(0)?;
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
            visibility,
            name,
            dimension,
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
        let end = self
            .expect(TokenKind::Semicolon, "`;` after support slot")?
            .range()
            .end();
        Some(SupportSlotDecl {
            visibility,
            name,
            syntax,
            range: TextRange::new(start, end),
        })
    }

    fn parse_field_slot(&mut self, start: u32) -> Option<FieldSlotDecl> {
        self.expect_keyword("field")?;
        self.expect_keyword("slot")?;
        let name = self
            .expect_identifier("component Field-slot name")?
            .text()
            .to_owned();
        self.expect_keyword("on")?;
        let support = self
            .expect_identifier("Field-slot support name")?
            .text()
            .to_owned();
        self.expect_keyword("as")?;
        self.expect_keyword("continuum")?;
        self.expect(TokenKind::Colon, "`:` before Field-slot dimension")?;
        let dimension = self.parse_expression(0)?;
        let shape = if self.at_keyword("shape") {
            self.bump();
            Some(self.parse_value_shape()?)
        } else {
            None
        };
        let end = self
            .expect(TokenKind::Semicolon, "`;` after Field slot")?
            .range()
            .end();
        Some(FieldSlotDecl {
            name,
            support,
            dimension,
            shape,
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
                let checkpoint = self.cursor;
                let diagnostic_checkpoint = self.diagnostics.len();
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
                        self.cursor = checkpoint;
                        self.diagnostics.truncate(diagnostic_checkpoint);
                        PortSyntax::ConservingMarker {
                            dimension: self.parse_expression(0)?,
                        }
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
            dimension: self.parse_expression(0)?,
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
                syntax,
                ports: ports.into_iter().map(|port| port.port).collect(),
                range: TextRange::new(start, end),
            }));
        }
        Some(ParsedConnection::Boundary(BoundaryConnectionDecl {
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

    fn parse_expression(&mut self, minimum_binding_power: u8) -> Option<Expr> {
        let mut left = if self.at(TokenKind::Minus) {
            let start = self.bump().range().start();
            let value = self.parse_expression(9)?;
            Expr {
                range: TextRange::new(start, value.range.end()),
                kind: ExprKind::Unary {
                    op: UnaryOp::Neg,
                    value: Box::new(value),
                },
            }
        } else if self.at(TokenKind::Number) {
            let token = self.bump();
            let value = self.parse_f64(&token)?;
            Expr {
                kind: ExprKind::Number(value),
                range: token.range(),
            }
        } else if self.at(TokenKind::Identifier) {
            let token = self.bump();
            let name = token.text().to_owned();
            let path = if self.at(TokenKind::Dot) {
                self.parse_name_path_from_first(token, "qualified name segment")?
            } else {
                NamePath::single(name, token.range())
            };
            if self.at(TokenKind::LeftParen) {
                self.bump();
                if self.at(TokenKind::RightParen) {
                    self.error_here("operator call requires at least one argument");
                    return None;
                }
                let mut arguments = vec![self.parse_expression(0)?];
                while self.at(TokenKind::Comma) {
                    self.bump();
                    arguments.push(self.parse_expression(0)?);
                }
                let end = self
                    .expect(TokenKind::RightParen, "`)` after operator arguments")?
                    .range()
                    .end();
                Expr {
                    kind: ExprKind::Call {
                        callee: path.clone(),
                        arguments,
                    },
                    range: TextRange::new(path.range().start(), end),
                }
            } else if self.at(TokenKind::LeftBracket) {
                let selector = self.parse_boundary_port_selector()?;
                let range = TextRange::new(path.range().start(), selector.range().end());
                Expr {
                    kind: ExprKind::BoundaryPortSelection {
                        port: Box::new(path),
                        selector: Box::new(selector),
                    },
                    range,
                }
            } else {
                let range = path.range();
                let kind = if path.is_qualified() {
                    ExprKind::Path(path)
                } else {
                    ExprKind::Name(path.as_str().to_owned())
                };
                Expr { kind, range }
            }
        } else if self.at(TokenKind::LeftParen) {
            let start = self.bump().range().start();
            let mut expression = self.parse_expression(0)?;
            let end = self
                .expect(TokenKind::RightParen, "`)` after expression")?
                .range()
                .end();
            expression.range = TextRange::new(start, end);
            expression
        } else {
            self.error_here("expected expression");
            return None;
        };

        loop {
            let (operator, left_power, right_power) = match self.current().kind() {
                TokenKind::Plus => (BinaryOp::Add, 1, 2),
                TokenKind::Minus => (BinaryOp::Sub, 1, 2),
                TokenKind::Star => (BinaryOp::Mul, 3, 4),
                TokenKind::Slash => (BinaryOp::Div, 3, 4),
                TokenKind::Caret => (BinaryOp::Pow, 7, 6),
                _ => break,
            };
            if left_power < minimum_binding_power {
                break;
            }
            self.bump();
            let right = self.parse_expression(right_power)?;
            let range = TextRange::new(left.range.start(), right.range.end());
            left = Expr {
                kind: ExprKind::Binary {
                    op: operator,
                    left: Box::new(left),
                    right: Box::new(right),
                },
                range,
            };
        }
        Some(left)
    }

    fn parse_f64(&mut self, token: &Token) -> Option<f64> {
        match token.text().parse::<f64>() {
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
mod tests {
    use super::*;

    #[test]
    fn parser_retains_exact_pure_operator_syntax_and_qualified_applications() {
        let source = r#"public pure operator dyadic(left: spatial[01], right: spatial[1]) -> spatial[2] =
  component(left, 0) * component(right, 1) + rational(03, 4) * delta(0, 1);

model coupled {
  field u: 1 = 0;
  field v: 1 = 0;
  relation law continuous { ops.dyadic(u, v) = 0; }
}"#;
        let document = parse("pure-operator.eqi", source)
            .into_document()
            .expect("bounded pure operator source");

        let operator = &document.pure_operators()[0];
        assert_eq!(operator.visibility(), VisibilitySyntax::Public);
        assert_eq!(operator.name(), "dyadic");
        assert_eq!(operator.formals().len(), 2);
        assert_eq!(operator.formals()[0].name(), "left");
        let PureValueClassSyntax::Spatial { rank } = operator.formals()[0].value_class() else {
            panic!("first formal is spatial");
        };
        assert_eq!(rank.value(), 1);
        assert_eq!(rank.spelling(), "01");
        assert_eq!(
            &source[rank.range().start() as usize..rank.range().end() as usize],
            "01"
        );
        assert_eq!(
            &source[operator.range().start() as usize..operator.range().end() as usize],
            &source[..source.find("\n\nmodel").expect("model separator")]
        );

        let PureOperatorExprKind::Binary { left, right, .. } = operator.body().kind() else {
            panic!("addition is the pure body root");
        };
        assert!(matches!(
            left.kind(),
            PureOperatorExprKind::Binary {
                op: PureOperatorBinaryOp::Mul,
                ..
            }
        ));
        let PureOperatorExprKind::Binary {
            op: PureOperatorBinaryOp::Mul,
            left: rational,
            right: delta,
        } = right.kind()
        else {
            panic!("right term retains exact rational and delta nodes");
        };
        let PureOperatorExprKind::Rational { numerator, .. } = rational.kind() else {
            panic!("left factor is rational");
        };
        assert_eq!(numerator.spelling(), "03");
        assert!(matches!(delta.kind(), PureOperatorExprKind::Delta { .. }));

        let Item::Relation(relation) = &document.models()[0].items()[2] else {
            panic!("model relation retained");
        };
        let ExprKind::Call { callee, arguments } = relation.residuals()[0].kind() else {
            panic!("residual is a qualified application");
        };
        assert_eq!(callee.segments().collect::<Vec<_>>(), ["ops", "dyadic"]);
        assert_eq!(arguments.len(), 2);
        assert!(matches!(arguments[0].kind(), ExprKind::Name(name) if name == "u"));
        assert!(matches!(arguments[1].kind(), ExprKind::Name(name) if name == "v"));
    }

    #[test]
    fn parser_rejects_operators_outside_the_exact_body_vocabulary() {
        for source in [
            "pure operator bad(x: scalar) -> scalar = rational(1.0, 2);",
            "pure operator bad(x: scalar) -> scalar = rational(1, 0);",
            "pure operator bad(x: scalar) -> scalar = rational(1, 2) / rational(3, 4);",
            "pure operator bad(x: scalar) -> scalar = x;",
            "pure operator bad() -> scalar = rational(1, 2);",
        ] {
            assert!(
                parse("invalid-pure-operator.eqi", source)
                    .into_document()
                    .is_err(),
                "source must fail closed: {source}"
            );
        }
    }

    #[test]
    fn parser_represents_scalar_component_selection_with_zero_axes() {
        let source = "pure operator negate(s: scalar) -> scalar = -component(s);";
        let document = parse("scalar-component.eqi", source)
            .into_document()
            .expect("scalar component selection");
        let PureOperatorExprKind::Neg(value) = document.pure_operators()[0].body().kind() else {
            panic!("negation retained");
        };
        assert!(matches!(
            value.kind(),
            PureOperatorExprKind::Component { result_axes, .. } if result_axes.is_empty()
        ));
    }

    #[test]
    fn parser_requires_pure_operators_before_models_and_nonempty_calls() {
        let late = parse(
            "late-operator.eqi",
            "model M {} pure operator identity(x: scalar) -> scalar = component(x, 0);",
        );
        assert!(late.into_document().is_err());

        let empty_call = parse(
            "empty-call.eqi",
            "model M { relation r continuous { ops.identity() = 0; } }",
        );
        assert!(empty_call.into_document().is_err());
    }

    #[test]
    fn parser_builds_continuous_and_periodic_relations() {
        let source = r#"
model thermal {
  field temperature: K = 293;
  field command: 1 = 0;
  clock control = periodic(period = 1 / 10, phase = 0 / 1);
  relation plant continuous {
    derivative(temperature) - command = 0;
  }
  relation controller periodic(control) {
    next(command) - pre(command) = 0;
  }
}
"#;
        let result = parse("thermal.eqi", source);
        let document = result.into_document().expect("valid source");

        assert_eq!(document.models().len(), 1);
        assert_eq!(document.models()[0].items().len(), 5);
    }

    #[test]
    fn parser_recovers_after_an_invalid_item() {
        let source = "model m { nonsense; field x: 1 = 0; }";
        let result = parse("recovery.eqi", source);

        assert!(!result.diagnostics().is_empty());
        assert_eq!(
            result.document().expect("recovered").models()[0]
                .items()
                .len(),
            1
        );
    }

    #[test]
    fn parser_retains_scalar_physical_contracts_and_source_ranges() {
        let source = r#"model circuit {
  domain electrical = scalar_physical(across = kg * m ^ 2 / (s ^ 3 * A), through = A);
  port terminal: conserving on electrical;
  relation component continuous { across(terminal) = 0; }
}"#;
        let document = parse("circuit.eqi", source)
            .into_document()
            .expect("valid physical source");
        let items = document.models()[0].items();

        let Item::Domain(domain) = &items[0] else {
            panic!("first item is the physical Domain");
        };
        let DomainSyntax::ScalarPhysical {
            across_dimension,
            through_dimension,
        } = domain.syntax()
        else {
            panic!("Domain retains the scalar physical contract");
        };
        assert_eq!(
            &source[domain.range().start() as usize..domain.range().end() as usize],
            "domain electrical = scalar_physical(across = kg * m ^ 2 / (s ^ 3 * A), through = A);"
        );
        assert_eq!(
            &source[across_dimension.range().start() as usize
                ..across_dimension.range().end() as usize],
            "kg * m ^ 2 / (s ^ 3 * A)"
        );
        assert_eq!(
            &source[through_dimension.range().start() as usize
                ..through_dimension.range().end() as usize],
            "A"
        );

        let Item::Port(port) = &items[1] else {
            panic!("second item is the physical Port");
        };
        assert!(matches!(
            port.syntax(),
            PortSyntax::ScalarPhysical { domain } if domain == "electrical"
        ));
        assert_eq!(
            &source[port.range().start() as usize..port.range().end() as usize],
            "port terminal: conserving on electrical;"
        );
    }

    #[test]
    fn parser_builds_typed_component_interfaces_instances_and_paths() {
        let source = r#"
connector Pin = scalar_physical(across = kg * m ^ 2 / (s ^ 3 * A), through = A);

component Pair {
  public parameter resistance: kg * m ^ 2 / (s ^ 3 * A ^ 2);
  parameter scale: 1 = 2;
  public port positive: conserving on Pin;
  port command: signal input 1;
  instance inner: Catalog.Resistor(resistance = resistance * scale);
  relation law continuous { across(inner.positive) - resistance = 0; }
  connect conserving inner.positive, positive;
}

model parallel {
  instance r2: Pair(resistance = 2);
  instance r4: Pair(resistance = 4);
  connect conserving r2.positive, r4.positive;
  boundary r2.positive;
}
"#;
        let document = parse("components.eqi", source)
            .into_document()
            .expect("component syntax is valid");

        assert_eq!(document.connectors().len(), 1);
        assert_eq!(document.connectors()[0].name(), "Pin");
        assert!(matches!(
            document.connectors()[0].syntax(),
            ConnectorSyntax::ScalarPhysical { .. }
        ));

        let component = &document.components()[0];
        assert_eq!(component.name(), "Pair");
        let ComponentItem::Parameter(resistance) = &component.items()[0] else {
            panic!("first member is the public Parameter");
        };
        assert_eq!(resistance.visibility(), VisibilitySyntax::Public);
        assert!(resistance.default().is_none());
        let ComponentItem::Parameter(scale) = &component.items()[1] else {
            panic!("second member is the private Parameter");
        };
        assert_eq!(scale.visibility(), VisibilitySyntax::Private);
        assert!(scale.default().is_some());
        let ComponentItem::Port(positive) = &component.items()[2] else {
            panic!("third member is the public Port");
        };
        assert_eq!(positive.visibility(), VisibilitySyntax::Public);
        let PortSyntax::ScalarPhysicalConnector { connector } = positive.syntax() else {
            panic!("physical component Ports retain nominal Connector syntax");
        };
        assert_eq!(connector.segments().collect::<Vec<_>>(), ["Pin"]);

        let ComponentItem::Instance(inner) = &component.items()[4] else {
            panic!("fifth member is the nested instance");
        };
        assert_eq!(
            inner.definition().segments().collect::<Vec<_>>(),
            ["Catalog", "Resistor"]
        );
        assert_eq!(inner.bindings()[0].parameter(), "resistance");

        let ComponentItem::Relation(relation) = &component.items()[5] else {
            panic!("sixth member is the Relation");
        };
        let ExprKind::Binary { left, .. } = relation.residuals()[0].kind() else {
            panic!("Relation retains its subtraction");
        };
        let ExprKind::Call { arguments, .. } = left.kind() else {
            panic!("left side is across(...)");
        };
        let ExprKind::Path(path) = arguments[0].kind() else {
            panic!("instance Port selection is a structured path");
        };
        assert_eq!(path.segments().collect::<Vec<_>>(), ["inner", "positive"]);
        assert_eq!(
            &source[path.range().start() as usize..path.range().end() as usize],
            "inner.positive"
        );

        let Item::Connection(connection) = &document.models()[0].items()[2] else {
            panic!("third model member is a Connection");
        };
        assert_eq!(
            connection.port_paths()[0].segments().collect::<Vec<_>>(),
            ["r2", "positive"]
        );
        let Item::Boundary(boundary) = &document.models()[0].items()[3] else {
            panic!("fourth model member is the boundary");
        };
        assert!(boundary.port_paths()[0].is_qualified());
    }

    #[test]
    fn parser_retains_component_support_slots_representations_and_mixed_bindings() {
        let source = r#"component BoundaryState {
  public support body: volume(ambient_dimension = 2);
  public support interface: boundary(parent = body);
  representation state_space = continuum;
  field state on body as state_space: 1 = 0;
}

model coupled {
  domain fluid = box(0, 1, 0, 1);
  domain wall = boundary(fluid, axis = 0, side = lower);
  instance probe: BoundaryState(gain = 2, support body = fluid, support interface = wall);
}"#;
        let document = parse("supports.eqi", source)
            .into_document()
            .expect("support-slot syntax is valid");
        let component = &document.components()[0];

        let ComponentItem::Support(body) = &component.items()[0] else {
            panic!("first member is the volume support slot");
        };
        assert_eq!(body.visibility(), VisibilitySyntax::Public);
        assert!(matches!(
            body.syntax(),
            SupportSlotSyntax::Volume {
                ambient_dimension: 2
            }
        ));
        assert_eq!(
            &source[body.range().start() as usize..body.range().end() as usize],
            "public support body: volume(ambient_dimension = 2);"
        );

        let ComponentItem::Support(interface) = &component.items()[1] else {
            panic!("second member is the boundary support slot");
        };
        assert!(matches!(
            interface.syntax(),
            SupportSlotSyntax::Boundary { parent } if parent == "body"
        ));
        let ComponentItem::Representation(representation) = &component.items()[2] else {
            panic!("third member is the private Representation");
        };
        assert_eq!(representation.name(), "state_space");

        let Item::Instance(instance) = &document.models()[0].items()[2] else {
            panic!("third model member is the component instance");
        };
        assert_eq!(instance.bindings().len(), 1);
        assert_eq!(instance.bindings()[0].parameter(), "gain");
        assert_eq!(instance.support_bindings().len(), 2);
        assert_eq!(instance.support_bindings()[0].slot(), "body");
        assert_eq!(instance.support_bindings()[0].target(), "fluid");
        assert_eq!(instance.support_bindings()[1].slot(), "interface");
        assert_eq!(instance.support_bindings()[1].target(), "wall");
    }

    #[test]
    fn parser_retains_occurrence_bound_field_slots_and_bindings() {
        let source = r#"component IsotropicBalance2d {
  public support body: volume(ambient_dimension = 2);
  public field slot displacement on body as continuum: m shape spatial_vector;
  public field slot load on body as continuum: kg / (m * s ^ 2) shape spatial_vector;
  public parameter mu: kg / (m * s ^ 2);
}

model Main {
  domain body = box(0, 1, 0, 1);
  representation space = continuum;
  field u on body as space: m shape spatial_vector;
  field f on body as space: kg / (m * s ^ 2) shape spatial_vector;
  instance law: IsotropicBalance2d(mu = 3, support body = body, field displacement = u, field load = f);
}"#;
        let document = parse("field-slots.eqi", source)
            .into_document()
            .expect("Field-slot syntax is valid");
        let component = &document.components()[0];

        let ComponentItem::FieldSlot(displacement) = &component.items()[1] else {
            panic!("second member is the displacement Field slot");
        };
        assert_eq!(displacement.name(), "displacement");
        assert_eq!(displacement.support(), "body");
        assert_eq!(displacement.shape(), Some(&ValueShapeSyntax::SpatialVector));
        assert_eq!(
            &source[displacement.range().start() as usize..displacement.range().end() as usize],
            "public field slot displacement on body as continuum: m shape spatial_vector;"
        );

        let Item::Instance(instance) = &document.models()[0].items()[4] else {
            panic!("fifth model member is the component instance");
        };
        assert_eq!(instance.bindings().len(), 1);
        assert_eq!(instance.support_bindings().len(), 1);
        assert_eq!(instance.field_bindings().len(), 2);
        assert_eq!(instance.field_bindings()[0].slot(), "displacement");
        assert_eq!(instance.field_bindings()[0].target(), "u");
        assert_eq!(instance.field_bindings()[1].slot(), "load");
        assert_eq!(instance.field_bindings()[1].target(), "f");
        assert_eq!(crate::format(&document), format!("{source}\n"));
    }

    #[test]
    fn field_discriminator_does_not_reserve_the_parameter_name_field() {
        let document = parse(
            "field-parameter.eqi",
            "component C { public parameter field: 1; } model m { instance c: C(field = 1); }",
        )
        .into_document()
        .expect("`field = expression` remains a Parameter binding");
        let Item::Instance(instance) = &document.models()[0].items()[0] else {
            panic!("model member is an instance");
        };

        assert_eq!(instance.bindings()[0].parameter(), "field");
        assert!(instance.field_bindings().is_empty());
    }

    #[test]
    fn parser_rejects_non_public_or_non_continuum_field_slots() {
        let private = parse(
            "private-slot.eqi",
            "component C { field slot state on body as continuum: 1; }",
        );
        assert!(private.into_document().is_err());

        let discrete = parse(
            "discrete-slot.eqi",
            "component C { public field slot state on body as discrete: 1; }",
        );
        assert!(discrete.into_document().is_err());
    }

    #[test]
    fn support_discriminator_does_not_reserve_the_parameter_name_support() {
        let document = parse(
            "support-parameter.eqi",
            "component C { public parameter support: 1; } model m { instance c: C(support = 1); }",
        )
        .into_document()
        .expect("`support = expression` remains a Parameter binding");
        let Item::Instance(instance) = &document.models()[0].items()[0] else {
            panic!("model member is an instance");
        };

        assert_eq!(instance.bindings()[0].parameter(), "support");
        assert!(instance.support_bindings().is_empty());
    }

    #[test]
    fn parser_accepts_visibility_typed_declaration_only_documents() {
        let source = r#"public connector Pin = scalar_physical(across = 1, through = A);
private component Internal {}
public component Resistor {}"#;
        let document = parse("library.eqi", source)
            .into_document()
            .expect("library declarations parse without a Model");

        assert!(document.models().is_empty());
        assert_eq!(
            document.connectors()[0].visibility(),
            VisibilitySyntax::Public
        );
        assert_eq!(
            document.components()[0].visibility(),
            VisibilitySyntax::Private
        );
        assert_eq!(
            document.components()[1].visibility(),
            VisibilitySyntax::Public
        );
        assert_eq!(
            &source[document.connectors()[0].range().start() as usize
                ..document.connectors()[0].range().end() as usize],
            "public connector Pin = scalar_physical(across = 1, through = A);"
        );
        assert_eq!(
            &source[document.components()[0].range().start() as usize
                ..document.components()[0].range().end() as usize],
            "private component Internal {}"
        );
    }

    #[test]
    fn parser_retains_public_and_private_model_visibility() {
        let public = parse("entry.eqi", "public model Main {}")
            .into_document()
            .expect("public Model is accepted");
        assert_eq!(public.models().len(), 1);
        assert_eq!(public.models()[0].visibility(), VisibilitySyntax::Public);
        assert_eq!(public.models()[0].range(), TextRange::new(0, 20));
        assert_eq!(crate::format(&public), "public model Main {\n}\n");

        let private = parse("entry.eqi", "private model Main {}")
            .into_document()
            .expect("an explicitly package-local Model is accepted");
        assert_eq!(private.models()[0].visibility(), VisibilitySyntax::Private);
        assert_eq!(private.models()[0].name(), "Main");
    }

    #[test]
    fn parser_discards_illegal_public_members_and_recovers() {
        let source = r#"
component Invalid {
  public relation exposed continuous { 1 = 0; }
  public instance child: Other;
  instance malformed: ;
  parameter retained: 1 = 1;
}
model root {}
"#;
        let result = parse("visibility.eqi", source);
        let document = result.document().expect("recovered compilation unit");

        assert_eq!(result.diagnostics().len(), 3);
        assert_eq!(document.components()[0].items().len(), 1);
        assert!(matches!(
            document.components()[0].items()[0],
            ComponentItem::Parameter(_)
        ));
    }

    #[test]
    fn parser_requires_compilation_unit_definitions_before_models() {
        let result = parse(
            "order.eqi",
            "model first {} component Late {} model second {}",
        );
        let document = result.document().expect("declarations are recovered");

        assert_eq!(result.diagnostics().len(), 1);
        assert_eq!(document.components()[0].name(), "Late");
        assert_eq!(document.models().len(), 2);
    }

    #[test]
    fn parser_retains_field_physical_connector_shapes_ports_and_flux_access() {
        let source = r#"
public connector MechanicalBoundary = field_physical(
  pairing = euclidean_boundary_duality,
  flux = traction: kg / (m * s ^ 2),
  frame = spatial,
  trace = velocity: m / s,
  shape = spatial_vector
);

model coupled {
  domain fluid = box(0, 1, 0, 1);
  domain wall = boundary(fluid, axis = 0, side = upper);
  representation state_space = continuum;
  field velocity on fluid as state_space: m / s shape [2];
  port interface: conserving MechanicalBoundary over wall;
  relation balance continuous on wall { flux(interface) = 0; }
}
"#;
        let document = parse("field-physical.eqi", source)
            .into_document()
            .expect("field-valued boundary syntax parses");

        let ConnectorSyntax::FieldPhysical {
            trace,
            flux,
            shape,
            frame,
            pairing,
        } = document.connectors()[0].syntax()
        else {
            panic!("field-physical Connector retained");
        };
        assert_eq!(trace.name(), "velocity");
        assert_eq!(flux.name(), "traction");
        assert_eq!(shape, &ValueShapeSyntax::SpatialVector);
        assert_eq!(*frame, FrameSyntax::Spatial);
        assert_eq!(*pairing, BoundaryPairingSyntax::EuclideanBoundaryDuality);

        let Item::Field(field) = &document.models()[0].items()[3] else {
            panic!("fourth item is the shaped Field");
        };
        assert_eq!(field.shape(), Some(&ValueShapeSyntax::Exact(vec![2])));
        let Item::Port(port) = &document.models()[0].items()[4] else {
            panic!("fifth item is the boundary Port");
        };
        let PortSyntax::FieldPhysical { connector, support } = port.syntax() else {
            panic!("field-physical Port retained");
        };
        assert_eq!(connector.as_str(), "MechanicalBoundary");
        assert_eq!(support, "wall");
        let Item::Relation(relation) = &document.models()[0].items()[5] else {
            panic!("sixth item is the Relation");
        };
        assert!(matches!(
            relation.residuals()[0].kind(),
            ExprKind::Call { callee, .. } if callee.as_str() == "flux"
        ));
    }

    #[test]
    fn shaped_fields_never_desugar_a_scalar_initial_value() {
        let valid = parse(
            "shaped-field.eqi",
            "model M { field velocity: m / s shape [2]; }",
        );
        let document = valid.into_document().expect("shaped Field without initial");
        let Item::Field(field) = &document.models()[0].items()[0] else {
            panic!("fixture contains one Field");
        };
        assert_eq!(field.initial(), None);

        let invalid = parse(
            "broadcast.eqi",
            "model M { field velocity: m / s shape [2] = 0; }",
        );
        assert!(invalid.diagnostics().iter().any(|diagnostic| {
            diagnostic
                .message()
                .contains("non-scalar Field cannot have a scalar initial value")
        }));
    }

    #[test]
    fn field_physical_connector_fields_are_closed_and_exactly_once() {
        let invalid_sources = [
            (
                "duplicate",
                "connector C = field_physical(trace = u: 1, flux = f: 1, shape = [], frame = invariant, trace = v: 1, pairing = euclidean_boundary_duality);",
            ),
            (
                "missing",
                "connector C = field_physical(trace = u: 1, flux = f: 1, shape = [], frame = invariant);",
            ),
            (
                "unknown",
                "connector C = field_physical(trace = u: 1, flux = f: 1, shape = [], frame = invariant, pairing = euclidean_boundary_duality, channels = 2);",
            ),
        ];

        for (case, source) in invalid_sources {
            let result = parse(format!("{case}.eqi"), source);
            assert!(!result.diagnostics().is_empty(), "{case} must fail closed");
            assert!(result.into_document().is_err(), "{case} cannot compile");
        }
    }

    #[test]
    fn parser_retains_closed_complete_exterior_family_syntax() {
        let source = r#"
component BoundaryLaw {
  public support body: volume(ambient_dimension = 2);
  public support exterior: complete_exterior(parent = body);
  public port mechanical[boundary in exterior]: conserving MechanicalBoundary over boundary;
  relation natural[boundary in exterior] continuous on boundary {
    flux(mechanical[boundary = boundary]) = 0;
  }
  connect conserving [boundary in exterior] child.mechanical[boundary = boundary], mechanical[boundary = boundary];
}

model coupled {
  instance law: BoundaryLaw(
    support body = fluid,
    support exterior = boundaries(x_lower, x_upper, y_lower, y_upper)
  );
  connect conserving law.mechanical[boundary = x_lower], environment;
}
"#;
        let document = parse("complete-exterior.eqi", source)
            .into_document()
            .expect("restricted boundary-family syntax parses");
        let component = &document.components()[0];

        let ComponentItem::Support(exterior) = &component.items()[1] else {
            panic!("second component member is the complete exterior");
        };
        assert!(matches!(
            exterior.syntax(),
            SupportSlotSyntax::CompleteExterior { parent } if parent == "body"
        ));

        let ComponentItem::PortFamily(port) = &component.items()[2] else {
            panic!("third component member is a Port family");
        };
        assert_eq!(port.binder().member(), "boundary");
        assert_eq!(port.binder().set(), "exterior");

        let ComponentItem::RelationFamily(relation) = &component.items()[3] else {
            panic!("fourth component member is a Relation family");
        };
        assert_eq!(relation.relation().domain(), Some("boundary"));
        let ExprKind::Call { arguments, .. } = relation.relation().residuals()[0].kind() else {
            panic!("family Relation residual contains flux selection");
        };
        assert!(matches!(
            arguments[0].kind(),
            ExprKind::BoundaryPortSelection { selector, .. }
                if selector.member() == "boundary" && selector.target() == "boundary"
        ));

        let ComponentItem::BoundaryConnection(connection) = &component.items()[4] else {
            panic!("fifth component member is a pointwise conserving Connection");
        };
        assert_eq!(connection.binder().expect("binder").set(), "exterior");

        let Item::Instance(instance) = &document.models()[0].items()[0] else {
            panic!("first model member is the component instance");
        };
        assert!(
            instance
                .support_bindings()
                .iter()
                .any(|binding| { binding.slot() == "body" && binding.target() == "fluid" })
        );
        assert_eq!(instance.boundary_set_bindings().len(), 1);
        assert_eq!(
            instance.boundary_set_bindings()[0]
                .members()
                .iter()
                .map(BoundarySetMemberSyntax::target)
                .collect::<Vec<_>>(),
            ["x_lower", "x_upper", "y_lower", "y_upper"]
        );

        let Item::BoundaryConnection(connection) = &document.models()[0].items()[1] else {
            panic!("second model member is a selected conserving Connection");
        };
        assert!(connection.binder().is_none());
        assert_eq!(
            connection.ports()[0].selector().expect("selector").target(),
            "x_lower"
        );
    }

    #[test]
    fn parser_retains_exact_model_spatial_periodic_pairs() {
        let source = r#"
model periodic {
  connect periodic upper.interface, lower.interface;
}
"#;
        let document = parse("periodic.eqi", source)
            .into_document()
            .expect("closed Model spatial-periodic pair parses");
        let Item::BoundaryConnection(connection) = &document.models()[0].items()[0] else {
            panic!("periodic syntax is retained as a boundary Connection");
        };
        assert_eq!(connection.syntax(), ConnectionSyntax::SpatialPeriodic);
        assert_eq!(connection.ports().len(), 2);
        assert!(connection.binder().is_none());

        let component = parse(
            "component-periodic.eqi",
            "component C { connect periodic upper, lower; }",
        );
        assert!(component.diagnostics().iter().any(|diagnostic| {
            diagnostic
                .message()
                .contains("allowed only in closed Models")
        }));
    }

    #[test]
    fn parser_rejects_boundary_binders_outside_the_closed_family_sites() {
        let invalid_sources = [
            (
                "signal-port",
                "component C { public port p[b in exterior]: signal input 1; }",
            ),
            (
                "periodic-relation",
                "component C { clock c = periodic(period = 1 / 1, phase = 0 / 1); relation r[b in exterior] periodic(c) on b { 1 = 0; } }",
            ),
            (
                "model-relation",
                "model M { relation r[b in exterior] continuous on b { 1 = 0; } }",
            ),
            (
                "model-connection",
                "model M { connect conserving [b in exterior] left, right; }",
            ),
        ];

        for (case, source) in invalid_sources {
            let result = parse(format!("{case}.eqi"), source);
            assert!(!result.diagnostics().is_empty(), "{case} must fail closed");
            assert!(result.into_document().is_err(), "{case} cannot compile");
        }
    }

    #[test]
    fn parser_and_formatter_retain_ordered_dimension_prefix_with_exact_ranges() {
        let source = "dimension Speed = m / s;\ndimension Acceleration = Speed / s;\nmodel M { field velocity: Speed = 0; }";
        let document = parse("dimensions.eqi", source)
            .into_document()
            .expect("dimension prefix parses");

        assert_eq!(document.dimension_syntax().len(), 2);
        let (name, _, range) = document.dimension_syntax().next().expect("first alias");
        assert_eq!(name, "Speed");
        assert_eq!(
            &source[range.start() as usize..range.end() as usize],
            "dimension Speed = m / s;"
        );
        let formatted = crate::format(&document);
        let reparsed = parse("dimensions.eqi", &formatted)
            .into_document()
            .expect("formatted prefix reparses");
        assert_eq!(crate::format(&reparsed), formatted);

        let misplaced = parse(
            "misplaced.eqi",
            "model M { field x: m = 0; } dimension Length = m;",
        );
        assert!(
            misplaced
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.message().contains("must form a prefix"))
        );
    }
}
