//! Directed and conserving endpoint parsing and lexical family selections.

use super::*;

impl Parser<'_> {
    pub(super) fn parse_connection(&mut self, component: bool) -> Option<ParsedConnection> {
        let start = self.expect_keyword("connect")?.range().start();
        let syntax = if self.at_keyword("conserving") {
            self.bump();
            ConnectionSyntax::Conserving
        } else if self.at_keyword("periodic") {
            self.bump();
            ConnectionSyntax::SpatialPeriodic
        } else {
            ConnectionSyntax::Signal
        };
        if syntax == ConnectionSyntax::SpatialPeriodic && component {
            self.error_here("spatial-periodic Connections are allowed only in closed Models");
            return None;
        }
        let binder = if self.at(TokenKind::LeftBracket) {
            if syntax == ConnectionSyntax::SpatialPeriodic {
                self.error_here("spatial-periodic Connections cannot declare a family binder");
                return None;
            }
            Some(self.parse_index_family_binder()?)
        } else {
            None
        };
        let mut ports = vec![self.parse_expression(0)?];
        if syntax == ConnectionSyntax::Signal {
            self.expect(TokenKind::Arrow, "`->` after signal output")?;
            ports.push(self.parse_expression(0)?);
        }
        while self.at(TokenKind::Comma) {
            self.bump();
            ports.push(self.parse_expression(0)?);
        }
        if ports.len() < 2 {
            self.error_here("Connection requires at least two Ports");
        }
        let end = self
            .expect(TokenKind::Semicolon, "`;` after Connection")?
            .range()
            .end();
        let boundary = syntax == ConnectionSyntax::SpatialPeriodic
            || ports
                .iter()
                .any(|port| matches!(port.kind(), ExprKind::BoundaryPortSelection { .. }));
        if boundary {
            if syntax == ConnectionSyntax::Signal {
                self.error_here("signal Connection requires exact declared signal Port selections");
                return None;
            }
            if syntax == ConnectionSyntax::SpatialPeriodic && ports.len() != 2 {
                self.error_here("spatial-periodic Connection requires exactly two Ports");
            }
            let mut references = Vec::new();
            for port in ports {
                let range = port.range;
                let (port, selector) = match port.kind {
                    ExprKind::BoundaryPortSelection { port, selector } => (*port, Some(*selector)),
                    ExprKind::Path(port) => (port, None),
                    ExprKind::Name(name) => (NamePath::single(name, range), None),
                    _ => {
                        self.error_here(
                            "boundary Connection requires exact boundary Port references",
                        );
                        return None;
                    }
                };
                references.push(BoundaryPortReferenceSyntax { port, selector });
            }
            return Some(ParsedConnection::Boundary(BoundaryConnectionDecl {
                comments: Default::default(),
                syntax,
                binder,
                ports: references,
                range: TextRange::new(start, end),
            }));
        }
        for port in &mut ports {
            if !matches!(
                port.kind(),
                ExprKind::Name(_) | ExprKind::Path(_) | ExprKind::Member { .. }
            ) {
                self.error_here("Connection endpoint requires an exact declared Port selection");
                return None;
            }
            if syntax == ConnectionSyntax::Conserving
                && let ExprKind::Name(name) = &port.kind
            {
                port.kind = ExprKind::Path(NamePath::single(name.clone(), port.range));
            }
        }
        Some(ParsedConnection::Ordinary(ConnectionDecl {
            comments: Default::default(),
            syntax,
            binder,
            ports,
            range: TextRange::new(start, end),
        }))
    }

    pub(super) fn parse_boundary_family_binder(&mut self) -> Option<FamilyBinderSyntax> {
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
        Some(FamilyBinderSyntax {
            member,
            set: NamePath::single(set, TextRange::new(start, end)),
            range: TextRange::new(start, end),
        })
    }

    pub(super) fn parse_boundary_port_selector(&mut self) -> Option<BoundaryPortSelectorSyntax> {
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
}
