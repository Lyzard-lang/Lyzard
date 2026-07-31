use crate::lexer::Span;

// ═══════════════════════════════════════════════════
//   ROOT
// ═══════════════════════════════════════════════════

#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub declarations: Vec<Declaration>,
    pub span: Span,
}

// ═══════════════════════════════════════════════════
//   DECLARATIONS
// ═══════════════════════════════════════════════════

#[derive(Debug, Clone, PartialEq)]
pub enum Declaration {
    Function(FnDecl),
    Let(LetDecl),
    Const(ConstDecl),
    Struct(StructDecl),
    Enum(EnumDecl),
    Impl(ImplDecl),
    Interface(InterfaceDecl),
    Import(ImportDecl),
    Module(ModuleDecl),
    Statement(Statement),
}

impl Declaration {
    pub fn span(&self) -> Span {
        match self {
            Self::Function(d) => d.span,
            Self::Let(d) => d.span,
            Self::Const(d) => d.span,
            Self::Struct(d) => d.span,
            Self::Enum(d) => d.span,
            Self::Impl(d) => d.span,
            Self::Interface(d) => d.span,
            Self::Import(d) => d.span,
            Self::Module(d) => d.span,
            Self::Statement(s) => s.span(),
        }
    }
}

/// fn add<T>(a: T, b: T) -> T { ... }
#[derive(Debug, Clone, PartialEq)]
pub struct FnDecl {
    pub name: String,
    pub generics: Vec<GenericParam>,
    pub params: Vec<Param>,
    pub return_type: Option<TypeExpr>,
    pub body: FnBody,
    pub is_pub: bool,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FnBody {
    Block(Block),     // fn add(a,b) -> int { return a + b }
    Arrow(Box<Expr>), // fn add(a,b) -> int => a + b
}

#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub name: String,
    pub param_type: Option<TypeExpr>,
    pub is_self: bool,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GenericParam {
    pub name: String,
    pub bounds: Vec<String>, // T: Comparable + Printable
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LetDecl {
    pub name: String,
    pub mutable: bool,
    pub type_annotation: Option<TypeExpr>,
    pub value: Expr,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConstDecl {
    pub name: String,
    pub type_annotation: Option<TypeExpr>,
    pub value: Expr,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StructDecl {
    pub name: String,
    pub generics: Vec<GenericParam>,
    pub fields: Vec<StructField>,
    pub is_pub: bool,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StructField {
    pub name: String,
    pub field_type: TypeExpr,
    pub is_pub: bool,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EnumDecl {
    pub name: String,
    pub generics: Vec<GenericParam>,
    pub variants: Vec<EnumVariant>,
    pub is_pub: bool,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EnumVariant {
    pub name: String,
    pub kind: EnumVariantKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum EnumVariantKind {
    Unit,                     // Ok, Err, Pending
    Tuple(Vec<TypeExpr>),     // Circle(float)
    Struct(Vec<StructField>), // Point { x: float, y: float }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImplDecl {
    pub target: String,
    pub generics: Vec<GenericParam>,
    pub for_interface: Option<String>,
    pub methods: Vec<FnDecl>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct InterfaceDecl {
    pub name: String,
    pub generics: Vec<GenericParam>,
    pub methods: Vec<InterfaceMethod>,
    pub is_pub: bool,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct InterfaceMethod {
    pub name: String,
    pub params: Vec<Param>,
    pub return_type: Option<TypeExpr>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImportDecl {
    pub path: Vec<String>,
    pub items: Option<Vec<String>>,
    pub alias: Option<String>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ModuleDecl {
    pub name: String,
    pub body: Vec<Declaration>,
    pub span: Span,
}

// ═══════════════════════════════════════════════════
//   STATEMENTS
// ═══════════════════════════════════════════════════

#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    Return(ReturnStmt),
    If(IfStmt),
    While(WhileStmt),
    For(ForStmt),
    Loop(LoopStmt),
    Match(MatchStmt),
    Spawn(SpawnStmt),
    Break(BreakStmt),
    Continue(ContinueStmt),
    Block(Block),
    Expression(ExprStmt),
    Let(LetDecl),
    Const(ConstDecl),
}

impl Statement {
    pub fn span(&self) -> Span {
        match self {
            Self::Return(s) => s.span,
            Self::If(s) => s.span,
            Self::While(s) => s.span,
            Self::For(s) => s.span,
            Self::Loop(s) => s.span,
            Self::Match(s) => s.span,
            Self::Spawn(s) => s.span,
            Self::Break(s) => s.span,
            Self::Continue(s) => s.span,
            Self::Block(b) => b.span,
            Self::Expression(s) => s.span,
            Self::Let(d) => d.span,
            Self::Const(d) => d.span,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Block {
    pub statements: Vec<Statement>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReturnStmt {
    pub value: Option<Expr>,
    pub span: Span,
}
#[derive(Debug, Clone, PartialEq)]
pub struct BreakStmt {
    pub label: Option<String>,
    pub span: Span,
}
#[derive(Debug, Clone, PartialEq)]
pub struct ContinueStmt {
    pub label: Option<String>,
    pub span: Span,
}
#[derive(Debug, Clone, PartialEq)]
pub struct ExprStmt {
    pub expr: Expr,
    pub span: Span,
}
#[derive(Debug, Clone, PartialEq)]
pub struct SpawnStmt {
    pub body: Block,
    pub span: Span,
}
#[derive(Debug, Clone, PartialEq)]
pub struct LoopStmt {
    pub body: Block,
    pub label: Option<String>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct IfStmt {
    pub condition: Expr,
    pub then_branch: Block,
    pub else_if_branches: Vec<ElseIfBranch>,
    pub else_branch: Option<Block>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ElseIfBranch {
    pub condition: Expr,
    pub body: Block,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WhileStmt {
    pub condition: Expr,
    pub body: Block,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ForStmt {
    pub variable: String,
    pub iterable: Expr,
    pub body: Block,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MatchStmt {
    pub subject: Expr,
    pub arms: Vec<MatchArm>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub guard: Option<Expr>,
    pub body: MatchBody,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MatchBody {
    Expr(Expr),
    Block(Block),
}

// ═══════════════════════════════════════════════════
//   EXPRESSIONS
// ═══════════════════════════════════════════════════

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Int(IntLit),
    Float(FloatLit),
    Str(StrLit),
    Bool(BoolLit),
    Char(CharLit),
    Null(NullLit),
    Array(ArrayLit),
    StructInit(StructInitExpr),
    Identifier(IdentExpr),
    Binary(BinaryExpr),
    Unary(UnaryExpr),
    Assign(AssignExpr),
    Call(CallExpr),
    MethodCall(MethodCallExpr),
    Field(FieldExpr),
    Index(IndexExpr),
    If(Box<IfExpr>),
    Match(Box<MatchExpr>),
    Block(Box<Block>),
    Propagate(PropagateExpr),
    NullCoalesce(NullCoalesceExpr),
    Cast(CastExpr),
    Range(RangeExpr),
    Closure(ClosureExpr),
}

impl Expr {
    pub fn span(&self) -> Span {
        match self {
            Self::Int(e) => e.span,
            Self::Float(e) => e.span,
            Self::Str(e) => e.span,
            Self::Bool(e) => e.span,
            Self::Char(e) => e.span,
            Self::Null(e) => e.span,
            Self::Array(e) => e.span,
            Self::StructInit(e) => e.span,
            Self::Identifier(e) => e.span,
            Self::Binary(e) => e.span,
            Self::Unary(e) => e.span,
            Self::Assign(e) => e.span,
            Self::Call(e) => e.span,
            Self::MethodCall(e) => e.span,
            Self::Field(e) => e.span,
            Self::Index(e) => e.span,
            Self::If(e) => e.span,
            Self::Match(e) => e.span,
            Self::Block(b) => b.span,
            Self::Propagate(e) => e.span,
            Self::NullCoalesce(e) => e.span,
            Self::Cast(e) => e.span,
            Self::Range(e) => e.span,
            Self::Closure(e) => e.span,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct IntLit {
    pub value: i64,
    pub span: Span,
}
#[derive(Debug, Clone, PartialEq)]
pub struct FloatLit {
    pub value: f64,
    pub span: Span,
}
#[derive(Debug, Clone, PartialEq)]
pub struct StrLit {
    pub value: String,
    pub span: Span,
}
#[derive(Debug, Clone, PartialEq)]
pub struct BoolLit {
    pub value: bool,
    pub span: Span,
}
#[derive(Debug, Clone, PartialEq)]
pub struct CharLit {
    pub value: char,
    pub span: Span,
}
#[derive(Debug, Clone, PartialEq)]
pub struct NullLit {
    pub span: Span,
}
#[derive(Debug, Clone, PartialEq)]
pub struct IdentExpr {
    pub name: String,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ArrayLit {
    pub elements: Vec<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StructInitExpr {
    pub name: String,
    pub fields: Vec<(String, Expr)>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BinaryExpr {
    pub op: BinaryOp,
    pub left: Box<Expr>,
    pub right: Box<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    NotEq,
    Lt,
    Lte,
    Gt,
    Gte,
    And,
    Or,
}

impl BinaryOp {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Add => "+",
            Self::Sub => "-",
            Self::Mul => "*",
            Self::Div => "/",
            Self::Mod => "%",
            Self::Eq => "==",
            Self::NotEq => "!=",
            Self::Lt => "<",
            Self::Lte => "<=",
            Self::Gt => ">",
            Self::Gte => ">=",
            Self::And => "&&",
            Self::Or => "||",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct UnaryExpr {
    pub op: UnaryOp,
    pub operand: Box<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum UnaryOp {
    Neg,
    Not,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AssignExpr {
    pub target: Box<Expr>,
    pub value: Box<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CallExpr {
    pub callee: Box<Expr>,
    pub args: Vec<Argument>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MethodCallExpr {
    pub object: Box<Expr>,
    pub method: String,
    pub args: Vec<Argument>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Argument {
    pub label: Option<String>,
    pub value: Expr,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FieldExpr {
    pub object: Box<Expr>,
    pub field: String,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct IndexExpr {
    pub object: Box<Expr>,
    pub index: Box<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct IfExpr {
    pub condition: Expr,
    pub then_branch: Block,
    pub else_branch: Option<Block>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MatchExpr {
    pub subject: Box<Expr>,
    pub arms: Vec<MatchArm>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PropagateExpr {
    pub expr: Box<Expr>,
    pub span: Span,
}
#[derive(Debug, Clone, PartialEq)]
pub struct NullCoalesceExpr {
    pub left: Box<Expr>,
    pub right: Box<Expr>,
    pub span: Span,
}
#[derive(Debug, Clone, PartialEq)]
pub struct CastExpr {
    pub expr: Box<Expr>,
    pub target_type: TypeExpr,
    pub span: Span,
}
#[derive(Debug, Clone, PartialEq)]
pub struct RangeExpr {
    pub start: Box<Expr>,
    pub end: Box<Expr>,
    pub inclusive: bool,
    pub span: Span,
}
#[derive(Debug, Clone, PartialEq)]
pub struct ClosureExpr {
    pub params: Vec<Param>,
    pub return_type: Option<TypeExpr>,
    pub body: FnBody,
    pub span: Span,
}

// ═══════════════════════════════════════════════════
//   TYPE EXPRESSIONS
// ═══════════════════════════════════════════════════

#[derive(Debug, Clone, PartialEq)]
pub enum TypeExpr {
    Named(NamedType),
    Optional(Box<TypeExpr>, Span),
    Array(Box<TypeExpr>, Span),
    Map(Box<TypeExpr>, Box<TypeExpr>, Span),
    Tuple(Vec<TypeExpr>, Span),
    Generic(GenericType),
    Never(Span),
}

impl TypeExpr {
    pub fn span(&self) -> Span {
        match self {
            Self::Named(t) => t.span,
            Self::Optional(_, s) => *s,
            Self::Array(_, s) => *s,
            Self::Map(_, _, s) => *s,
            Self::Tuple(_, s) => *s,
            Self::Generic(t) => t.span,
            Self::Never(s) => *s,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct NamedType {
    pub name: String,
    pub span: Span,
}
#[derive(Debug, Clone, PartialEq)]
pub struct GenericType {
    pub name: String,
    pub args: Vec<TypeExpr>,
    pub span: Span,
}

// ═══════════════════════════════════════════════════
//   PATTERNS
// ═══════════════════════════════════════════════════

#[derive(Debug, Clone, PartialEq)]
pub enum Pattern {
    Wildcard(Span),
    Binding(BindingPattern),
    Literal(LiteralPattern),
    EnumVariant(EnumVariantPattern),
    Or(OrPattern),
}

impl Pattern {
    pub fn span(&self) -> Span {
        match self {
            Self::Wildcard(s) => *s,
            Self::Binding(p) => p.span,
            Self::Literal(p) => p.span,
            Self::EnumVariant(p) => p.span,
            Self::Or(p) => p.span,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BindingPattern {
    pub name: String,
    pub mutable: bool,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LiteralPattern {
    pub value: LiteralValue,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LiteralValue {
    Int(i64),
    Float(f64),
    Str(String),
    Bool(bool),
    Char(char),
    Null,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EnumVariantPattern {
    pub enum_name: Option<String>,
    pub variant_name: String,
    pub bindings: Vec<Pattern>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OrPattern {
    pub alternatives: Vec<Pattern>,
    pub span: Span,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Span;
    fn s() -> Span {
        Span::dummy()
    }

    #[test]
    fn test_binary_op_str() {
        assert_eq!(BinaryOp::Add.as_str(), "+");
        assert_eq!(BinaryOp::Eq.as_str(), "==");
        assert_eq!(BinaryOp::And.as_str(), "&&");
    }

    #[test]
    fn test_expr_span_accessible() {
        let e = Expr::Int(IntLit {
            value: 1,
            span: Span::new(0, 1, 1, 1),
        });
        assert_eq!(e.span().line, 1);
    }

    #[test]
    fn test_box_recursive_type_compiles() {
        let inner = Box::new(Expr::Int(IntLit {
            value: 1,
            span: s(),
        }));
        let outer = Expr::Unary(UnaryExpr {
            op: UnaryOp::Neg,
            operand: inner,
            span: s(),
        });
        assert!(matches!(outer, Expr::Unary(_)));
    }

    #[test]
    fn test_fn_body_variants() {
        let b = FnBody::Block(Block {
            statements: vec![],
            span: s(),
        });
        let a = FnBody::Arrow(Box::new(Expr::Null(NullLit { span: s() })));
        assert!(matches!(b, FnBody::Block(_)));
        assert!(matches!(a, FnBody::Arrow(_)));
    }

    #[test]
    fn test_all_statement_spans() {
        let stmts = [
            Statement::Break(BreakStmt {
                label: None,
                span: Span::new(0, 5, 1, 1),
            }),
            Statement::Continue(ContinueStmt {
                label: None,
                span: Span::new(0, 8, 2, 1),
            }),
        ];
        assert_eq!(stmts[0].span().line, 1);
        assert_eq!(stmts[1].span().line, 2);
    }

    #[test]
    fn test_type_optional_wraps_inner() {
        let inner = TypeExpr::Named(NamedType {
            name: "int".to_string(),
            span: s(),
        });
        let opt = TypeExpr::Optional(Box::new(inner), s());
        assert!(matches!(opt, TypeExpr::Optional(_, _)));
    }

    #[test]
    fn test_enum_variant_kinds() {
        let unit = EnumVariantKind::Unit;
        let tuple = EnumVariantKind::Tuple(vec![]);
        let struct_ = EnumVariantKind::Struct(vec![]);
        assert!(matches!(unit, EnumVariantKind::Unit));
        assert!(matches!(tuple, EnumVariantKind::Tuple(_)));
        assert!(matches!(struct_, EnumVariantKind::Struct(_)));
    }

    #[test]
    fn test_pattern_or() {
        let p = Pattern::Or(OrPattern {
            alternatives: vec![Pattern::Wildcard(s()), Pattern::Wildcard(s())],
            span: s(),
        });
        if let Pattern::Or(o) = p {
            assert_eq!(o.alternatives.len(), 2);
        }
    }
}
