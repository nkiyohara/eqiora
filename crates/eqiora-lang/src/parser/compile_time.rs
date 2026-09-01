use crate::ast::{LetDecl, ParameterDecl, TextRange};
use crate::lexer::TokenKind;

use super::Parser;

impl Parser<'_> {
    pub(super) fn parse_parameter(&mut self) -> Option<ParameterDecl> {
        let start = self.expect_keyword("parameter")?.range().start();
        let name = self
            .expect_identifier("declaration name")?
            .text()
            .to_owned();
        self.expect(TokenKind::Colon, "`:` before dimension")?;
        let dimension = self.parse_expression(0)?;
        self.expect(TokenKind::Equal, "`=` before value")?;
        let initial = self.parse_signed_number()?;
        let end = self
            .expect(TokenKind::Semicolon, "`;` after declaration")?
            .range()
            .end();
        Some(ParameterDecl {
            name,
            dimension,
            initial,
            range: TextRange::new(start, end),
        })
    }

    pub(super) fn parse_let(&mut self) -> Option<LetDecl> {
        let start = self.expect_keyword("let")?.range().start();
        let name = self.expect_identifier("alias name")?.text().to_owned();
        let dimension = if self.at(TokenKind::Colon) {
            self.bump();
            Some(self.parse_expression(0)?)
        } else {
            None
        };
        self.expect(TokenKind::Equal, "`=` before alias expression")?;
        let value = self.parse_expression(0)?;
        let end = self
            .expect(TokenKind::Semicolon, "`;` after declaration")?
            .range()
            .end();
        Some(LetDecl {
            name,
            dimension,
            value,
            range: TextRange::new(start, end),
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::ast::Item;
    use crate::{format, parse};

    #[test]
    fn parser_and_formatter_retain_annotated_and_inferred_let_aliases() {
        let source =
            "model M { let wave_number = math.pi / length; let checked: 1 / m = wave_number; }";
        let document = parse("let.eqi", source)
            .into_document()
            .expect("let aliases parse");
        let Item::Let(declaration) = &document.models()[0].items()[0] else {
            panic!("model item is a let alias");
        };
        assert_eq!(declaration.name(), "wave_number");
        assert!(declaration.dimension().is_none());
        let Item::Let(checked) = &document.models()[0].items()[1] else {
            panic!("second model item is a let alias");
        };
        assert!(checked.dimension().is_some());
        let formatted = format(&document);
        assert_eq!(
            formatted,
            "model M {\n  let wave_number = math.pi / length;\n  let checked: 1 / m = wave_number;\n}\n"
        );
        let reparsed = parse("formatted-let.eqi", &formatted)
            .into_document()
            .expect("formatted let aliases parse");
        assert_eq!(format(&reparsed), formatted);
    }
}
