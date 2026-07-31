mod error;
mod token;

pub use error::LexError;
pub use token::{Span, Token, TokenKind};

type InternMap =
    std::collections::HashSet<std::rc::Rc<str>, std::hash::BuildHasherDefault<FnvHasher>>;

/// Fast FNV-1a hasher for the identifier interner.
struct FnvHasher(u64);

impl Default for FnvHasher {
    fn default() -> Self {
        FnvHasher(0xcbf2_9ce4_8422_2325)
    }
}

impl std::hash::Hasher for FnvHasher {
    fn write(&mut self, bytes: &[u8]) {
        let mut h = self.0;
        for &b in bytes {
            h = (h ^ b as u64).wrapping_mul(0x0000_0100_0000_01b3);
        }
        self.0 = h;
    }

    fn finish(&self) -> u64 {
        self.0
    }
}

pub struct Lexer {
    source: Vec<u8>,
    pos: usize,
    line: usize,
    col: usize,
    col_dirty: bool,
    file: std::rc::Rc<str>,
    interns: InternMap,
}

impl Lexer {
    pub fn new(source: &str, file: impl Into<String>) -> Self {
        let mut bytes = source.as_bytes().to_vec();
        if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
            bytes.drain(..3);
        }
        Lexer {
            source: bytes,
            pos: 0,
            line: 1,
            col: 1,
            col_dirty: false,
            file: std::rc::Rc::from(file.into()),
            interns: std::collections::HashSet::with_hasher(Default::default()),
        }
    }

    #[cfg(test)]
    fn is_at_end(&self) -> bool {
        self.pos >= self.source.len()
    }

    fn char_at(&self, p: usize) -> char {
        if p >= self.source.len() {
            '\0'
        } else {
            let b = self.source[p];
            if b < 0x80 {
                b as char
            } else {
                std::str::from_utf8(&self.source[p..])
                    .ok()
                    .and_then(|s| s.chars().next())
                    .unwrap_or('\u{FFFD}')
            }
        }
    }

    // ── Char-based helpers (kept for tests / future phases) ──

    #[cfg(test)]
    fn current(&self) -> char {
        self.char_at(self.pos)
    }

    #[cfg(test)]
    fn peek(&self) -> char {
        self.char_at(self.pos + 1)
    }

    #[cfg(test)]
    fn peek2(&self) -> char {
        self.char_at(self.pos + 2)
    }

    #[cfg(test)]
    fn advance(&mut self) -> char {
        let ch = self.char_at(self.pos);
        self.pos += ch.len_utf8();
        self.col += 1;
        ch
    }

    #[cfg(test)]
    fn advance_if(&mut self, expected: char) -> bool {
        if !self.is_at_end() && self.current() == expected {
            self.pos += expected.len_utf8();
            self.col += 1;
            true
        } else {
            false
        }
    }

    #[cfg(test)]
    fn advance_while<F: Fn(char) -> bool>(&mut self, predicate: F) {
        while !self.is_at_end() && predicate(self.current()) {
            self.advance();
        }
    }

    fn make_token(&self, kind: TokenKind, span: Span) -> Token {
        Token::new(kind, span, self.file.clone())
    }

    #[cfg(test)]
    fn is_digit(ch: char) -> bool {
        ch.is_ascii_digit()
    }

    #[cfg(test)]
    fn is_alpha(ch: char) -> bool {
        ch.is_ascii_alphabetic() || ch == '_'
    }

    #[cfg(test)]
    fn is_alphanumeric(ch: char) -> bool {
        ch.is_ascii_alphanumeric() || ch == '_'
    }

    #[cfg(test)]
    fn is_whitespace(ch: char) -> bool {
        matches!(ch, ' ' | '\t' | '\r')
    }

    #[cfg(test)]
    fn skip_whitespace(&mut self) {
        while !self.is_at_end() && Self::is_whitespace(self.current()) {
            self.advance();
        }
    }

    // ── Byte-based helpers (hot path) ────────────────────────

    fn skip_whitespace_byte(&mut self) {
        while self.pos < self.source.len() {
            match self.source[self.pos] {
                b' ' | b'\t' | b'\r' => {
                    self.pos += 1;
                    self.col += 1;
                }
                _ => break,
            }
        }
    }

    fn current_byte(&self) -> u8 {
        if self.pos >= self.source.len() {
            0
        } else {
            self.source[self.pos]
        }
    }

    fn peek_byte(&self) -> u8 {
        if self.pos + 1 >= self.source.len() {
            0
        } else {
            self.source[self.pos + 1]
        }
    }

    fn advance_byte(&mut self) -> u8 {
        let b = self.source[self.pos];
        self.pos += 1;
        b
    }

    fn advance_if_byte(&mut self, expected: u8) -> bool {
        if self.pos < self.source.len() && self.source[self.pos] == expected {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn advance_char(&mut self) -> char {
        let ch = std::str::from_utf8(&self.source[self.pos..])
            .ok()
            .and_then(|s| s.chars().next())
            .unwrap_or('\u{FFFD}');
        self.pos += ch.len_utf8();
        self.col += 1;
        ch
    }

    fn sync_col(&mut self, start: usize, start_col: usize) {
        if self.col_dirty {
            let count = std::str::from_utf8(&self.source[start..self.pos])
                .unwrap()
                .chars()
                .count();
            self.col = start_col + count;
            self.col_dirty = false;
        } else {
            self.col = start_col + (self.pos - start);
        }
    }

    fn finish_span(&mut self, start: usize, line: usize, col: usize) -> Span {
        let end = self.pos;
        self.sync_col(start, col);
        Span::new(start, end, line, col)
    }

    fn skip_comments(&mut self) -> Result<bool, LexError> {
        let mut skipped = false;

        loop {
            if self.current_byte() == b'-' && self.peek_byte() == b'-' {
                let start = self.pos;
                let start_col = self.col;
                self.pos += 2;
                while self.pos < self.source.len() && self.source[self.pos] != b'\n' {
                    if self.source[self.pos] >= 0x80 {
                        self.col_dirty = true;
                    }
                    self.pos += 1;
                }
                self.sync_col(start, start_col);
                skipped = true;
                continue;
            }

            if self.current_byte() == b'/' && self.peek_byte() == b'-' {
                let start = self.pos;
                let start_line = self.line;
                let start_col = self.col;
                let mut col = self.col;

                self.pos += 2;
                col += 2;

                let mut closed = false;
                while self.pos < self.source.len() {
                    let b = self.source[self.pos];
                    if b == b'-'
                        && self.pos + 1 < self.source.len()
                        && self.source[self.pos + 1] == b'/'
                    {
                        self.pos += 2;
                        col += 2;
                        closed = true;
                        break;
                    }
                    if b == b'\n' {
                        self.line += 1;
                        col = 1;
                        self.pos += 1;
                    } else if b >= 0x80 {
                        self.col_dirty = true;
                        let clen = if b >= 0xF0 {
                            4
                        } else if b >= 0xE0 {
                            3
                        } else if b >= 0xC0 {
                            2
                        } else {
                            1
                        };
                        self.pos += clen;
                        col += 1;
                    } else {
                        self.pos += 1;
                        col += 1;
                    }
                }

                if !closed {
                    return Err(LexError::UnterminatedComment {
                        span: Span::new(start, self.pos, start_line, start_col),
                        file: std::rc::Rc::clone(&self.file),
                    });
                }

                self.col = col;
                skipped = true;
                continue;
            }

            break;
        }

        Ok(skipped)
    }

    fn lex_number(&mut self) -> Result<Token, LexError> {
        let start = self.pos;
        let start_line = self.line;
        let start_col = self.col;

        if self.current_byte() == b'0' {
            match self.peek_byte() {
                b'x' | b'X' => return self.lex_hex(start, start_line, start_col),
                b'b' | b'B' => return self.lex_binary(start, start_line, start_col),
                b'o' | b'O' => return self.lex_octal(start, start_line, start_col),
                _ => {}
            }
        }

        let mut val: i64 = 0;
        while self.pos < self.source.len() {
            let b = self.source[self.pos];
            if b == b'_' {
                self.pos += 1;
                continue;
            }
            if !b.is_ascii_digit() {
                break;
            }
            match val
                .checked_mul(10)
                .and_then(|v| v.checked_add((b - b'0') as i64))
            {
                Some(v) => val = v,
                None => {
                    while self.pos < self.source.len()
                        && (self.source[self.pos].is_ascii_digit() || self.source[self.pos] == b'_')
                    {
                        self.pos += 1;
                    }
                    let raw = self.number_raw(start);
                    let span = self.finish_span(start, start_line, start_col);
                    return Err(LexError::NumberOverflow {
                        raw,
                        span,
                        file: std::rc::Rc::clone(&self.file),
                    });
                }
            }
            self.pos += 1;
        }

        if self.current_byte() == b'.' && self.peek_byte().is_ascii_digit() {
            self.pos += 1;
            while self.pos < self.source.len() {
                let b = self.source[self.pos];
                if b == b'_' {
                    self.pos += 1;
                    continue;
                }
                if !b.is_ascii_digit() {
                    break;
                }
                self.pos += 1;
            }

            if matches!(self.current_byte(), b'e' | b'E') {
                self.pos += 1;
                if matches!(self.current_byte(), b'+' | b'-') {
                    self.pos += 1;
                }
                while self.pos < self.source.len() && self.source[self.pos].is_ascii_digit() {
                    self.pos += 1;
                }
            }

            let span = self.finish_span(start, start_line, start_col);
            let raw = self.number_raw(start);
            let val: f64 = raw.parse().map_err(|_| LexError::NumberOverflow {
                raw,
                span,
                file: std::rc::Rc::clone(&self.file),
            })?;
            return Ok(self.make_token(TokenKind::FloatLiteral(val), span));
        }

        let span = self.finish_span(start, start_line, start_col);
        Ok(self.make_token(TokenKind::IntLiteral(val), span))
    }

    fn number_raw(&self, start: usize) -> String {
        self.source[start..self.pos]
            .iter()
            .copied()
            .filter(|&b| b != b'_')
            .map(char::from)
            .collect()
    }

    fn lex_hex(&mut self, start: usize, line: usize, col: usize) -> Result<Token, LexError> {
        self.pos += 2;

        let mut val: i64 = 0;
        let mut any = false;
        while self.pos < self.source.len() {
            let b = self.source[self.pos];
            if b == b'_' {
                self.pos += 1;
                continue;
            }
            let digit = match b {
                b'0'..=b'9' => (b - b'0') as i64,
                b'a'..=b'f' => (b - b'a' + 10) as i64,
                b'A'..=b'F' => (b - b'A' + 10) as i64,
                _ => break,
            };
            match val.checked_mul(16).and_then(|v| v.checked_add(digit)) {
                Some(v) => val = v,
                None => {
                    while self.pos < self.source.len()
                        && (self.source[self.pos].is_ascii_hexdigit()
                            || self.source[self.pos] == b'_')
                    {
                        self.pos += 1;
                    }
                    let span = self.finish_span(start, line, col);
                    let raw = format!("0x{}", self.number_raw(start + 2));
                    return Err(LexError::NumberOverflow {
                        raw,
                        span,
                        file: std::rc::Rc::clone(&self.file),
                    });
                }
            }
            any = true;
            self.pos += 1;
        }

        let span = self.finish_span(start, line, col);
        if !any {
            return Err(LexError::NumberOverflow {
                raw: "0x".to_string(),
                span,
                file: std::rc::Rc::clone(&self.file),
            });
        }
        Ok(self.make_token(TokenKind::IntLiteral(val), span))
    }

    fn lex_binary(&mut self, start: usize, line: usize, col: usize) -> Result<Token, LexError> {
        self.pos += 2;

        let mut val: i64 = 0;
        let mut any = false;
        while self.pos < self.source.len() {
            let b = self.source[self.pos];
            if b == b'_' {
                self.pos += 1;
                continue;
            }
            if b != b'0' && b != b'1' {
                break;
            }
            match val
                .checked_mul(2)
                .and_then(|v| v.checked_add((b - b'0') as i64))
            {
                Some(v) => val = v,
                None => {
                    while self.pos < self.source.len()
                        && matches!(self.source[self.pos], b'0' | b'1' | b'_')
                    {
                        self.pos += 1;
                    }
                    let span = self.finish_span(start, line, col);
                    let raw = format!("0b{}", self.number_raw(start + 2));
                    return Err(LexError::NumberOverflow {
                        raw,
                        span,
                        file: std::rc::Rc::clone(&self.file),
                    });
                }
            }
            any = true;
            self.pos += 1;
        }

        let span = self.finish_span(start, line, col);
        if !any {
            return Err(LexError::NumberOverflow {
                raw: "0b".to_string(),
                span,
                file: std::rc::Rc::clone(&self.file),
            });
        }
        Ok(self.make_token(TokenKind::IntLiteral(val), span))
    }

    fn lex_octal(&mut self, start: usize, line: usize, col: usize) -> Result<Token, LexError> {
        self.pos += 2;

        let mut val: i64 = 0;
        let mut any = false;
        while self.pos < self.source.len() {
            let b = self.source[self.pos];
            if b == b'_' {
                self.pos += 1;
                continue;
            }
            if !matches!(b, b'0'..=b'7') {
                break;
            }
            match val
                .checked_mul(8)
                .and_then(|v| v.checked_add((b - b'0') as i64))
            {
                Some(v) => val = v,
                None => {
                    while self.pos < self.source.len()
                        && (matches!(self.source[self.pos], b'0'..=b'7')
                            || self.source[self.pos] == b'_')
                    {
                        self.pos += 1;
                    }
                    let span = self.finish_span(start, line, col);
                    let raw = format!("0o{}", self.number_raw(start + 2));
                    return Err(LexError::NumberOverflow {
                        raw,
                        span,
                        file: std::rc::Rc::clone(&self.file),
                    });
                }
            }
            any = true;
            self.pos += 1;
        }

        let span = self.finish_span(start, line, col);
        if !any {
            return Err(LexError::NumberOverflow {
                raw: "0o".to_string(),
                span,
                file: std::rc::Rc::clone(&self.file),
            });
        }
        Ok(self.make_token(TokenKind::IntLiteral(val), span))
    }

    fn lex_string(&mut self) -> Result<Token, LexError> {
        let start = self.pos;
        let start_line = self.line;
        let start_col = self.col;

        self.pos += 1;
        self.col += 1;

        let mut value = String::with_capacity(32);

        loop {
            if self.pos >= self.source.len() {
                return Err(LexError::UnterminatedString {
                    span: Span::new(start, self.pos, start_line, start_col),
                    file: std::rc::Rc::clone(&self.file),
                });
            }

            let b = self.source[self.pos];

            if b == b'\n' {
                return Err(LexError::UnterminatedString {
                    span: Span::new(start, self.pos, start_line, start_col),
                    file: std::rc::Rc::clone(&self.file),
                });
            }

            if b == b'"' {
                self.pos += 1;
                self.col += 1;
                break;
            }

            if b == b'\\' {
                self.pos += 1;
                self.col += 1;
                let escaped = self.lex_escape_sequence(start, start_line, start_col)?;
                value.push(escaped);
                continue;
            }

            if b >= 0x80 {
                self.col_dirty = true;
                value.push(self.advance_char());
            } else {
                value.push(b as char);
                self.pos += 1;
                self.col += 1;
            }
        }

        let span = Span::new(start, self.pos, start_line, start_col);
        Ok(self.make_token(TokenKind::StringLiteral(value), span))
    }

    fn lex_escape_sequence(
        &mut self,
        string_start: usize,
        string_line: usize,
        string_col: usize,
    ) -> Result<char, LexError> {
        if self.pos >= self.source.len() {
            return Err(LexError::UnterminatedString {
                span: Span::new(string_start, self.pos, string_line, string_col),
                file: std::rc::Rc::clone(&self.file),
            });
        }

        let escape_col = self.col;
        let escape_line = self.line;
        let ch = self.advance_char();

        match ch {
            'n' => Ok('\n'),
            't' => Ok('\t'),
            'r' => Ok('\r'),
            '\\' => Ok('\\'),
            '"' => Ok('"'),
            '\'' => Ok('\''),
            '0' => Ok('\0'),
            'u' => self.lex_unicode_escape(string_start, string_line, string_col),
            other => Err(LexError::InvalidEscape {
                ch: other,
                span: Span::new(
                    self.pos.saturating_sub(2),
                    self.pos,
                    escape_line,
                    escape_col.saturating_sub(1),
                ),
                file: std::rc::Rc::clone(&self.file),
            }),
        }
    }

    fn lex_unicode_escape(
        &mut self,
        string_start: usize,
        string_line: usize,
        string_col: usize,
    ) -> Result<char, LexError> {
        if !self.advance_if_byte(b'{') {
            return Err(LexError::InvalidEscape {
                ch: 'u',
                span: Span::new(string_start, self.pos, string_line, string_col),
                file: std::rc::Rc::clone(&self.file),
            });
        }

        let mut hex = String::with_capacity(6);
        while self.pos < self.source.len() && self.source[self.pos] != b'}' {
            let b = self.source[self.pos];
            if b.is_ascii_hexdigit() {
                hex.push(self.advance_byte() as char);
            } else {
                return Err(LexError::InvalidEscape {
                    ch: self.char_at(self.pos),
                    span: Span::new(string_start, self.pos, string_line, string_col),
                    file: std::rc::Rc::clone(&self.file),
                });
            }
        }

        if !self.advance_if_byte(b'}') {
            return Err(LexError::UnterminatedString {
                span: Span::new(string_start, self.pos, string_line, string_col),
                file: std::rc::Rc::clone(&self.file),
            });
        }

        let codepoint = u32::from_str_radix(&hex, 16).map_err(|_| LexError::InvalidEscape {
            ch: 'u',
            span: Span::new(string_start, self.pos, string_line, string_col),
            file: std::rc::Rc::clone(&self.file),
        })?;

        char::from_u32(codepoint).ok_or_else(|| LexError::InvalidEscape {
            ch: 'u',
            span: Span::new(string_start, self.pos, string_line, string_col),
            file: std::rc::Rc::clone(&self.file),
        })
    }

    /// Does this token end an expression? If so, a following newline is significant.
    fn ends_expression(kind: &TokenKind) -> bool {
        matches!(
            kind,
            TokenKind::IntLiteral(_)
                | TokenKind::FloatLiteral(_)
                | TokenKind::StringLiteral(_)
                | TokenKind::BoolLiteral(_)
                | TokenKind::CharLiteral(_)
                | TokenKind::Identifier(_)
                | TokenKind::RightParen
                | TokenKind::RightBracket
                | TokenKind::RightBrace
                | TokenKind::Return
                | TokenKind::Break
                | TokenKind::Continue
        )
    }

    // ════════════════════════════════════════
    //   CHAR LITERAL LEXING
    // ════════════════════════════════════════

    fn lex_char(&mut self) -> Result<Token, LexError> {
        let start = self.pos;
        let start_line = self.line;
        let start_col = self.col;

        self.pos += 1; // consume opening '
        self.col += 1;

        if self.pos >= self.source.len() || self.source[self.pos] == b'\n' {
            return Err(LexError::UnterminatedChar {
                span: Span::new(start, self.pos, start_line, start_col),
                file: std::rc::Rc::clone(&self.file),
            });
        }

        let ch = if self.source[self.pos] == b'\\' {
            self.pos += 1; // consume backslash
            self.col += 1;
            self.lex_escape_sequence(start, start_line, start_col)?
        } else if self.source[self.pos] >= 0x80 {
            self.col_dirty = true;
            self.advance_char()
        } else {
            let b = self.source[self.pos];
            self.pos += 1;
            self.col += 1;
            b as char
        };

        if self.pos >= self.source.len() || self.source[self.pos] != b'\'' {
            return Err(LexError::InvalidCharLiteral {
                content: ch.to_string(),
                span: Span::new(start, self.pos, start_line, start_col),
                file: std::rc::Rc::clone(&self.file),
            });
        }

        self.pos += 1; // consume closing '
        self.col += 1;
        let span = Span::new(start, self.pos, start_line, start_col);
        Ok(self.make_token(TokenKind::CharLiteral(ch), span))
    }

    // ════════════════════════════════════════
    //   IDENTIFIER AND KEYWORD LEXING
    // ════════════════════════════════════════

    fn lex_identifier(&mut self) -> Result<Token, LexError> {
        let start = self.pos;
        let start_line = self.line;
        let start_col = self.col;

        while self.pos < self.source.len() {
            let b = self.source[self.pos];
            if b.is_ascii_alphanumeric() || b == b'_' {
                self.pos += 1;
            } else {
                break;
            }
        }

        let span = self.finish_span(start, start_line, start_col);
        // SAFETY: the scanned range only consumed ASCII alphanumeric/underscore
        // bytes, so the slice is pure ASCII and therefore valid UTF-8.
        let slice = unsafe { std::str::from_utf8_unchecked(&self.source[start..self.pos]) };

        let kind = match slice {
            "let" => TokenKind::Let,
            "mut" => TokenKind::Mut,
            "fn" => TokenKind::Fn,
            "return" => TokenKind::Return,
            "if" => TokenKind::If,
            "else" => TokenKind::Else,
            "while" => TokenKind::While,
            "for" => TokenKind::For,
            "in" => TokenKind::In,
            "break" => TokenKind::Break,
            "continue" => TokenKind::Continue,
            "loop" => TokenKind::Loop,
            "struct" => TokenKind::Struct,
            "impl" => TokenKind::Impl,
            "enum" => TokenKind::Enum,
            "match" => TokenKind::Match,
            "pub" => TokenKind::Pub,
            "import" => TokenKind::Import,
            "module" => TokenKind::Module,
            "spawn" => TokenKind::Spawn,
            "select" => TokenKind::Select,
            "null" => TokenKind::Null,
            "true" => TokenKind::BoolLiteral(true),
            "false" => TokenKind::BoolLiteral(false),
            "int" => TokenKind::IntType,
            "float" => TokenKind::FloatType,
            "bool" => TokenKind::BoolType,
            "str" => TokenKind::StrType,
            "char" => TokenKind::CharType,
            "void" => TokenKind::VoidType,
            _ => {
                if let Some(existing) = self.interns.get(slice) {
                    TokenKind::Identifier(std::rc::Rc::clone(existing))
                } else {
                    let rc: std::rc::Rc<str> = std::rc::Rc::from(slice);
                    self.interns.insert(std::rc::Rc::clone(&rc));
                    TokenKind::Identifier(rc)
                }
            }
        };

        Ok(self.make_token(kind, span))
    }

    // ════════════════════════════════════════
    //   OPERATOR AND PUNCTUATION LEXING
    // ════════════════════════════════════════

    fn lex_operator(&mut self) -> Result<Token, LexError> {
        let start = self.pos;
        let start_line = self.line;
        let start_col = self.col;

        let b = self.advance_byte();

        let kind = match b {
            b'(' => TokenKind::LeftParen,
            b')' => TokenKind::RightParen,
            b'{' => TokenKind::LeftBrace,
            b'}' => TokenKind::RightBrace,
            b'[' => TokenKind::LeftBracket,
            b']' => TokenKind::RightBracket,
            b',' => TokenKind::Comma,
            b';' => TokenKind::Semicolon,
            b'%' => TokenKind::Percent,
            b'#' => TokenKind::Hash,
            b'*' => TokenKind::Star,
            b':' => TokenKind::Colon,
            b'+' => TokenKind::Plus,

            b'-' => {
                if self.advance_if_byte(b'>') {
                    TokenKind::Arrow
                } else {
                    TokenKind::Minus
                }
            }

            b'/' => TokenKind::Slash,

            b'=' => {
                if self.advance_if_byte(b'=') {
                    TokenKind::EqualsEquals
                } else if self.advance_if_byte(b'>') {
                    TokenKind::FatArrow
                } else {
                    TokenKind::Equals
                }
            }

            b'!' => {
                if self.advance_if_byte(b'=') {
                    TokenKind::BangEquals
                } else {
                    TokenKind::Bang
                }
            }

            b'<' => {
                if self.advance_if_byte(b'=') {
                    TokenKind::LessEquals
                } else {
                    TokenKind::Less
                }
            }

            b'>' => {
                if self.advance_if_byte(b'=') {
                    TokenKind::GreaterEquals
                } else {
                    TokenKind::Greater
                }
            }

            b'&' => {
                if self.advance_if_byte(b'&') {
                    TokenKind::And
                } else {
                    return Err(LexError::UnexpectedChar {
                        ch: '&',
                        span: Span::new(start, self.pos, start_line, start_col),
                        file: std::rc::Rc::clone(&self.file),
                    });
                }
            }

            b'|' => {
                if self.advance_if_byte(b'|') {
                    TokenKind::Or
                } else {
                    return Err(LexError::UnexpectedChar {
                        ch: '|',
                        span: Span::new(start, self.pos, start_line, start_col),
                        file: std::rc::Rc::clone(&self.file),
                    });
                }
            }

            b'.' => {
                if self.advance_if_byte(b'.') {
                    if self.advance_if_byte(b'=') {
                        TokenKind::DotDotEquals
                    } else {
                        TokenKind::DotDot
                    }
                } else {
                    TokenKind::Dot
                }
            }

            b'?' => {
                if self.advance_if_byte(b'?') {
                    TokenKind::QuestionQuestion
                } else {
                    TokenKind::Question
                }
            }

            b'\n' => {
                self.line += 1;
                self.col = 1;
                TokenKind::Newline
            }

            other => {
                return Err(LexError::UnexpectedChar {
                    ch: if other >= 0x80 {
                        self.char_at(start)
                    } else {
                        other as char
                    },
                    span: Span::new(start, self.pos, start_line, start_col),
                    file: std::rc::Rc::clone(&self.file),
                })
            }
        };

        let span = self.finish_span(start, start_line, start_col);
        Ok(self.make_token(kind, span))
    }

    // ════════════════════════════════════════
    //   THE MAIN TOKENIZE LOOP
    // ════════════════════════════════════════

    /// Public API: tokenize a full source file
    pub fn tokenize(source: &str, file: &str) -> Result<Vec<Token>, LexError> {
        let mut lexer = Lexer::new(source, file);
        lexer.run()
    }

    /// Internal tokenize loop
    fn run(&mut self) -> Result<Vec<Token>, LexError> {
        let mut tokens: Vec<Token> = Vec::with_capacity(self.source.len() / 3 + 16);
        let mut newline_significant = false;

        loop {
            // Step 1: Skip spaces/tabs
            self.skip_whitespace_byte();

            // Step 2: Skip comments (may loop multiple times for back-to-back comments)
            self.skip_comments()?;

            // Step 3: Skip whitespace again after comments
            self.skip_whitespace_byte();

            // Step 4: End of file
            if self.pos >= self.source.len() {
                let span = Span::new(self.pos, self.pos, self.line, self.col);
                tokens.push(self.make_token(TokenKind::EOF, span));
                break;
            }

            let b = self.source[self.pos];

            // Step 5: Handle newlines (significant or skip)
            if b == b'\n' {
                if newline_significant {
                    let start = self.pos;
                    let nl_col = self.col;
                    self.pos += 1;
                    self.line += 1;
                    self.col = 1;
                    let span = Span::new(start, self.pos, self.line - 1, nl_col);
                    tokens.push(self.make_token(TokenKind::Newline, span));
                    newline_significant = false;
                } else {
                    self.pos += 1;
                    self.line += 1;
                    self.col = 1;
                }
                continue;
            }

            // Step 6: Lex the next token
            let token = if b.is_ascii_digit() {
                self.lex_number()?
            } else if b == b'"' {
                self.lex_string()?
            } else if b == b'\'' {
                self.lex_char()?
            } else if b.is_ascii_alphabetic() || b == b'_' {
                self.lex_identifier()?
            } else {
                self.lex_operator()?
            };

            newline_significant = Self::ends_expression(&token.kind);
            tokens.push(token);
        }

        Ok(tokens)
    }
}

#[cfg(test)]
mod lexer_core_tests {
    use super::*;

    #[test]
    fn test_new_lexer() {
        let lexer = Lexer::new("hello", "test.lyz");
        assert!(!lexer.is_at_end());
        assert_eq!(lexer.current(), 'h');
        assert_eq!(lexer.line, 1);
        assert_eq!(lexer.col, 1);
    }

    #[test]
    fn test_empty_source() {
        let lexer = Lexer::new("", "test.lyz");
        assert!(lexer.is_at_end());
        assert_eq!(lexer.current(), '\0');
    }

    #[test]
    fn test_advance() {
        let mut lexer = Lexer::new("abc", "test.lyz");
        assert_eq!(lexer.advance(), 'a');
        assert_eq!(lexer.current(), 'b');
        assert_eq!(lexer.advance(), 'b');
        assert_eq!(lexer.current(), 'c');
    }

    #[test]
    fn test_peek() {
        let lexer = Lexer::new("abc", "test.lyz");
        assert_eq!(lexer.current(), 'a');
        assert_eq!(lexer.peek(), 'b');
        assert_eq!(lexer.peek2(), 'c');
    }

    #[test]
    fn test_peek_at_end() {
        let lexer = Lexer::new("a", "test.lyz");
        assert_eq!(lexer.peek(), '\0');
        assert_eq!(lexer.peek2(), '\0');
    }

    #[test]
    fn test_advance_if_matches() {
        let mut lexer = Lexer::new("=>", "test.lyz");
        assert!(lexer.advance_if('='));
        assert_eq!(lexer.current(), '>');
        assert!(!lexer.advance_if('x'));
        assert_eq!(lexer.current(), '>');
    }

    #[test]
    fn test_advance_while() {
        let mut lexer = Lexer::new("   hello", "test.lyz");
        lexer.advance_while(|c| c == ' ');
        assert_eq!(lexer.current(), 'h');
    }

    #[test]
    fn test_character_classification() {
        assert!(Lexer::is_digit('5'));
        assert!(!Lexer::is_digit('a'));
        assert!(Lexer::is_alpha('a'));
        assert!(Lexer::is_alpha('_'));
        assert!(!Lexer::is_alpha('5'));
        assert!(Lexer::is_alphanumeric('a'));
        assert!(Lexer::is_alphanumeric('5'));
        assert!(Lexer::is_whitespace(' '));
        assert!(Lexer::is_whitespace('\t'));
        assert!(!Lexer::is_whitespace('\n'));
    }

    #[test]
    fn test_unicode_advance() {
        let mut lexer = Lexer::new("café", "test.lyz");
        assert_eq!(lexer.advance(), 'c');
        assert_eq!(lexer.advance(), 'a');
        assert_eq!(lexer.advance(), 'f');
        let e_with_accent = lexer.advance();
        assert_eq!(e_with_accent, 'é');
        assert!(lexer.is_at_end());
    }
}

#[cfg(test)]
mod whitespace_tests {
    use super::*;

    #[test]
    fn test_skip_spaces() {
        let mut lexer = Lexer::new("   hello", "test.lyz");
        lexer.skip_whitespace();
        assert_eq!(lexer.current(), 'h');
    }

    #[test]
    fn test_skip_tabs() {
        let mut lexer = Lexer::new("\t\thello", "test.lyz");
        lexer.skip_whitespace();
        assert_eq!(lexer.current(), 'h');
    }

    #[test]
    fn test_newline_not_skipped_by_whitespace() {
        let mut lexer = Lexer::new("\nhello", "test.lyz");
        lexer.skip_whitespace();
        assert_eq!(lexer.current(), '\n');
    }

    #[test]
    fn test_skip_single_line_comment() {
        let mut lexer = Lexer::new("-- this is a comment\nhello", "test.lyz");
        let skipped = lexer.skip_comments().unwrap();
        assert!(skipped);
        assert_eq!(lexer.current(), '\n');
    }

    #[test]
    fn test_skip_multi_line_comment() {
        let mut lexer = Lexer::new("/- this\nis\na comment -/hello", "test.lyz");
        let skipped = lexer.skip_comments().unwrap();
        assert!(skipped);
        assert_eq!(lexer.current(), 'h');
    }

    #[test]
    fn test_unterminated_comment_error() {
        let mut lexer = Lexer::new("/- this never closes", "test.lyz");
        let result = lexer.skip_comments();
        assert!(result.is_err());
        match result.unwrap_err() {
            LexError::UnterminatedComment { .. } => {}
            other => panic!("Expected UnterminatedComment, got {:?}", other),
        }
    }

    #[test]
    fn test_newline_significance() {
        assert!(Lexer::ends_expression(&TokenKind::RightParen));
        assert!(Lexer::ends_expression(&TokenKind::Identifier("x".into())));
        assert!(Lexer::ends_expression(&TokenKind::IntLiteral(42)));
        assert!(!Lexer::ends_expression(&TokenKind::Plus));
        assert!(!Lexer::ends_expression(&TokenKind::LeftBrace));
        assert!(!Lexer::ends_expression(&TokenKind::Newline));
        assert!(!Lexer::ends_expression(&TokenKind::EOF));
    }

    #[test]
    fn test_multi_line_comment_updates_line_count() {
        let mut lexer = Lexer::new("/- line1\nline2\nline3 -/x", "test.lyz");
        lexer.skip_comments().unwrap();
        assert_eq!(lexer.line, 3);
        assert_eq!(lexer.current(), 'x');
    }
}

#[cfg(test)]
mod number_tests {
    use super::*;

    fn lex_one(src: &str) -> Result<Token, LexError> {
        let mut lexer = Lexer::new(src, "test.lyz");
        lexer.lex_number()
    }

    #[test]
    fn test_simple_integer() {
        let tok = lex_one("42").unwrap();
        assert_eq!(tok.kind, TokenKind::IntLiteral(42));
    }

    #[test]
    fn test_zero() {
        let tok = lex_one("0").unwrap();
        assert_eq!(tok.kind, TokenKind::IntLiteral(0));
    }

    #[test]
    fn test_large_integer() {
        let tok = lex_one("9223372036854775807").unwrap();
        assert_eq!(tok.kind, TokenKind::IntLiteral(i64::MAX));
    }

    #[test]
    fn test_integer_with_underscores() {
        let tok = lex_one("1_000_000").unwrap();
        assert_eq!(tok.kind, TokenKind::IntLiteral(1_000_000));
    }

    #[test]
    fn test_float() {
        let tok = lex_one("3.25").unwrap();
        assert_eq!(tok.kind, TokenKind::FloatLiteral(3.25));
    }

    #[test]
    fn test_float_scientific() {
        let tok = lex_one("1.5e10").unwrap();
        assert_eq!(tok.kind, TokenKind::FloatLiteral(1.5e10));
    }

    #[test]
    fn test_hex() {
        let tok = lex_one("0xFF").unwrap();
        assert_eq!(tok.kind, TokenKind::IntLiteral(255));
    }

    #[test]
    fn test_binary() {
        let tok = lex_one("0b1010").unwrap();
        assert_eq!(tok.kind, TokenKind::IntLiteral(10));
    }

    #[test]
    fn test_octal() {
        let tok = lex_one("0o17").unwrap();
        assert_eq!(tok.kind, TokenKind::IntLiteral(15));
    }

    #[test]
    fn test_integer_overflow_error() {
        let result = lex_one("99999999999999999999999999");
        assert!(result.is_err());
        match result.unwrap_err() {
            LexError::NumberOverflow { .. } => {}
            other => panic!("Expected NumberOverflow, got {:?}", other),
        }
    }

    #[test]
    fn test_span_is_correct() {
        let tok = lex_one("123").unwrap();
        assert_eq!(tok.span.line, 1);
        assert_eq!(tok.span.col, 1);
        assert_eq!(tok.span.len(), 3);
    }
}

#[cfg(test)]
mod string_tests {
    use super::*;

    fn lex_str(src: &str) -> Result<Token, LexError> {
        let mut lexer = Lexer::new(src, "test.lyz");
        lexer.lex_string()
    }

    fn unwrap_str(tok: Token) -> String {
        match tok.kind {
            TokenKind::StringLiteral(s) => s,
            other => panic!("Expected StringLiteral, got {:?}", other),
        }
    }

    #[test]
    fn test_simple_string() {
        let s = unwrap_str(lex_str("\"hello\"").unwrap());
        assert_eq!(s, "hello");
    }

    #[test]
    fn test_empty_string() {
        let s = unwrap_str(lex_str("\"\"").unwrap());
        assert_eq!(s, "");
    }

    #[test]
    fn test_escape_newline() {
        let s = unwrap_str(lex_str("\"line1\\nline2\"").unwrap());
        assert_eq!(s, "line1\nline2");
    }

    #[test]
    fn test_escape_tab() {
        let s = unwrap_str(lex_str("\"a\\tb\"").unwrap());
        assert_eq!(s, "a\tb");
    }

    #[test]
    fn test_escape_backslash() {
        let s = unwrap_str(lex_str("\"path\\\\to\\\\file\"").unwrap());
        assert_eq!(s, "path\\to\\file");
    }

    #[test]
    fn test_escape_quote() {
        let s = unwrap_str(lex_str("\"say \\\"hi\\\"\"").unwrap());
        assert_eq!(s, "say \"hi\"");
    }

    #[test]
    fn test_unicode_escape() {
        let s = unwrap_str(lex_str("\"\\u{0041}\"").unwrap());
        assert_eq!(s, "A");
    }

    #[test]
    fn test_emoji_unicode_escape() {
        let s = unwrap_str(lex_str("\"\\u{1F600}\"").unwrap());
        assert_eq!(s, "\u{1F600}");
    }

    #[test]
    fn test_unterminated_string() {
        let result = lex_str("\"never closed");
        assert!(result.is_err());
        match result.unwrap_err() {
            LexError::UnterminatedString { .. } => {}
            e => panic!("Expected UnterminatedString, got {:?}", e),
        }
    }

    #[test]
    fn test_invalid_escape() {
        let result = lex_str("\"\\q\"");
        assert!(result.is_err());
        match result.unwrap_err() {
            LexError::InvalidEscape { ch: 'q', .. } => {}
            e => panic!("Expected InvalidEscape('q'), got {:?}", e),
        }
    }

    #[test]
    fn test_string_with_utf8_content() {
        let tok = lex_str("\"\u{0645}\u{0631}\u{062D}\u{0628}\u{0627}\"").unwrap();
        assert_eq!(tok.span.start, 0);
        assert_eq!(tok.span.end, 12); // 2 quotes + 5 chars x 2 bytes each
        match tok.kind {
            TokenKind::StringLiteral(s) => {
                assert_eq!(s, "\u{0645}\u{0631}\u{062D}\u{0628}\u{0627}")
            }
            other => panic!("Expected StringLiteral, got {:?}", other),
        }
    }

    #[test]
    fn test_span_covers_full_string_including_quotes() {
        let tok = lex_str("\"hi\"").unwrap();
        assert_eq!(tok.span.len(), 4);
    }
}

#[cfg(test)]
mod char_ident_tests {
    use super::*;

    #[test]
    fn test_char_literal_simple() {
        let mut l = Lexer::new("'a'", "test.lyz");
        let tok = l.lex_char().unwrap();
        assert_eq!(tok.kind, TokenKind::CharLiteral('a'));
    }

    #[test]
    fn test_char_literal_escape() {
        let mut l = Lexer::new("'\\n'", "test.lyz");
        let tok = l.lex_char().unwrap();
        assert_eq!(tok.kind, TokenKind::CharLiteral('\n'));
    }

    #[test]
    fn test_char_literal_too_many_chars() {
        let mut l = Lexer::new("'ab'", "test.lyz");
        assert!(l.lex_char().is_err());
    }

    #[test]
    fn test_keyword_let() {
        let mut l = Lexer::new("let", "test.lyz");
        let tok = l.lex_identifier().unwrap();
        assert_eq!(tok.kind, TokenKind::Let);
    }

    #[test]
    fn test_keyword_fn() {
        let mut l = Lexer::new("fn", "test.lyz");
        let tok = l.lex_identifier().unwrap();
        assert_eq!(tok.kind, TokenKind::Fn);
    }

    #[test]
    fn test_identifier_not_keyword() {
        let mut l = Lexer::new("myVariable", "test.lyz");
        let tok = l.lex_identifier().unwrap();
        assert_eq!(tok.kind, TokenKind::Identifier("myVariable".into()));
    }

    #[test]
    fn test_identifier_underscore_prefix() {
        let mut l = Lexer::new("_privateVar", "test.lyz");
        let tok = l.lex_identifier().unwrap();
        assert_eq!(tok.kind, TokenKind::Identifier("_privateVar".into()));
    }

    #[test]
    fn test_bool_true_false() {
        let mut l = Lexer::new("true", "test.lyz");
        assert_eq!(
            l.lex_identifier().unwrap().kind,
            TokenKind::BoolLiteral(true)
        );
        let mut l = Lexer::new("false", "test.lyz");
        assert_eq!(
            l.lex_identifier().unwrap().kind,
            TokenKind::BoolLiteral(false)
        );
    }

    #[test]
    fn test_all_keywords_recognized() {
        let keywords = [
            ("let", TokenKind::Let),
            ("mut", TokenKind::Mut),
            ("fn", TokenKind::Fn),
            ("return", TokenKind::Return),
            ("if", TokenKind::If),
            ("else", TokenKind::Else),
            ("while", TokenKind::While),
            ("for", TokenKind::For),
            ("in", TokenKind::In),
            ("break", TokenKind::Break),
            ("struct", TokenKind::Struct),
            ("enum", TokenKind::Enum),
            ("match", TokenKind::Match),
            ("spawn", TokenKind::Spawn),
        ];
        for (kw, expected) in keywords {
            let mut l = Lexer::new(kw, "test.lyz");
            assert_eq!(
                l.lex_identifier().unwrap().kind,
                expected,
                "keyword: {}",
                kw
            );
        }
    }
}

#[cfg(test)]
mod operator_tests {
    use super::*;

    fn lex_op(src: &str) -> TokenKind {
        let mut l = Lexer::new(src, "test.lyz");
        l.lex_operator().unwrap().kind
    }

    #[test]
    fn test_arrow() {
        assert_eq!(lex_op("->"), TokenKind::Arrow);
    }
    #[test]
    fn test_fat_arrow() {
        assert_eq!(lex_op("=>"), TokenKind::FatArrow);
    }
    #[test]
    fn test_equals_equals() {
        assert_eq!(lex_op("=="), TokenKind::EqualsEquals);
    }
    #[test]
    fn test_bang_equals() {
        assert_eq!(lex_op("!="), TokenKind::BangEquals);
    }
    #[test]
    fn test_less_equals() {
        assert_eq!(lex_op("<="), TokenKind::LessEquals);
    }
    #[test]
    fn test_greater_equals() {
        assert_eq!(lex_op(">="), TokenKind::GreaterEquals);
    }
    #[test]
    fn test_and() {
        assert_eq!(lex_op("&&"), TokenKind::And);
    }
    #[test]
    fn test_or() {
        assert_eq!(lex_op("||"), TokenKind::Or);
    }
    #[test]
    fn test_dot_dot() {
        assert_eq!(lex_op(".."), TokenKind::DotDot);
    }
    #[test]
    fn test_dot_dot_equals() {
        assert_eq!(lex_op("..="), TokenKind::DotDotEquals);
    }
    #[test]
    fn test_question_question() {
        assert_eq!(lex_op("??"), TokenKind::QuestionQuestion);
    }
    #[test]
    fn test_single_equals() {
        assert_eq!(lex_op("="), TokenKind::Equals);
    }
    #[test]
    fn test_less() {
        assert_eq!(lex_op("<"), TokenKind::Less);
    }
    #[test]
    fn test_single_dot() {
        assert_eq!(lex_op("."), TokenKind::Dot);
    }
    #[test]
    fn test_bang() {
        assert_eq!(lex_op("!"), TokenKind::Bang);
    }

    #[test]
    fn test_unknown_char_error() {
        let mut l = Lexer::new("@", "test.lyz");
        assert!(l.lex_operator().is_err());
    }

    #[test]
    fn test_ambiguous_minus_not_arrow() {
        let mut l = Lexer::new("- 5", "test.lyz");
        let tok = l.lex_operator().unwrap();
        assert_eq!(tok.kind, TokenKind::Minus);
        assert_eq!(l.current(), ' ');
    }
}

#[cfg(test)]
mod tokenize_tests {
    use super::*;

    fn kinds(src: &str) -> Vec<TokenKind> {
        Lexer::tokenize(src, "test.lyz")
            .unwrap()
            .into_iter()
            .map(|t| t.kind)
            .collect()
    }

    #[test]
    fn test_empty_source() {
        let toks = kinds("");
        assert_eq!(toks, vec![TokenKind::EOF]);
    }

    #[test]
    fn test_skips_utf8_bom() {
        let toks = kinds("\u{FEFF}let x = 5");
        assert_eq!(
            toks,
            vec![
                TokenKind::Let,
                TokenKind::Identifier("x".into()),
                TokenKind::Equals,
                TokenKind::IntLiteral(5),
                TokenKind::EOF,
            ]
        );
    }

    #[test]
    fn test_simple_let() {
        let toks = kinds("let x = 5");
        assert_eq!(
            toks,
            vec![
                TokenKind::Let,
                TokenKind::Identifier("x".into()),
                TokenKind::Equals,
                TokenKind::IntLiteral(5),
                TokenKind::EOF,
            ]
        );
    }

    #[test]
    fn test_function_declaration() {
        let src = "fn add(a: int, b: int) -> int";
        let toks = kinds(src);
        assert_eq!(toks[0], TokenKind::Fn);
        assert_eq!(toks[1], TokenKind::Identifier("add".into()));
        assert_eq!(toks[2], TokenKind::LeftParen);
        assert_eq!(toks[10], TokenKind::RightParen);
        assert_eq!(toks[11], TokenKind::Arrow);
        assert_eq!(toks[12], TokenKind::IntType);
    }

    #[test]
    fn test_newline_significant_after_rparen() {
        let toks = kinds("foo()\nbar()");
        assert!(toks.contains(&TokenKind::Newline));
    }

    #[test]
    fn test_newline_not_significant_after_plus() {
        let toks = kinds("1 +\n2");
        assert!(!toks.contains(&TokenKind::Newline));
    }

    #[test]
    fn test_comment_skipped() {
        let toks = kinds("let x = 5 -- this is a comment\nlet y = 6");
        assert!(!toks
            .iter()
            .any(|t| matches!(t, TokenKind::Identifier(s) if s.as_ref() == "this")));
        assert!(toks.contains(&TokenKind::Let));
    }

    #[test]
    fn test_multi_line_program() {
        let src = r#"
fn greet(name: str) -> str {
    return "Hello, " + name
}
"#;
        let result = Lexer::tokenize(src, "test.lyz");
        assert!(result.is_ok());
        let toks = result.unwrap();
        assert!(toks.iter().any(|t| t.kind == TokenKind::Fn));
        assert!(toks.iter().any(|t| t.kind == TokenKind::Return));
        assert!(toks.last().unwrap().kind == TokenKind::EOF);
    }

    #[test]
    fn test_all_token_kinds_produced() {
        let src = r#"
let mut x: int = 42
let f: float = 3.14
let s: str = "hello"
let b: bool = true
let c: char = 'A'
fn add(a: int, b: int) -> int => a + b
if x > 0 { return x } else { return 0 }
for i in 0..10 { print(i) }
while x != 0 { x = x - 1 }
match x { 0 -> "zero" _ -> "other" }
spawn { doWork() }
"#;
        let result = Lexer::tokenize(src, "test.lyz");
        assert!(result.is_ok(), "Lex error: {:?}", result.err());
    }

    #[test]
    fn test_error_propagated() {
        let result = Lexer::tokenize("let x = @bad", "test.lyz");
        assert!(result.is_err());
    }

    #[test]
    fn test_always_ends_with_eof() {
        let toks = kinds("let x = 5");
        assert_eq!(toks.last().unwrap(), &TokenKind::EOF);

        let toks = kinds("");
        assert_eq!(toks.last().unwrap(), &TokenKind::EOF);
    }
}
