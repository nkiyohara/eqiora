//! Byte-accurate lexer retaining trivia and invalid source fragments.

use eqiora_core::diagnostic::codes;
use eqiora_core::{Diagnostic, Span};

use crate::TextRange;

/// Lexical token class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum TokenKind {
    /// Identifier or keyword; keyword interpretation belongs to the parser.
    Identifier,
    /// Decimal floating-point or integer literal.
    Number,
    /// Spaces, tabs, or newlines.
    Whitespace,
    /// `//` through the end of the line.
    LineComment,
    /// `{`.
    LeftBrace,
    /// `}`.
    RightBrace,
    /// `(`.
    LeftParen,
    /// `)`.
    RightParen,
    /// `[`.
    LeftBracket,
    /// `]`.
    RightBracket,
    /// `:`.
    Colon,
    /// `;`.
    Semicolon,
    /// `,`.
    Comma,
    /// `.` in a qualified name.
    Dot,
    /// `=`.
    Equal,
    /// `+`.
    Plus,
    /// `-`.
    Minus,
    /// `*`.
    Star,
    /// `/`.
    Slash,
    /// `^`.
    Caret,
    /// `->`.
    Arrow,
    /// Source fragment that is not part of language v0.
    Error,
    /// Synthetic end marker.
    Eof,
}

impl TokenKind {
    /// Whether the token carries no syntax by itself.
    #[must_use]
    pub const fn is_trivia(self) -> bool {
        matches!(self, Self::Whitespace | Self::LineComment)
    }
}

/// One lossless source token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    kind: TokenKind,
    text: String,
    range: TextRange,
}

impl Token {
    /// Token class.
    #[must_use]
    pub const fn kind(&self) -> TokenKind {
        self.kind
    }

    /// Exact source text.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Exact byte range.
    #[must_use]
    pub const fn range(&self) -> TextRange {
        self.range
    }
}

/// Lossless tokens plus lexical diagnostics.
#[derive(Debug, Clone, PartialEq)]
pub struct LexResult {
    tokens: Vec<Token>,
    diagnostics: Vec<Diagnostic>,
}

impl LexResult {
    /// Tokens including trivia and EOF.
    #[must_use]
    pub fn tokens(&self) -> &[Token] {
        &self.tokens
    }

    /// Every lexical diagnostic in source order.
    #[must_use]
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    pub(crate) fn into_parts(self) -> (Vec<Token>, Vec<Diagnostic>) {
        (self.tokens, self.diagnostics)
    }
}

/// Tokenize one UTF-8 source file without discarding bytes.
#[must_use]
pub fn lex(file: impl Into<String>, source: &str) -> LexResult {
    let file = file.into();
    let mut tokens = Vec::new();
    let mut diagnostics = Vec::new();
    let mut offset = 0_usize;

    while offset < source.len() {
        let bytes = source.as_bytes();
        let start = offset;
        let kind = match bytes[offset] {
            byte if byte.is_ascii_whitespace() => {
                offset += 1;
                while offset < bytes.len() && bytes[offset].is_ascii_whitespace() {
                    offset += 1;
                }
                TokenKind::Whitespace
            }
            b'/' if bytes.get(offset + 1) == Some(&b'/') => {
                offset += 2;
                while offset < bytes.len() && bytes[offset] != b'\n' {
                    offset += 1;
                }
                TokenKind::LineComment
            }
            byte if byte.is_ascii_alphabetic() || byte == b'_' => {
                offset += 1;
                while offset < bytes.len()
                    && (bytes[offset].is_ascii_alphanumeric() || bytes[offset] == b'_')
                {
                    offset += 1;
                }
                TokenKind::Identifier
            }
            byte if byte.is_ascii_digit() => {
                offset = scan_number(source, offset);
                TokenKind::Number
            }
            b'-' if bytes.get(offset + 1) == Some(&b'>') => {
                offset += 2;
                TokenKind::Arrow
            }
            b'{' => single(&mut offset, TokenKind::LeftBrace),
            b'}' => single(&mut offset, TokenKind::RightBrace),
            b'(' => single(&mut offset, TokenKind::LeftParen),
            b')' => single(&mut offset, TokenKind::RightParen),
            b'[' => single(&mut offset, TokenKind::LeftBracket),
            b']' => single(&mut offset, TokenKind::RightBracket),
            b':' => single(&mut offset, TokenKind::Colon),
            b';' => single(&mut offset, TokenKind::Semicolon),
            b',' => single(&mut offset, TokenKind::Comma),
            b'.' => single(&mut offset, TokenKind::Dot),
            b'=' => single(&mut offset, TokenKind::Equal),
            b'+' => single(&mut offset, TokenKind::Plus),
            b'-' => single(&mut offset, TokenKind::Minus),
            b'*' => single(&mut offset, TokenKind::Star),
            b'/' => single(&mut offset, TokenKind::Slash),
            b'^' => single(&mut offset, TokenKind::Caret),
            _ => {
                let width = source[offset..].chars().next().map_or(1, char::len_utf8);
                offset += width;
                diagnostics.push(source_error(
                    &file,
                    start,
                    offset,
                    format!("invalid Eqiora Language token {:?}", &source[start..offset]),
                ));
                TokenKind::Error
            }
        };
        tokens.push(Token {
            kind,
            text: source[start..offset].to_owned(),
            range: range(start, offset),
        });
    }
    tokens.push(Token {
        kind: TokenKind::Eof,
        text: String::new(),
        range: range(source.len(), source.len()),
    });
    LexResult {
        tokens,
        diagnostics,
    }
}

fn scan_number(source: &str, mut offset: usize) -> usize {
    let bytes = source.as_bytes();
    while offset < bytes.len() && bytes[offset].is_ascii_digit() {
        offset += 1;
    }
    if bytes.get(offset) == Some(&b'.') && bytes.get(offset + 1).is_some_and(u8::is_ascii_digit) {
        offset += 1;
        while offset < bytes.len() && bytes[offset].is_ascii_digit() {
            offset += 1;
        }
    }
    if matches!(bytes.get(offset), Some(b'e' | b'E')) {
        let exponent_start = offset;
        offset += 1;
        if matches!(bytes.get(offset), Some(b'+' | b'-')) {
            offset += 1;
        }
        let digits_start = offset;
        while offset < bytes.len() && bytes[offset].is_ascii_digit() {
            offset += 1;
        }
        if offset == digits_start {
            return exponent_start;
        }
    }
    offset
}

fn single(offset: &mut usize, kind: TokenKind) -> TokenKind {
    *offset += 1;
    kind
}

fn source_error(file: &str, start: usize, end: usize, message: String) -> Diagnostic {
    Diagnostic::error(codes::INVALID_TOKEN, message).with_span(Span {
        file: file.to_owned(),
        start: u32::try_from(start).unwrap_or(u32::MAX),
        end: u32::try_from(end).unwrap_or(u32::MAX),
    })
}

fn range(start: usize, end: usize) -> TextRange {
    TextRange::new(
        u32::try_from(start).unwrap_or(u32::MAX),
        u32::try_from(end).unwrap_or(u32::MAX),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lexer_retains_every_source_byte() {
        let source = "model thermal { // state\n field t: K = 293.0; }";
        let result = lex("thermal.eqi", source);
        let reconstructed = result.tokens().iter().map(Token::text).collect::<String>();

        assert_eq!(reconstructed, source);
        assert!(result.diagnostics().is_empty());
        assert!(
            result
                .tokens()
                .iter()
                .any(|token| token.kind() == TokenKind::LineComment)
        );
    }

    #[test]
    fn lexer_distinguishes_qualified_names_from_decimal_points() {
        let source = "connect conserving drive.motor.positive, 1.25;";
        let result = lex("qualified.eqi", source);
        let dots = result
            .tokens()
            .iter()
            .filter(|token| token.kind() == TokenKind::Dot)
            .count();

        assert_eq!(dots, 2);
        assert!(result.diagnostics().is_empty());
        assert!(
            result
                .tokens()
                .iter()
                .any(|token| { token.kind() == TokenKind::Number && token.text() == "1.25" })
        );
    }

    #[test]
    fn lexer_retains_exact_value_shape_delimiters() {
        let source = "field velocity: m / s shape [2, 3];";
        let result = lex("shape.eqi", source);

        assert!(result.diagnostics().is_empty());
        assert!(
            result
                .tokens()
                .iter()
                .any(|token| { token.kind() == TokenKind::LeftBracket && token.text() == "[" })
        );
        assert!(
            result
                .tokens()
                .iter()
                .any(|token| { token.kind() == TokenKind::RightBracket && token.text() == "]" })
        );
        assert_eq!(
            result.tokens().iter().map(Token::text).collect::<String>(),
            source
        );
    }
}
