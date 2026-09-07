//! Attach trivia to recovered declaration owners, never across skipped syntax.

use crate::ast::comments::{CommentPosition, CommentTrivia, SourceComments, SyntaxAnchor};
use crate::ast::{DocComment, Document, TextRange};
use crate::lexer::{Token, TokenKind};
use eqiora_core::{Diagnostic, Span, diagnostic::codes};

mod owners;
use owners::OwnerIndex;

pub(super) fn attach(
    document: &mut Document,
    tokens: &[Token],
    source: &str,
    file: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !tokens
        .iter()
        .any(|token| matches!(token.kind(), TokenKind::LineComment | TokenKind::DocComment))
    {
        return;
    }
    let mut ranges = Vec::new();
    document.visit_comments(|range, _| ranges.push(range));
    let root = ranges.len();
    ranges.push(TextRange::new(0, source.len() as u32));
    let mut owners: Vec<_> = ranges
        .iter()
        .map(|range| SourceComments {
            range: *range,
            ..SourceComments::default()
        })
        .collect();
    let significant: Vec<_> = tokens
        .iter()
        .filter(|token| !token.kind().is_trivia() && token.kind() != TokenKind::Eof)
        .collect();
    let owner_index = OwnerIndex::new(&ranges);
    for token in &significant {
        let owner = owner_index.containing(token.range());
        owners[owner]
            .tokens
            .push((SyntaxAnchor::Token(token.kind()), token.range()));
    }
    for (index, range) in ranges[..root].iter().enumerate() {
        let parent = owner_index.parents[index];
        owners[parent]
            .tokens
            .push((SyntaxAnchor::Child(*range), *range));
    }
    for owner in &mut owners {
        owner.tokens.sort_by_key(|(_, range)| range.start());
    }

    let mut next_detached_doc = None;
    for (start, end) in comment_groups(tokens, source).into_iter().rev() {
        let token = &tokens[start];
        let before = &source[..token.range().start() as usize];
        let inline = !before
            .rsplit('\n')
            .next()
            .unwrap_or_default()
            .trim()
            .is_empty();
        let range = TextRange::new(token.range().start(), tokens[end].range().end());
        let next =
            significant.get(significant.partition_point(|next| next.range().start() < range.end()));
        let next_owner = next.and_then(|next| owner_index.starting_at(next.range().start()));
        let gap = next.map_or("", |next| {
            &source[range.end() as usize..next.range().start() as usize]
        });
        let directly_leading = !inline && gap.trim().is_empty();
        let attached = token.kind() == TokenKind::DocComment
            && directly_leading
            && gap.matches('\n').count() == 1
            && next_owner.is_some();
        let previous = significant
            .partition_point(|previous| previous.range().end() <= range.start())
            .checked_sub(1)
            .and_then(|index| significant.get(index));
        let previous_owner =
            previous.and_then(|previous| owner_index.ending_at(previous.range().end()));
        let crosses_detached_documentation = next
            .zip(next_detached_doc)
            .is_some_and(|(next, detached)| detached < next.range().start());
        if token.kind() == TokenKind::DocComment && !attached {
            next_detached_doc = Some(range.start());
        }
        let (owner, position) = if !inline
            && let Some(next_owner) = next_owner
            && (attached
                || (token.kind() == TokenKind::LineComment && !crosses_detached_documentation))
        {
            (next_owner, CommentPosition::Leading)
        } else if inline && let Some(previous_owner) = previous_owner {
            (previous_owner, CommentPosition::Trailing)
        } else {
            let owner = owner_index.containing(range);
            let token = owners[owner]
                .tokens
                .partition_point(|(_, token)| token.end() <= range.start());
            (owner, CommentPosition::Gap { token, inline })
        };
        if attached {
            let text = tokens[start..=end]
                .iter()
                .filter(|token| token.kind() == TokenKind::DocComment)
                .map(|token| {
                    token
                        .text()
                        .strip_prefix("///")
                        .unwrap()
                        .strip_prefix(' ')
                        .unwrap_or(&token.text()[3..])
                })
                .collect::<Vec<_>>()
                .join("\n");
            if let Some(doc) = DocComment::new(text, range) {
                owners[owner].doc = Some(doc);
            } else {
                diagnostics.push(
                    Diagnostic::error(
                        codes::SYNTAX_ERROR,
                        "declaration documentation exceeds the 16384-byte limit",
                    )
                    .with_span(Span {
                        file: file.to_owned(),
                        start: range.start(),
                        end: range.end(),
                    }),
                );
            }
        }
        for (offset, token) in tokens[start..=end]
            .iter()
            .filter(|token| matches!(token.kind(), TokenKind::LineComment | TokenKind::DocComment))
            .enumerate()
        {
            owners[owner].comments.push(CommentTrivia {
                text: token.text().to_owned(),
                range: token.range(),
                position: match position {
                    CommentPosition::Gap { token, inline } => CommentPosition::Gap {
                        token,
                        inline: inline && offset == 0,
                    },
                    other => other,
                },
                detached_doc: token.kind() == TokenKind::DocComment && !attached,
            });
        }
    }
    for owner in &mut owners {
        owner.comments.sort_by_key(|comment| comment.range.start());
        if owner.comments.is_empty() {
            owner.tokens.clear();
        }
    }
    let mut owners = owners.into_iter();
    document.visit_comments_mut(|_, comments| *comments = owners.next().unwrap());
    document.comments = owners.next().unwrap();
}

fn comment_groups(tokens: &[Token], source: &str) -> Vec<(usize, usize)> {
    let mut groups = Vec::new();
    let mut index = 0;
    while index < tokens.len() {
        let token = &tokens[index];
        if !matches!(token.kind(), TokenKind::LineComment | TokenKind::DocComment) {
            index += 1;
            continue;
        }
        let start = index;
        let mut end = index;
        let before = &source[..token.range().start() as usize];
        let inline = !before
            .rsplit('\n')
            .next()
            .unwrap_or_default()
            .trim()
            .is_empty();
        if token.kind() == TokenKind::DocComment && !inline {
            while let (Some(space), Some(next)) = (tokens.get(end + 1), tokens.get(end + 2)) {
                if space.kind() != TokenKind::Whitespace
                    || space.text().matches('\n').count() != 1
                    || next.kind() != TokenKind::DocComment
                {
                    break;
                }
                end += 2;
            }
        }
        groups.push((start, end));
        index = end + 1;
    }
    groups
}

#[cfg(test)]
mod tests {
    use crate::{Item, format, parse};

    #[test]
    fn same_gap_keeps_previous_child_trailing_before_enclosing_detached_docs() {
        let source = "model M {\nfield a:1=0; // tail a\n/// detached\n\nfield b:1=0;\n}\n";
        let document = parse("docs.eqi", source).into_document().unwrap();
        let text = format(&document);
        let mut reparsed = parse("formatted.eqi", &text).into_document().unwrap();
        reparsed.models[0].items.remove(0);
        let without_a = format(&reparsed);
        assert!(!without_a.contains("// tail a"), "{without_a}");
        assert!(without_a.contains("/// detached"));
    }

    #[test]
    fn ordinary_prose_before_detached_docs_remains_in_the_enclosing_scope() {
        let source = "model M {\n// context\n/// detached\n\n/// B.\nfield b:1=0;\n}\n";
        let document = parse("docs.eqi", source).into_document().unwrap();
        let text = format(&document);
        assert!(text.find("// context").unwrap() < text.find("/// detached").unwrap());
        let mut reparsed = parse("formatted.eqi", &text).into_document().unwrap();
        let Item::Field(field) = &reparsed.models[0].items[0] else {
            panic!("Field")
        };
        assert_eq!(reparsed.doc_comment(field.range).unwrap().summary(), "B.");
        reparsed.models[0].items.clear();
        let without_b = format(&reparsed);
        assert!(without_b.contains("// context"));
        assert!(without_b.contains("/// detached"));
        assert!(!without_b.contains("/// B."));
    }

    #[test]
    fn edited_owner_preserves_detached_gaps_and_exact_child_comments() {
        let source = "model M {\n/// A.\nfield a:1=0;\n/// detached\n\nfield b:1=0;\n/// C.\nfield c:1=0;\n// footer\n}\n";
        let mut document = parse("docs.eqi", source).into_document().unwrap();
        document.models[0].items.remove(1);
        document.models[0].items.swap(0, 1);
        let Item::Field(field) = &mut document.models[0].items[1] else {
            panic!("Field")
        };
        field.name = "renamed".to_owned();
        let text = format(&document);
        assert!(text.contains("/// detached\n\n"));
        assert!(text.contains("// footer\n}"));
        let parsed = parse("edited.eqi", &text).into_document().unwrap();
        for item in &parsed.models[0].items {
            let Item::Field(field) = item else {
                panic!("Field")
            };
            let expected = if field.name == "c" { "C." } else { "A." };
            assert_eq!(parsed.doc_comment(field.range).unwrap().summary(), expected);
        }
        assert_eq!(format(&parsed), text);
    }
}
