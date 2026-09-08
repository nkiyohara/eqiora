//! Directed and conserving endpoint parsing and lexical family selections.

use super::*;

impl Parser<'_> {
    pub(super) fn parse_connection(&mut self, allow_family: bool) -> Option<ParsedConnection> {
        let start = self.expect_keyword("connect")?.range().start();
        if !self.at_keyword("conserving") && !self.at_keyword("periodic") {
            let mut ports = vec![self.parse_expression(0)?];
            self.expect(TokenKind::Arrow, "`->` after signal output")?;
            ports.push(self.parse_expression(0)?);
            while self.at(TokenKind::Comma) {
                self.bump();
                ports.push(self.parse_expression(0)?);
            }
            for endpoint in &ports {
                if !matches!(
                    endpoint.kind(),
                    ExprKind::Name(_) | ExprKind::Path(_) | ExprKind::Member { .. }
                ) {
                    self.error_here(
                        "Connection endpoint requires an exact declared Port selection",
                    );
                    return None;
                }
            }
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
            self.error_here(
                "expected directed Port connection, `conserving`, or `periodic` after `connect`",
            );
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
                ports: ports
                    .into_iter()
                    .map(|port| Expr {
                        resolved_nominal: None,
                        range: port.port.range(),
                        kind: ExprKind::Path(port.port),
                    })
                    .collect(),
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

    pub(super) fn parse_boundary_port_reference(
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

    pub(super) fn parse_boundary_port_reference_list(
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
}
