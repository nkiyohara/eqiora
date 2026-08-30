use super::Parser;
use crate::ast::{TextRange, VisibilitySyntax};
use crate::ast_property::{
    ComponentPropertyDecl, PropertyBindingDecl, PropertyContractDecl, PropertyReleaseDecl,
};
use crate::lexer::{Token, TokenKind};

impl Parser<'_> {
    pub(super) fn at_support_binding(&mut self) -> bool {
        self.at_discriminated_binding("support")
    }
    pub(super) fn at_field_binding(&mut self) -> bool {
        self.at_discriminated_binding("field")
    }
    fn at_discriminated_binding(&mut self, discriminator: &str) -> bool {
        self.at_keyword(discriminator)
            && self
                .following_significant_token()
                .is_some_and(|token| token.kind() == TokenKind::Identifier)
    }
    pub(super) fn at_field_slot_declaration(&mut self) -> bool {
        self.at_keyword("field")
            && self.following_significant_token().is_some_and(|token| {
                token.kind() == TokenKind::Identifier && token.text() == "slot"
            })
    }
    pub(super) fn following_significant_token(&self) -> Option<&Token> {
        self.tokens[self.cursor.saturating_add(1)..]
            .iter()
            .find(|token| !token.kind().is_trivia())
    }
    pub(super) fn previous_significant_range(&self) -> TextRange {
        self.tokens[..self.cursor]
            .iter()
            .rev()
            .find(|token| !token.kind().is_trivia())
            .map_or(TextRange::new(0, 0), Token::range)
    }

    pub(super) fn parse_top_property(
        &mut self,
        start: u32,
        visibility: VisibilitySyntax,
        models_started: bool,
        contracts: &mut Vec<PropertyContractDecl>,
        releases: &mut Vec<PropertyReleaseDecl>,
    ) {
        if models_started {
            self.error_here("property declarations must precede model declarations");
        }
        self.bump();
        let parsed = if self.at_keyword("contract") {
            self.parse_property_contract(start, visibility)
                .map(|value| contracts.push(value))
        } else if self.at_keyword("release") {
            self.parse_property_release(start, visibility)
                .map(|value| releases.push(value))
        } else {
            self.error_here("expected `contract` or `release` after `property`");
            None
        };
        if parsed.is_none() {
            self.recover_top_level();
        }
    }

    pub(super) fn parse_property_contract(
        &mut self,
        start: u32,
        visibility: VisibilitySyntax,
    ) -> Option<PropertyContractDecl> {
        self.expect_keyword("contract")?;
        let name = self
            .expect_identifier("property contract name")?
            .text()
            .to_owned();
        self.expect(TokenKind::LeftBrace, "`{` after property contract name")?;
        self.expect_keyword("scalar")?;
        self.expect_keyword("value")?;
        self.expect(TokenKind::Colon, "`:` before property dimension")?;
        let dimension = self.parse_expression(0)?;
        self.expect(TokenKind::Semicolon, "`;` after property role")?;
        let end = self
            .expect(TokenKind::RightBrace, "`}` after property contract")?
            .range()
            .end();
        Some(PropertyContractDecl {
            visibility,
            name,
            dimension,
            range: TextRange::new(start, end),
        })
    }

    pub(super) fn parse_property_release(
        &mut self,
        start: u32,
        visibility: VisibilitySyntax,
    ) -> Option<PropertyReleaseDecl> {
        self.expect_keyword("release")?;
        let name = self
            .expect_identifier("property release name")?
            .text()
            .to_owned();
        self.expect_keyword("implements")?;
        let contract = self.parse_name_path("property contract name")?;
        self.expect(TokenKind::LeftBrace, "`{` after property release contract")?;
        self.expect_keyword("value")?;
        self.expect(TokenKind::Equal, "`=` after release value")?;
        let source_value = self.parse_expression(0)?;
        self.expect(TokenKind::Semicolon, "`;` after release value")?;
        self.expect_keyword("source_unit")?;
        self.expect(TokenKind::Colon, "`:` before source unit dimension")?;
        let source_dimension = self.parse_expression(0)?;
        self.expect(TokenKind::Equal, "`=` before coherent-SI scale")?;
        let coherent_si_scale = self.parse_expression(0)?;
        self.expect(TokenKind::Semicolon, "`;` after source unit")?;
        self.expect_keyword("validity")?;
        self.expect(TokenKind::Equal, "`=` after validity")?;
        self.expect_keyword("unconditional")?;
        self.expect(TokenKind::Semicolon, "`;` after validity")?;
        self.expect_keyword("citation")?;
        self.expect(TokenKind::Equal, "`=` after citation")?;
        let citation = self.parse_name_path("citation identity")?;
        self.expect(TokenKind::Semicolon, "`;` after citation")?;
        self.expect_keyword("license")?;
        self.expect(TokenKind::Equal, "`=` after license")?;
        let license = self.parse_name_path("license identity")?;
        self.expect(TokenKind::Semicolon, "`;` after license")?;
        let end = self
            .expect(TokenKind::RightBrace, "`}` after property release")?
            .range()
            .end();
        Some(PropertyReleaseDecl {
            visibility,
            name,
            contract,
            source_value,
            source_dimension,
            coherent_si_scale,
            citation,
            license,
            range: TextRange::new(start, end),
        })
    }

    pub(super) fn at_component_property(&mut self) -> bool {
        self.at_keyword("public")
            && self.following_significant_token().is_some_and(|token| {
                token.kind() == TokenKind::Identifier && token.text() == "property"
            })
    }

    pub(super) fn parse_component_property(&mut self) -> Option<ComponentPropertyDecl> {
        let start = self.bump().range().start();
        self.expect_keyword("property")?;
        let name = self
            .expect_identifier("property requirement name")?
            .text()
            .to_owned();
        self.expect(TokenKind::Colon, "`:` before property contract")?;
        let contract = self.parse_name_path("property contract name")?;
        let end = self
            .expect(TokenKind::Semicolon, "`;` after property requirement")?
            .range()
            .end();
        Some(ComponentPropertyDecl {
            name,
            contract,
            range: TextRange::new(start, end),
        })
    }

    pub(super) fn parse_property_binding(&mut self, start: u32) -> Option<PropertyBindingDecl> {
        self.expect_keyword("property")?;
        let property = self
            .expect_identifier("public property requirement name")?
            .text()
            .to_owned();
        self.expect(TokenKind::Equal, "`=` in property binding")?;
        let release = self.parse_name_path("property release name")?;
        let end = release.range().end();
        Some(PropertyBindingDecl {
            property,
            release,
            range: TextRange::new(start, end),
        })
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn contract_release_requirement_and_binding_round_trip() {
        let source = r#"public property contract Diffusivity {
  scalar value: m ^ 2 / s;
}

property release ReferenceDiffusivity implements Diffusivity {
  value = 25;
  source_unit: m ^ 2 / s = 1 / 1000;
  validity = unconditional;
  citation = org.example.measurement;
  license = spdx.CC0_1_0;
}

public component Diffusion {
  public property diffusivity: Diffusivity;
  relation law continuous { diffusivity = 0; }
}

model Main {
  instance domain: Diffusion(property diffusivity = ReferenceDiffusivity);
}"#;
        let document = crate::parse("property.eqi", source)
            .into_document()
            .expect("valid property source");
        assert_eq!(document.property_contract_syntax().len(), 1);
        assert_eq!(document.property_release_syntax().len(), 1);
        assert_eq!(
            document.components()[0].property_requirement_syntax().len(),
            1
        );
        let formatted = crate::format(&document);
        let reparsed = crate::parse("formatted.eqi", &formatted)
            .into_document()
            .expect("formatted source reparses");
        assert_eq!(crate::format(&reparsed), formatted);
    }
}
