//! Closed authored-Formulation parsing.

use crate::ast::formulation::FormulationDecl;
use crate::ast::{ComponentDecl, TextRange, VisibilitySyntax};
use crate::lexer::TokenKind;

use super::{ParsedComponentItem, Parser};

impl Parser<'_> {
    pub(super) fn parse_component(
        &mut self,
        start: u32,
        visibility: VisibilitySyntax,
    ) -> Option<ComponentDecl> {
        self.expect_keyword("component")?;
        let name = self.expect_identifier("component name")?.text().to_owned();
        self.expect(TokenKind::LeftBrace, "`{` after component name")?;
        let mut items = Vec::new();
        let mut formulations = Vec::new();
        let mut property_requirements = Vec::new();
        let mut formulations_started = false;
        while !self.at(TokenKind::RightBrace) && !self.at(TokenKind::Eof) {
            if self.at_keyword("form") {
                formulations_started = true;
                formulations.push(self.parse_formulation()?);
                continue;
            }
            if formulations_started {
                self.error_here("component declarations must precede authored forms");
                self.recover_item();
                continue;
            }
            if self.at_component_property() {
                property_requirements.push(self.parse_component_property()?);
                continue;
            }
            match self.parse_component_item() {
                Some(ParsedComponentItem::Retained(item)) => items.push(*item),
                Some(ParsedComponentItem::Discarded) => {}
                None => self.recover_item(),
            }
        }
        let end = self
            .expect(TokenKind::RightBrace, "`}` to close component")?
            .range()
            .end();
        Some(ComponentDecl {
            visibility,
            name,
            items,
            formulations,
            property_requirements,
            range: TextRange::new(start, end),
        })
    }

    pub(super) fn parse_formulation(&mut self) -> Option<FormulationDecl> {
        let start = self.expect_keyword("form")?.range().start();
        if self.at_keyword("primal") {
            self.bump();
        } else {
            self.error_here("expected `primal` after `form`");
            return None;
        };
        self.expect_keyword("for")?;
        let relation = self
            .expect_identifier("Formulation Relation")?
            .text()
            .to_owned();
        self.expect(TokenKind::LeftBrace, "`{` before authored Formulation")?;
        let left = self.parse_expression(0)?;
        self.expect(TokenKind::Equal, "`=` in authored Formulation")?;
        let right = self.parse_expression(0)?;
        self.expect(
            TokenKind::Semicolon,
            "`;` after authored Formulation equality",
        )?;
        let end = self
            .expect(
                TokenKind::RightBrace,
                "`}` after the single authored Formulation equality",
            )?
            .range()
            .end();
        Some(FormulationDecl {
            relation,
            left,
            right,
            range: TextRange::new(start, end),
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::ast::{ComponentItem, ExprKind};
    use crate::parse;

    #[test]
    fn retains_one_component_primal_form_outside_model_items() {
        let source = r#"
component Diffusion {
  public support region: volume(ambient_dimension = 2);
  representation space = continuum;
  field potential on region as space: 1 = 0;
  parameter diffusion: 1 = 1;
  parameter source: 1 / m ^ 2 = 1;
  relation balance continuous on region { -div(diffusion * grad(potential)) = source; }
  form primal for balance {
    integrate(region, dot(grad(test(potential)), diffusion * grad(potential)))
      = integrate(region, test(potential) * source);
  }
}
"#;
        let document = parse("form.eqi", source).into_document().unwrap();
        let component = &document.components()[0];
        let forms = component.formulations().collect::<Vec<_>>();
        let [(relation, left, right, _)] = forms.as_slice() else {
            panic!("one form expected")
        };
        assert_eq!(*relation, "balance");
        assert!(
            matches!(left.kind(), ExprKind::Call { callee, arguments } if callee.as_str() == "integrate" && arguments.len() == 2)
        );
        assert!(
            matches!(right.kind(), ExprKind::Call { callee, arguments } if callee.as_str() == "integrate" && arguments.len() == 2)
        );
        assert!(component.items().iter().all(
            |item| !matches!(item, ComponentItem::Relation(relation) if relation.name() == "primal")
        ));

        let misplaced = parse(
            "misplaced.eqi",
            "component C { relation r continuous { 1 = 0; } form primal for r { integrate(d, test(x)) = integrate(d, test(x)); } parameter p: 1 = 1; }",
        );
        assert!(misplaced.diagnostics().iter().any(|diagnostic| {
            diagnostic
                .message()
                .contains("declarations must precede authored forms")
        }));
    }
}
