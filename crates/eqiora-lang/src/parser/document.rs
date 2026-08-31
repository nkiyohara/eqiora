use crate::ast::{DimensionDecl, Document, TextRange, VisibilitySyntax};
use crate::lexer::TokenKind;

use super::Parser;

impl Parser<'_> {
    pub(super) fn parse_document(&mut self) -> Option<Document> {
        let mut dimensions = Vec::new();
        let mut property_contracts = Vec::new();
        let mut property_releases = Vec::new();
        let mut connectors = Vec::new();
        let mut components = Vec::new();
        let mut pure_operators = Vec::new();
        let mut models = Vec::new();
        let mut models_started = false;
        let mut declarations_started = false;
        while !self.at(TokenKind::Eof) {
            let modifier = if self.at_keyword("public") {
                Some((VisibilitySyntax::Public, self.bump().clone()))
            } else if self.at_keyword("private") {
                Some((VisibilitySyntax::Private, self.bump().clone()))
            } else {
                None
            };
            let visibility = modifier
                .as_ref()
                .map_or(VisibilitySyntax::Private, |(visibility, _)| *visibility);
            let declaration_start = modifier.as_ref().map_or_else(
                || self.current().range().start(),
                |(_, token)| token.range().start(),
            );

            if self.at_keyword("dimension") {
                if modifier.is_some() {
                    self.error_here(
                        "compilation-unit dimension aliases have no visibility modifier",
                    );
                }
                if declarations_started {
                    self.error_here("compilation-unit dimension aliases must form a prefix before all other declarations");
                }
                if let Some(dimension) = self.parse_dimension(declaration_start) {
                    dimensions.push(dimension);
                } else {
                    self.recover_top_level();
                }
            } else if self.at_keyword("property") {
                declarations_started = true;
                self.parse_top_property(
                    declaration_start,
                    visibility,
                    models_started,
                    &mut property_contracts,
                    &mut property_releases,
                );
            } else if self.at_keyword("connector") {
                declarations_started = true;
                if models_started {
                    self.error_here(
                        "compilation-unit Connector declarations must precede model declarations",
                    );
                }
                if let Some(connector) = self.parse_connector(declaration_start, visibility) {
                    connectors.push(connector);
                } else {
                    self.recover_top_level();
                }
            } else if self.at_keyword("component") {
                declarations_started = true;
                if models_started {
                    self.error_here(
                        "compilation-unit component declarations must precede model declarations",
                    );
                }
                if let Some(component) = self.parse_component(declaration_start, visibility) {
                    components.push(component);
                } else {
                    self.recover_top_level();
                }
            } else if self.at_keyword("pure") {
                declarations_started = true;
                if models_started {
                    self.error_here(
                        "compilation-unit pure operator declarations must precede model declarations",
                    );
                }
                if let Some(operator) = self.parse_pure_operator(declaration_start, visibility) {
                    pure_operators.push(operator);
                } else {
                    self.recover_top_level();
                }
            } else if self.at_keyword("model") {
                declarations_started = true;
                if let Some((VisibilitySyntax::Public, token)) = &modifier {
                    self.error_token(
                        token,
                        "`model` declarations are package-local and cannot be public in v1",
                    );
                }
                models_started = true;
                if let Some(model) = self.parse_model() {
                    models.push(model);
                } else {
                    self.recover_top_level();
                }
            } else {
                self.error_here(
                    "expected `dimension`, `property`, `connector`, `component`, `pure operator`, or `model` declaration",
                );
                self.recover_top_level();
            }
        }
        (!(dimensions.is_empty()
            && property_contracts.is_empty()
            && property_releases.is_empty()
            && connectors.is_empty()
            && components.is_empty()
            && pure_operators.is_empty()
            && models.is_empty()))
        .then_some(Document {
            dimensions,
            property_contracts,
            property_releases,
            connectors,
            components,
            pure_operators,
            models,
        })
    }

    fn parse_dimension(&mut self, start: u32) -> Option<DimensionDecl> {
        self.expect_keyword("dimension")?;
        let name = self
            .expect_identifier("dimension alias name")?
            .text()
            .to_owned();
        self.expect(TokenKind::Equal, "`=` before dimension expression")?;
        let expression = self.parse_expression(0)?;
        let end = self
            .expect(TokenKind::Semicolon, "`;` after dimension alias")?
            .range()
            .end();
        Some(DimensionDecl {
            name,
            expression,
            range: TextRange::new(start, end),
        })
    }
}
