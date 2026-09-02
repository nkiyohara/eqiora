use crate::ast::document::ImportDecl;
use crate::ast::{DimensionDecl, Document, TextRange, VisibilitySyntax};
use crate::lexer::TokenKind;

use super::Parser;

impl Parser<'_> {
    pub(super) fn parse_document(&mut self) -> Option<Document> {
        let mut imports = Vec::new();
        let mut dimensions = Vec::new();
        let mut property_contracts = Vec::new();
        let mut property_releases = Vec::new();
        let mut connectors = Vec::new();
        let mut components = Vec::new();
        let mut pure_operators = Vec::new();
        let mut models = Vec::new();
        let mut models_started = false;
        let mut declarations_started = false;
        let mut import_prefix_closed = false;
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

            if self.at_keyword("import") {
                if let Some((_, token)) = &modifier {
                    self.error_token(token, "module imports have no visibility modifier");
                }
                if import_prefix_closed {
                    self.error_here("module imports must form a prefix before all declarations");
                }
                if let Some(import) = self.parse_import(declaration_start) {
                    imports.push(import);
                } else {
                    self.recover_top_level();
                }
            } else if self.at_keyword("dimension") {
                import_prefix_closed = true;
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
                import_prefix_closed = true;
                declarations_started = true;
                self.parse_top_property(
                    declaration_start,
                    visibility,
                    models_started,
                    &mut property_contracts,
                    &mut property_releases,
                );
            } else if self.at_keyword("connector") {
                import_prefix_closed = true;
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
                import_prefix_closed = true;
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
                import_prefix_closed = true;
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
                import_prefix_closed = true;
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
                    "expected `import`, `dimension`, `property`, `connector`, `component`, `pure operator`, or `model` declaration",
                );
                self.recover_top_level();
            }
        }
        (!(imports.is_empty()
            && dimensions.is_empty()
            && property_contracts.is_empty()
            && property_releases.is_empty()
            && connectors.is_empty()
            && components.is_empty()
            && pure_operators.is_empty()
            && models.is_empty()))
        .then_some(Document {
            imports,
            dimensions,
            property_contracts,
            property_releases,
            connectors,
            components,
            pure_operators,
            models,
        })
    }

    fn parse_import(&mut self, start: u32) -> Option<ImportDecl> {
        self.expect_keyword("import")?;
        let module = self.parse_name_path("logical module name")?;
        self.expect_keyword("as")?;
        let alias = self
            .expect_identifier("module import alias")?
            .text()
            .to_owned();
        let end = self
            .expect(TokenKind::Semicolon, "`;` after module import")?
            .range()
            .end();
        Some(ImportDecl {
            module,
            alias,
            range: TextRange::new(start, end),
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

#[cfg(test)]
mod tests {
    use crate::{format, parse};

    #[test]
    fn parser_and_formatter_retain_explicit_module_import_prefix() {
        let source =
            "import geometry.channel as channel;\nimport materials.water as water;\nmodel Main {}";
        let document = parse("main.eqi", source)
            .into_document()
            .expect("module imports parse");
        let imports = document.imports().collect::<Vec<_>>();

        assert_eq!(imports.len(), 2);
        assert_eq!(imports[0].0.as_str(), "geometry.channel");
        assert_eq!(imports[0].1, "channel");
        assert_eq!(
            &source[imports[0].2.start() as usize..imports[0].2.end() as usize],
            "import geometry.channel as channel;"
        );

        let formatted = format(&document);
        assert!(formatted.starts_with(
            "import geometry.channel as channel;\nimport materials.water as water;\n\nmodel Main"
        ));
        let reparsed = parse("main.eqi", &formatted)
            .into_document()
            .expect("formatted imports reparse");
        assert_eq!(format(&reparsed), formatted);

        let misplaced = parse(
            "misplaced.eqi",
            "model Main {} import geometry.channel as channel;",
        );
        assert!(
            misplaced
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.message().contains("imports must form a prefix"))
        );
    }
}
