use super::{Parser, Token, TokenKind};
use crate::{Document, Notation, TextRange};
use eqiora_core::{Diagnostic, Span, diagnostic::codes};

impl Parser<'_> {
    /// Only declaration headers call this; reference names never consume notation.
    pub(super) fn declaration_name(&mut self, expected: &str) -> Option<Token> {
        let name = self.expect_identifier(expected)?;
        if self.at(TokenKind::Notation) {
            let token = self.bump().clone();
            match Notation::parse(token.text()) {
                Ok(notation) => self
                    .notations
                    .push((name.range(), notation.at(token.range()))),
                Err(error) => self.diagnostics.push(
                    Diagnostic::error(codes::SYNTAX_ERROR, error.message()).with_span(Span {
                        file: self.file.clone(),
                        start: token.range().start() + error.range().start(),
                        end: token.range().start() + error.range().end(),
                    }),
                ),
            }
        }
        Some(name)
    }
}

pub(super) fn attach(
    document: &mut Document,
    tokens: &[Token],
    notations: Vec<(TextRange, Notation)>,
) {
    if notations.is_empty() {
        return;
    }
    let mut ranges = Vec::new();
    document.visit_comments(|range, _| ranges.push(range));
    let root = ranges.len();
    ranges.push(TextRange::new(
        0,
        tokens.last().map_or(0, |token| token.range().end()),
    ));
    let owners = super::comments::owners::OwnerIndex::new(&ranges);
    let mut admitted = std::collections::HashMap::new();
    for (name, notation) in notations {
        let index = owners.containing(name);
        if index == root {
            continue;
        }
        let declaration = ranges[index];
        let start = tokens.partition_point(|token| token.range().start() < declaration.start());
        let end = tokens.partition_point(|token| token.range().end() <= name.start());
        let header = tokens[start..end]
            .iter()
            .filter(|token| !token.kind().is_trivia());
        // A failed/recovered declaration cannot lend notation to a containing
        // Model or Component: a header contains only its keywords/modifiers.
        if header
            .take(4)
            .enumerate()
            .all(|(index, token)| index < 3 && token.kind() == TokenKind::Identifier)
        {
            admitted.insert(declaration, notation);
        }
    }
    document.visit_comments_mut(|range, metadata| {
        metadata.notation = admitted.remove(&range);
    });
}
