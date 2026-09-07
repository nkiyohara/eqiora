//! Declaration documentation and source-owned comment trivia.

use super::TextRange;

/// Source provenance owned by one syntax node, never a whole-source replay.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct SourceComments {
    pub(crate) range: TextRange,
    pub(crate) doc: Option<DocComment>,
    pub(crate) comments: Vec<CommentTrivia>,
    pub(crate) tokens: Vec<(SyntaxAnchor, TextRange)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SyntaxAnchor {
    Token(crate::lexer::TokenKind),
    Child(TextRange),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CommentTrivia {
    pub(crate) text: String,
    pub(crate) range: TextRange,
    pub(crate) position: CommentPosition,
    pub(crate) detached_doc: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CommentPosition {
    Leading,
    Trailing,
    Gap { token: usize, inline: bool },
}

mod visit;

/// Documentation attached to one declaration or signature entry in its source.
///
/// The range refers to the original UTF-8 source, including the `///` markers.
/// Cloning a declaration preserves that provenance; formatting does not retarget it
/// to the generated document. Parse the generated source to obtain its ranges.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocComment {
    text: String,
    range: TextRange,
}

impl DocComment {
    pub(crate) const MAX_BYTES: usize = 16_384;

    pub(crate) fn new(text: String, range: TextRange) -> Option<Self> {
        (text.len() <= Self::MAX_BYTES).then_some(Self { text, range })
    }

    /// Documentation prose without comment markers.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Original source range, not a position in subsequently formatted source.
    #[must_use]
    pub const fn range(&self) -> TextRange {
        self.range
    }

    /// First nonempty paragraph, with whitespace between its lines normalized.
    #[must_use]
    pub fn summary(&self) -> String {
        self.text
            .lines()
            .skip_while(|line| line.trim().is_empty())
            .take_while(|line| !line.trim().is_empty())
            .map(str::trim)
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Safe Markdown paragraphs with optional emphasis.
    ///
    /// Links, images, HTML, directives, headings and fenced code are rendered
    /// literally. Control characters are removed and indentation is normalized,
    /// so source prose cannot introduce a renderer command or active URL.
    #[must_use]
    pub fn markdown(&self) -> String {
        self.text
            .lines()
            .map(|line| {
                let normalized: String = line
                    .chars()
                    .filter_map(|character| {
                        if character == '\t' {
                            Some(' ')
                        } else if character.is_control() {
                            None
                        } else {
                            Some(character)
                        }
                    })
                    .collect();
                let line = normalized.as_str();
                let line = line.trim();
                let ordered_marker = line.bytes().take_while(u8::is_ascii_digit).count();
                let thematic = line
                    .chars()
                    .all(|character| matches!(character, '*' | '_' | ' ' | '\t'));
                let list = line
                    .strip_prefix('*')
                    .is_some_and(|rest| rest.starts_with(char::is_whitespace));
                let mut rendered = String::new();
                for (index, character) in line.chars().enumerate() {
                    if character.is_control() {
                        if character == '\t' {
                            rendered.push(' ');
                        }
                        continue;
                    }
                    // Entity expansion happens after Markdown link recognition;
                    // bare URLs, email addresses and numbered-list markers stay
                    // text even in renderers with GFM autolinking enabled.
                    if let Some(entity) = match character {
                        '&' => Some("&amp;"),
                        ':' => Some("&#58;"),
                        '.' => Some("&#46;"),
                        '@' => Some("&#64;"),
                        _ => None,
                    } {
                        rendered.push_str(entity);
                        continue;
                    }
                    if matches!(
                        character,
                        '\\' | '`'
                            | '['
                            | ']'
                            | '<'
                            | '>'
                            | '#'
                            | '!'
                            | '+'
                            | '-'
                            | '='
                            | '~'
                            | '|'
                    ) || (thematic && matches!(character, '*' | '_'))
                        || (list && index == 0)
                        || (character == ')' && ordered_marker != 0 && index == ordered_marker)
                    {
                        rendered.push('\\');
                    }
                    rendered.push(character);
                }
                rendered
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_uses_first_nonempty_paragraph_and_original_utf8_range() {
        let range = TextRange::new(7, 91);
        let doc = DocComment::new(
            "\n  温度の変化。\n  Second line.\n\nFurther prose.".into(),
            range,
        )
        .unwrap();
        assert_eq!(doc.summary(), "温度の変化。 Second line.");
        assert_eq!(doc.range(), range);
    }

    #[test]
    fn unsafe_markdown_is_literal_and_emphasis_remains_available() {
        let doc = DocComment::new(
            "**Summary**\n\n<script>alert(1)</script>\n[run](command:delete)\n[x](javascript:alert(1))\n```html\n<img src=x>\n```\n\\[escape](command:run)".into(),
            TextRange::default(),
        )
        .unwrap();
        assert_eq!(
            doc.markdown(),
            "**Summary**\n\n\\<script\\>alert(1)\\</script\\>\n\\[run\\](command&#58;delete)\n\\[x\\](javascript&#58;alert(1))\n\\`\\`\\`html\n\\<img src\\=x\\>\n\\`\\`\\`\n\\\\\\[escape\\](command&#58;run)"
        );
    }

    #[test]
    fn bound_counts_utf8_bytes_not_characters() {
        assert!(DocComment::new("é".repeat(8192), TextRange::default()).is_some());
        assert!(DocComment::new("é".repeat(8193), TextRange::default()).is_none());
    }

    #[test]
    fn autolinks_lists_and_thematic_breaks_are_literal_markdown() {
        let doc = DocComment::new(
            "https://example.test www.example.test a@example.test\n* item\n___\n1. numbered\n**Summary**".into(),
            TextRange::default(),
        ).unwrap();
        assert_eq!(
            doc.markdown(),
            "https&#58;//example&#46;test www&#46;example&#46;test a&#64;example&#46;test\n\\* item\n\\_\\_\\_\n1&#46; numbered\n**Summary**"
        );
    }

    #[test]
    fn control_prefixed_and_parenthesized_markers_stay_literal() {
        let doc =
            DocComment::new("\0* item\n\0___\n1) numbered".into(), TextRange::default()).unwrap();
        assert_eq!(doc.markdown(), "\\* item\n\\_\\_\\_\n1\\) numbered");
    }
}
