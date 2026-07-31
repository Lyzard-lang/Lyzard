/// Every possible kind of token in LYZARD.
/// This is the full vocabulary of the language.
#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    // ── LITERALS ─────────────────────────────────────────────
    IntLiteral(i64),           // 42, 1_000_000
    FloatLiteral(f64),         // 3.14, 2.718
    StringLiteral(String),     // "hello world"
    BoolLiteral(bool),         // true, false
    CharLiteral(char),         // 'a', '\n'

    // ── IDENTIFIERS ──────────────────────────────────────────
    Identifier(String),        // myVar, user, _private

    // ── KEYWORDS ─────────────────────────────────────────────
    Let,       // let
    Mut,       // mut
    Fn,        // fn
    Return,    // return
    If,        // if
    Else,      // else
    While,     // while
    For,       // for
    In,        // in
    Break,     // break
    Continue,  // continue
    Loop,      // loop
    Struct,    // struct
    Impl,      // impl
    Enum,      // enum
    Match,     // match
    Pub,       // pub
    Import,    // import
    Module,    // module
    Spawn,     // spawn
    Select,    // select
    Null,      // null

    // ── BUILT-IN TYPES ───────────────────────────────────────
    IntType,   // int
    FloatType, // float
    BoolType,  // bool
    StrType,   // str
    CharType,  // char
    VoidType,  // void

    // ── ARITHMETIC OPERATORS ─────────────────────────────────
    Plus,      // +
    Minus,     // -
    Star,      // *
    Slash,     // /
    Percent,   // %

    // ── ASSIGNMENT ───────────────────────────────────────────
    Equals,    // =

    // ── COMPARISON ───────────────────────────────────────────
    EqualsEquals,    // ==
    BangEquals,      // !=
    Less,            // <
    LessEquals,      // <=
    Greater,         // >
    GreaterEquals,   // >=

    // ── LOGICAL ──────────────────────────────────────────────
    And,   // &&
    Or,    // ||
    Bang,  // !

    // ── ARROWS ───────────────────────────────────────────────
    Arrow,    // ->
    FatArrow, // =>

    // ── RANGE ────────────────────────────────────────────────
    DotDot,       // ..
    DotDotEquals, // ..=

    // ── OPTIONAL / ERROR PROPAGATION ─────────────────────────
    Question,         // ?
    QuestionQuestion, // ??  (null coalescing)

    // ── PUNCTUATION ──────────────────────────────────────────
    LeftParen,    // (
    RightParen,   // )
    LeftBrace,    // {
    RightBrace,   // }
    LeftBracket,  // [
    RightBracket, // ]
    Comma,        // ,
    Colon,        // :
    Semicolon,    // ;
    Dot,          // .
    Hash,         // #

    // ── SPECIAL ──────────────────────────────────────────────
    Newline,  // \n (significant in LYZARD)
    EOF,      // end of file
}

impl TokenKind {
    /// Human-readable name for error messages
    pub fn name(&self) -> &'static str {
        match self {
            Self::IntLiteral(_)    => "integer literal",
            Self::FloatLiteral(_)  => "float literal",
            Self::StringLiteral(_) => "string literal",
            Self::BoolLiteral(_)   => "bool literal",
            Self::CharLiteral(_)   => "char literal",
            Self::Identifier(_)    => "identifier",
            Self::Let              => "'let'",
            Self::Mut              => "'mut'",
            Self::Fn               => "'fn'",
            Self::Return           => "'return'",
            Self::If               => "'if'",
            Self::Else             => "'else'",
            Self::While            => "'while'",
            Self::For              => "'for'",
            Self::In               => "'in'",
            Self::Break            => "'break'",
            Self::Continue         => "'continue'",
            Self::Loop             => "'loop'",
            Self::Struct           => "'struct'",
            Self::Impl             => "'impl'",
            Self::Enum             => "'enum'",
            Self::Match            => "'match'",
            Self::Pub              => "'pub'",
            Self::Import           => "'import'",
            Self::Module           => "'module'",
            Self::Spawn            => "'spawn'",
            Self::Select           => "'select'",
            Self::Null             => "'null'",
            Self::IntType          => "'int'",
            Self::FloatType        => "'float'",
            Self::BoolType         => "'bool'",
            Self::StrType          => "'str'",
            Self::CharType         => "'char'",
            Self::VoidType         => "'void'",
            Self::Plus             => "'+'",
            Self::Minus            => "'-'",
            Self::Star             => "'*'",
            Self::Slash            => "'/'",
            Self::Percent          => "'%'",
            Self::Equals           => "'='",
            Self::EqualsEquals     => "'=='",
            Self::BangEquals       => "'!='",
            Self::Less             => "'<'",
            Self::LessEquals       => "'<='",
            Self::Greater          => "'>'",
            Self::GreaterEquals    => "'>='",
            Self::And              => "'&&'",
            Self::Or               => "'||'",
            Self::Bang             => "'!'",
            Self::Arrow            => "'->'",
            Self::FatArrow         => "'=>'",
            Self::DotDot           => "'..'",
            Self::DotDotEquals     => "'..='",
            Self::Question         => "'?'",
            Self::QuestionQuestion => "'??'",
            Self::LeftParen        => "'('",
            Self::RightParen       => "')'",
            Self::LeftBrace        => "'{'",
            Self::RightBrace       => "'}'",
            Self::LeftBracket      => "'['",
            Self::RightBracket     => "']'",
            Self::Comma            => "','",
            Self::Colon            => "':'",
            Self::Semicolon        => "';'",
            Self::Dot              => "'.'",
            Self::Hash             => "'#'",
            Self::Newline          => "newline",
            Self::EOF              => "end of file",
        }
    }

    /// Is this token a keyword?
    pub fn is_keyword(&self) -> bool {
        matches!(self,
            Self::Let | Self::Mut | Self::Fn | Self::Return | Self::If | Self::Else |
            Self::While | Self::For | Self::In | Self::Break | Self::Continue | Self::Loop |
            Self::Struct | Self::Impl | Self::Enum | Self::Match | Self::Pub |
            Self::Import | Self::Module | Self::Spawn | Self::Select | Self::Null |
            Self::IntType | Self::FloatType | Self::BoolType | Self::StrType |
            Self::CharType | Self::VoidType
        )
    }

    /// Is this a literal value?
    pub fn is_literal(&self) -> bool {
        matches!(self,
            Self::IntLiteral(_) | Self::FloatLiteral(_) | Self::StringLiteral(_) |
            Self::BoolLiteral(_) | Self::CharLiteral(_)
        )
    }

    /// Is this an operator?
    pub fn is_operator(&self) -> bool {
        matches!(self,
            Self::Plus | Self::Minus | Self::Star | Self::Slash | Self::Percent |
            Self::Equals | Self::EqualsEquals | Self::BangEquals | Self::Less |
            Self::LessEquals | Self::Greater | Self::GreaterEquals | Self::And |
            Self::Or | Self::Bang
        )
    }
}

impl std::fmt::Display for TokenKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_kind_names() {
        assert_eq!(TokenKind::Let.name(), "'let'");
        assert_eq!(TokenKind::Fn.name(), "'fn'");
        assert_eq!(TokenKind::EOF.name(), "end of file");
        assert_eq!(TokenKind::IntLiteral(0).name(), "integer literal");
    }

    #[test]
    fn test_is_keyword() {
        assert!(TokenKind::Let.is_keyword());
        assert!(TokenKind::Fn.is_keyword());
        assert!(TokenKind::While.is_keyword());
        assert!(!TokenKind::Plus.is_keyword());
        assert!(!TokenKind::Identifier("x".to_string()).is_keyword());
    }

    #[test]
    fn test_is_literal() {
        assert!(TokenKind::IntLiteral(42).is_literal());
        assert!(TokenKind::FloatLiteral(3.25).is_literal());
        assert!(TokenKind::StringLiteral("hi".to_string()).is_literal());
        assert!(TokenKind::BoolLiteral(true).is_literal());
        assert!(!TokenKind::Let.is_literal());
        assert!(!TokenKind::Plus.is_literal());
    }

    #[test]
    fn test_clone_and_eq() {
        let a = TokenKind::IntLiteral(99);
        let b = a.clone();
        assert_eq!(a, b);

        let c = TokenKind::StringLiteral("hello".to_string());
        let d = TokenKind::StringLiteral("world".to_string());
        assert_ne!(c, d);
    }

    #[test]
    fn test_display() {
        assert_eq!(format!("{}", TokenKind::Plus), "'+'");
        assert_eq!(format!("{}", TokenKind::Fn), "'fn'");
    }
}

/// A span in the source file — where a token lives
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    pub line: usize,
    pub col: usize,
}

impl Span {
    pub fn new(start: usize, end: usize, line: usize, col: usize) -> Self {
        Span { start, end, line, col }
    }

    pub fn dummy() -> Self {
        Span { start: 0, end: 0, line: 0, col: 0 }
    }

    pub fn len(&self) -> usize {
        self.end.saturating_sub(self.start)
    }

    pub fn is_empty(&self) -> bool {
        self.start >= self.end
    }

    pub fn merge(self, other: Span) -> Span {
        Span {
            start: self.start.min(other.start),
            end:   self.end.max(other.end),
            line:  self.line.min(other.line),
            col:   self.col,
        }
    }
}

impl std::fmt::Display for Span {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.line, self.col)
    }
}

/// A token: its kind + its position in the source file
#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
    pub file: String,
}

impl Token {
    pub fn new(kind: TokenKind, span: Span, file: impl Into<String>) -> Self {
        Token { kind, span, file: file.into() }
    }

    pub fn dummy(kind: TokenKind) -> Self {
        Token { kind, span: Span::dummy(), file: "<test>".to_string() }
    }

    pub fn is_eof(&self) -> bool {
        self.kind == TokenKind::EOF
    }

    pub fn is_newline(&self) -> bool {
        self.kind == TokenKind::Newline
    }

    pub fn identifier_name(&self) -> &str {
        match &self.kind {
            TokenKind::Identifier(name) => name,
            _ => panic!("Token is not an identifier: {:?}", self.kind),
        }
    }
}

impl std::fmt::Display for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} at {}", self.kind, self.span)
    }
}

#[cfg(test)]
mod span_tests {
    use super::*;

    #[test]
    fn test_span_new() {
        let span = Span::new(0, 3, 1, 1);
        assert_eq!(span.len(), 3);
        assert_eq!(span.line, 1);
        assert_eq!(span.col, 1);
    }

    #[test]
    fn test_span_merge() {
        let a = Span::new(0, 3, 1, 1);
        let b = Span::new(5, 8, 1, 6);
        let merged = a.merge(b);
        assert_eq!(merged.start, 0);
        assert_eq!(merged.end, 8);
    }

    #[test]
    fn test_span_display() {
        let span = Span::new(0, 3, 5, 12);
        assert_eq!(format!("{}", span), "5:12");
    }

    #[test]
    fn test_token_new() {
        let token = Token::dummy(TokenKind::Let);
        assert!(!token.is_eof());
        assert!(!token.is_newline());
        assert_eq!(token.kind, TokenKind::Let);
    }

    #[test]
    fn test_token_is_eof() {
        let eof = Token::dummy(TokenKind::EOF);
        assert!(eof.is_eof());
    }

    #[test]
    fn test_token_identifier_name() {
        let tok = Token::dummy(TokenKind::Identifier("myVar".to_string()));
        assert_eq!(tok.identifier_name(), "myVar");
    }

    #[test]
    fn test_token_display() {
        let tok = Token::new(
            TokenKind::Let,
            Span::new(0, 3, 1, 1),
            "main.lyz"
        );
        let display = format!("{}", tok);
        assert!(display.contains("'let'"));
        assert!(display.contains("1:1"));
    }
}
