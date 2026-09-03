use crate::ast::document::{ImportDecl, ModuleDecl};
use crate::ast::{DimensionDecl, Document, ModelDecl, TextRange, VisibilitySyntax};
use crate::lexer::TokenKind;

use super::Parser;

impl Parser<'_> {
    pub(super) fn parse_document(&mut self) -> Option<Document> {
        let mut module = None;
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
        let mut module_prefix_closed = false;
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

            if self.at_keyword("module") {
                if let Some((_, token)) = &modifier {
                    self.error_token(token, "module declarations have no visibility modifier");
                }
                if module_prefix_closed {
                    self.error_here("the module declaration must precede imports and declarations");
                }
                if module.is_some() {
                    self.error_here("a source file has exactly one module declaration");
                }
                if let Some(declaration) = self.parse_module(declaration_start) {
                    module.get_or_insert(declaration);
                } else {
                    self.recover_top_level();
                }
            } else if self.at_keyword("import") {
                module_prefix_closed = true;
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
                module_prefix_closed = true;
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
                module_prefix_closed = true;
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
                module_prefix_closed = true;
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
                module_prefix_closed = true;
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
                module_prefix_closed = true;
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
                module_prefix_closed = true;
                import_prefix_closed = true;
                declarations_started = true;
                models_started = true;
                if let Some(model) = self.parse_model(declaration_start, visibility) {
                    models.push(model);
                } else {
                    self.recover_top_level();
                }
            } else {
                let expected = if import_prefix_closed {
                    "expected `dimension`, `property`, `connector`, `component`, `pure operator`, or `model` declaration"
                } else if module.is_some() {
                    "expected `import`, `dimension`, `property`, `connector`, `component`, `pure operator`, or `model` declaration"
                } else {
                    "expected `module`, `import`, `dimension`, `property`, `connector`, `component`, `pure operator`, or `model` declaration"
                };
                self.error_here(expected);
                self.recover_top_level();
            }
        }
        (module.is_some()
            || !(imports.is_empty()
                && dimensions.is_empty()
                && property_contracts.is_empty()
                && property_releases.is_empty()
                && connectors.is_empty()
                && components.is_empty()
                && pure_operators.is_empty()
                && models.is_empty()))
        .then_some(Document {
            module,
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

    pub(super) fn parse_model(
        &mut self,
        declaration_start: u32,
        visibility: VisibilitySyntax,
    ) -> Option<ModelDecl> {
        self.expect_keyword("model")?;
        let name = self.expect_identifier("model name")?.text().to_owned();
        self.expect(TokenKind::LeftBrace, "`{` after model name")?;
        let mut items = Vec::new();
        while !self.at(TokenKind::RightBrace) && !self.at(TokenKind::Eof) {
            match self.parse_item() {
                Some(item) => items.push(item),
                None => self.recover_item(),
            }
        }
        let end = self
            .expect(TokenKind::RightBrace, "`}` to close model")?
            .range()
            .end();
        Some(ModelDecl {
            visibility,
            name,
            items,
            range: TextRange::new(declaration_start, end),
        })
    }

    fn parse_module(&mut self, start: u32) -> Option<ModuleDecl> {
        self.expect_keyword("module")?;
        let name = self.parse_name_path("logical module name")?;
        let end = self
            .expect(TokenKind::Semicolon, "`;` after module declaration")?
            .range()
            .end();
        Some(ModuleDecl {
            name,
            range: TextRange::new(start, end),
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

    #[test]
    fn parser_and_formatter_retain_source_owned_module_identity() {
        let source = "module library.primitives;\nimport materials.water as water;\npublic component Resistor {}";
        let document = parse("primitives.eqi", source)
            .into_document()
            .expect("module declaration parses");
        let (module, range) = document.module().expect("declared module");
        assert_eq!(module.as_str(), "library.primitives");
        assert_eq!(
            &source[range.start() as usize..range.end() as usize],
            "module library.primitives;"
        );

        let formatted = format(&document);
        assert!(formatted.starts_with(
            "module library.primitives;\nimport materials.water as water;\n\npublic component Resistor"
        ));
        assert_eq!(
            format(
                &parse("primitives.eqi", &formatted)
                    .into_document()
                    .expect("formatted module reparses")
            ),
            formatted
        );

        let empty_module = parse("empty.eqi", "module library.empty;")
            .into_document()
            .expect("an explicitly named empty module is a document");
        assert_eq!(format(&empty_module), "module library.empty;\n");

        let misplaced = parse(
            "misplaced.eqi",
            "import a.b as b; module c.d; model Main {}",
        );
        assert!(misplaced.diagnostics().iter().any(|diagnostic| {
            diagnostic
                .message()
                .contains("module declaration must precede")
        }));
    }

    #[test]
    fn declaration_diagnostic_offers_header_forms_only_while_their_prefix_is_open() {
        let before = parse("before.eqi", "unexpected");
        assert_eq!(
            before.diagnostics()[0].message(),
            "expected `module`, `import`, `dimension`, `property`, `connector`, `component`, `pure operator`, or `model` declaration"
        );

        let after = parse("after.eqi", "model Main {} unexpected");
        assert_eq!(
            after.diagnostics()[0].message(),
            "expected `dimension`, `property`, `connector`, `component`, `pure operator`, or `model` declaration"
        );
    }
}
