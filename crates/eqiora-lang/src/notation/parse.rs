use super::{
    Notation, NotationAccent, NotationAtom, NotationError, NotationMark, NotationNode,
    NotationStyle,
};
use crate::TextRange;

pub(super) fn parse(input: &str) -> Result<Notation, NotationError> {
    let error = |message| NotationError {
        range: TextRange::new(0, input.len().min(u32::MAX as usize) as u32),
        message,
    };
    // Check raw bytes before tokenization, recursion or AST allocation.
    if input.len() > Notation::MAX_BYTES {
        return Err(error("notation exceeds the 1024-byte limit"));
    }
    if !input.starts_with("@{") || !input.ends_with('}') {
        return Err(error("notation requires one complete `@{...}` island"));
    }
    let mut parser = Parser {
        input,
        cursor: 2,
        tokens: 0,
        script_atoms: 0,
    };
    let root = parser.node(1, false)?;
    parser.expect(b'}')?;
    if parser.cursor != input.len() {
        return Err(parser.error("notation contains more than one symbol"));
    }
    // MAX_TOKENS and MAX_DEPTH already bound this walk and its arithmetic.
    let emitted_bytes = 3 + root.emitted_bytes();
    if emitted_bytes > Notation::MAX_EMITTED_BYTES {
        return Err(error("canonical notation exceeds the 4096-byte limit"));
    }
    Ok(Notation {
        root,
        range: TextRange::new(0, input.len() as u32),
        emitted_bytes,
    })
}

struct Parser<'a> {
    input: &'a str,
    cursor: usize,
    tokens: usize,
    script_atoms: usize,
}

impl Parser<'_> {
    fn error(&self, message: &'static str) -> NotationError {
        let end = self.cursor
            + self.input[self.cursor..]
                .chars()
                .next()
                .map_or(0, char::len_utf8);
        NotationError {
            range: TextRange::new(self.cursor as u32, end as u32),
            message,
        }
    }

    fn peek(&mut self) -> Option<u8> {
        while self
            .input
            .as_bytes()
            .get(self.cursor)
            .is_some_and(u8::is_ascii_whitespace)
        {
            self.cursor += 1;
        }
        self.input.as_bytes().get(self.cursor).copied()
    }

    fn token(&mut self) -> Result<(), NotationError> {
        if self.tokens == Notation::MAX_TOKENS {
            return Err(self.error("notation exceeds the 256-token limit"));
        }
        self.tokens += 1;
        Ok(())
    }

    fn expect(&mut self, byte: u8) -> Result<(), NotationError> {
        if self.peek() != Some(byte) {
            return Err(self.error("expected a notation group delimiter"));
        }
        self.token()?;
        self.cursor += 1;
        Ok(())
    }

    fn node(&mut self, depth: usize, script: bool) -> Result<NotationNode, NotationError> {
        self.peek();
        if depth > Notation::MAX_DEPTH {
            return Err(self.error("notation exceeds the 8-level depth limit"));
        }
        if script {
            if self.script_atoms == Notation::MAX_SCRIPT_ATOMS {
                return Err(self.error("notation exceeds the 32-script-atom limit"));
            }
            self.script_atoms += 1;
        }
        let mut node = self.base(depth, script)?;
        let mut subscript = Vec::new();
        let mut superscript = Vec::new();
        while let Some(marker @ (b'_' | b'^' | b'\'')) = self.peek() {
            if matches!(node, NotationNode::Scripted { .. } | NotationNode::Mark(_)) {
                return Err(self.error(
                    "intrinsic scripts cannot be applied twice to the same symbol or to a mark",
                ));
            }
            self.token()?;
            self.cursor += 1;
            if marker == b'\'' {
                if !superscript.is_empty() {
                    return Err(self.error("duplicate intrinsic superscript"));
                }
                loop {
                    if self.script_atoms == Notation::MAX_SCRIPT_ATOMS {
                        return Err(self.error("notation exceeds the 32-script-atom limit"));
                    }
                    self.script_atoms += 1;
                    superscript.push(NotationNode::Mark(NotationMark::Prime));
                    if self.peek() != Some(b'\'') {
                        break;
                    }
                    self.token()?;
                    self.cursor += 1;
                }
            } else {
                let destination = if marker == b'_' {
                    &mut subscript
                } else {
                    &mut superscript
                };
                if !destination.is_empty() {
                    return Err(self.error("duplicate intrinsic script"));
                }
                *destination = self.script(depth + 1)?;
            }
        }
        if !subscript.is_empty() || !superscript.is_empty() {
            node = NotationNode::Scripted {
                base: Box::new(node),
                subscript,
                superscript,
            };
        }
        Ok(node)
    }

    fn script(&mut self, depth: usize) -> Result<Vec<NotationNode>, NotationError> {
        if self.peek() != Some(b'{') {
            // An unbraced script is exactly one atom; its following script belongs
            // to the outer symbol, as in x_i^2.
            if depth > Notation::MAX_DEPTH {
                return Err(self.error("notation exceeds the 8-level depth limit"));
            }
            if self.script_atoms == Notation::MAX_SCRIPT_ATOMS {
                return Err(self.error("notation exceeds the 32-script-atom limit"));
            }
            self.script_atoms += 1;
            return Ok(vec![self.base(depth, true)?]);
        }
        self.expect(b'{')?;
        let mut nodes = Vec::new();
        while self.peek() != Some(b'}') {
            nodes.push(self.node(depth, true)?);
        }
        if nodes.is_empty() {
            return Err(self.error("intrinsic script must not be empty"));
        }
        self.expect(b'}')?;
        Ok(nodes)
    }

    fn base(&mut self, depth: usize, script: bool) -> Result<NotationNode, NotationError> {
        let byte = self
            .peek()
            .ok_or_else(|| self.error("expected a notation symbol"))?;
        self.token()?;
        match byte {
            b'a'..=b'z' | b'A'..=b'Z' => {
                self.cursor += 1;
                Ok(NotationNode::Atom(NotationAtom::Latin(char::from(byte))))
            }
            b'0'..=b'9' if script => {
                self.cursor += 1;
                Ok(NotationNode::Atom(NotationAtom::Digit(byte - b'0')))
            }
            b'+' | b'-' | b'*' | b'\'' if script => {
                self.cursor += 1;
                Ok(NotationNode::Mark(match byte {
                    b'+' => NotationMark::Plus,
                    b'-' => NotationMark::Minus,
                    b'*' => NotationMark::Star,
                    _ => NotationMark::Prime,
                }))
            }
            b'{' => {
                self.cursor += 1;
                let node = self.node(depth + 1, script)?;
                self.expect(b'}')?;
                Ok(node)
            }
            b'\\' => {
                let start = self.cursor;
                self.cursor += 1;
                let command_start = self.cursor;
                while self
                    .input
                    .as_bytes()
                    .get(self.cursor)
                    .is_some_and(u8::is_ascii_alphabetic)
                {
                    self.cursor += 1;
                }
                let command = &self.input[command_start..self.cursor];
                if let Some(atom) = NotationAtom::command(command) {
                    return Ok(NotationNode::Atom(atom));
                }
                if script && let Some(mark) = NotationMark::command(command) {
                    return Ok(NotationNode::Mark(mark));
                }
                let style = NotationStyle::command(command);
                let accent = NotationAccent::command(command);
                if style.is_none() && accent.is_none() {
                    return Err(NotationError {
                        range: TextRange::new(start as u32, self.cursor as u32),
                        message: "unknown or forbidden notation control sequence",
                    });
                }
                self.expect(b'{')?;
                let inner = Box::new(self.node(depth + 1, script)?);
                self.expect(b'}')?;
                Ok(if let Some(style) = style {
                    NotationNode::Styled(style, inner)
                } else {
                    NotationNode::Accented(accent.expect("admitted accent"), inner)
                })
            }
            _ => Err(self.error(
                "expected an admitted notation symbol; text, layout and expressions are forbidden",
            )),
        }
    }
}
