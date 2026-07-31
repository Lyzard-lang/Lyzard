pub mod builtins;
pub mod env;
pub mod error;
pub mod value;

use crate::lexer::Span;
use crate::parser::ast::*;

use self::builtins::all_builtins;
use self::env::Environment;
use self::error::RuntimeError;
use self::value::Value;

/// Tree-walking interpreter: walks the validated AST and produces values.
#[derive(Debug)]
pub struct Interpreter {
    pub env: Environment,
    pub output: Vec<String>,
    pub capture_output: bool,
}

impl Default for Interpreter {
    fn default() -> Self {
        Self::new()
    }
}

impl Interpreter {
    pub fn new() -> Self {
        let mut env = Environment::new();
        for (name, _, func) in all_builtins() {
            env.define(name, Value::Builtin { name, func });
        }
        Interpreter {
            env,
            output: Vec::new(),
            capture_output: false,
        }
    }

    /// Execute a whole program: register top-level functions first so
    /// forward references work, then run every other top-level declaration.
    pub fn run(&mut self, program: &Program) -> Result<(), RuntimeError> {
        for decl in &program.declarations {
            if let Declaration::Function(f) = decl {
                self.env.define(
                    &f.name,
                    Value::Function {
                        name: f.name.clone(),
                        params: f.params.clone(),
                        body: f.body.clone(),
                        closure: self.env.clone(),
                    },
                );
            }
        }

        for decl in &program.declarations {
            match decl {
                Declaration::Function(_) => {}
                Declaration::Let(d) => {
                    let value = self.eval_expr(&d.value)?;
                    self.env.define(&d.name, value);
                }
                Declaration::Const(c) => {
                    let value = self.eval_expr(&c.value)?;
                    self.env.define(&c.name, value);
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// Evaluate an expression into a runtime value.
    pub fn eval_expr(&mut self, expr: &Expr) -> Result<Value, RuntimeError> {
        match expr {
            Expr::Int(i) => Ok(Value::Int(i.value)),
            Expr::Float(f) => Ok(Value::Float(f.value)),
            Expr::Str(s) => Ok(Value::Str(s.value.clone())),
            Expr::Bool(b) => Ok(Value::Bool(b.value)),
            Expr::Char(c) => Ok(Value::Char(c.value)),
            Expr::Null(_) => Ok(Value::Null),
            Expr::Identifier(id) => {
                self.env
                    .lookup(&id.name)
                    .cloned()
                    .ok_or_else(|| RuntimeError::UndefinedName {
                        name: id.name.clone(),
                        span: id.span,
                    })
            }
            Expr::Array(arr) => {
                let mut items = Vec::new();
                for e in &arr.elements {
                    items.push(self.eval_expr(e)?);
                }
                Ok(Value::Array(items))
            }
            Expr::StructInit(si) => {
                let mut fields = Vec::new();
                for (name, e) in &si.fields {
                    fields.push((name.clone(), self.eval_expr(e)?));
                }
                Ok(Value::Struct(si.name.clone(), fields))
            }
            Expr::Binary(bin) => {
                let left = self.eval_expr(&bin.left)?;
                let right = self.eval_expr(&bin.right)?;
                self.eval_binary(bin.op, left, right, bin.span)
            }
            Expr::Unary(un) => {
                let operand = self.eval_expr(&un.operand)?;
                self.eval_unary(un.op, operand)
            }
            _ => Err(RuntimeError::NotImplemented {
                feature: "this expression".to_string(),
                span: expr.span(),
            }),
        }
    }

    fn eval_binary(
        &mut self,
        op: BinaryOp,
        left: Value,
        right: Value,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        match (op, left, right) {
            (BinaryOp::Add, Value::Int(a), Value::Int(b)) => Ok(Value::Int(a + b)),
            (BinaryOp::Add, Value::Float(a), Value::Float(b)) => Ok(Value::Float(a + b)),
            (BinaryOp::Add, Value::Int(a), Value::Float(b)) => Ok(Value::Float(a as f64 + b)),
            (BinaryOp::Add, Value::Float(a), Value::Int(b)) => Ok(Value::Float(a + b as f64)),
            (BinaryOp::Add, Value::Str(a), Value::Str(b)) => Ok(Value::Str(a + &b)),
            (BinaryOp::Add, Value::Str(a), other) => Ok(Value::Str(a + &other.to_display_string())),

            (BinaryOp::Sub, Value::Int(a), Value::Int(b)) => Ok(Value::Int(a - b)),
            (BinaryOp::Sub, Value::Float(a), Value::Float(b)) => Ok(Value::Float(a - b)),
            (BinaryOp::Sub, Value::Int(a), Value::Float(b)) => Ok(Value::Float(a as f64 - b)),
            (BinaryOp::Sub, Value::Float(a), Value::Int(b)) => Ok(Value::Float(a - b as f64)),

            (BinaryOp::Mul, Value::Int(a), Value::Int(b)) => Ok(Value::Int(a * b)),
            (BinaryOp::Mul, Value::Float(a), Value::Float(b)) => Ok(Value::Float(a * b)),
            (BinaryOp::Mul, Value::Int(a), Value::Float(b)) => Ok(Value::Float(a as f64 * b)),
            (BinaryOp::Mul, Value::Float(a), Value::Int(b)) => Ok(Value::Float(a * b as f64)),

            (BinaryOp::Div, Value::Int(a), Value::Int(b)) => {
                if b == 0 {
                    return Err(RuntimeError::DivisionByZero { span });
                }
                Ok(Value::Int(a / b))
            }
            (BinaryOp::Div, Value::Float(a), Value::Float(b)) => {
                if b == 0.0 {
                    return Err(RuntimeError::DivisionByZero { span });
                }
                Ok(Value::Float(a / b))
            }
            (BinaryOp::Div, Value::Int(a), Value::Float(b)) => {
                if b == 0.0 {
                    return Err(RuntimeError::DivisionByZero { span });
                }
                Ok(Value::Float(a as f64 / b))
            }
            (BinaryOp::Div, Value::Float(a), Value::Int(b)) => {
                if b == 0 {
                    return Err(RuntimeError::DivisionByZero { span });
                }
                Ok(Value::Float(a / b as f64))
            }

            (BinaryOp::Mod, Value::Int(a), Value::Int(b)) => {
                if b == 0 {
                    return Err(RuntimeError::DivisionByZero { span });
                }
                Ok(Value::Int(a % b))
            }
            (BinaryOp::Mod, Value::Float(a), Value::Float(b)) => {
                if b == 0.0 {
                    return Err(RuntimeError::DivisionByZero { span });
                }
                Ok(Value::Float(a % b))
            }
            (BinaryOp::Mod, Value::Int(a), Value::Float(b)) => {
                if b == 0.0 {
                    return Err(RuntimeError::DivisionByZero { span });
                }
                Ok(Value::Float(a as f64 % b))
            }
            (BinaryOp::Mod, Value::Float(a), Value::Int(b)) => {
                if b == 0 {
                    return Err(RuntimeError::DivisionByZero { span });
                }
                Ok(Value::Float(a % b as f64))
            }

            (BinaryOp::Lt, Value::Int(a), Value::Int(b)) => Ok(Value::Bool(a < b)),
            (BinaryOp::Lt, Value::Float(a), Value::Float(b)) => Ok(Value::Bool(a < b)),
            (BinaryOp::Lt, Value::Int(a), Value::Float(b)) => Ok(Value::Bool((a as f64) < b)),
            (BinaryOp::Lt, Value::Float(a), Value::Int(b)) => Ok(Value::Bool(a < b as f64)),
            (BinaryOp::Lte, Value::Int(a), Value::Int(b)) => Ok(Value::Bool(a <= b)),
            (BinaryOp::Lte, Value::Float(a), Value::Float(b)) => Ok(Value::Bool(a <= b)),
            (BinaryOp::Lte, Value::Int(a), Value::Float(b)) => Ok(Value::Bool((a as f64) <= b)),
            (BinaryOp::Lte, Value::Float(a), Value::Int(b)) => Ok(Value::Bool(a <= b as f64)),
            (BinaryOp::Gt, Value::Int(a), Value::Int(b)) => Ok(Value::Bool(a > b)),
            (BinaryOp::Gt, Value::Float(a), Value::Float(b)) => Ok(Value::Bool(a > b)),
            (BinaryOp::Gt, Value::Int(a), Value::Float(b)) => Ok(Value::Bool((a as f64) > b)),
            (BinaryOp::Gt, Value::Float(a), Value::Int(b)) => Ok(Value::Bool(a > b as f64)),
            (BinaryOp::Gte, Value::Int(a), Value::Int(b)) => Ok(Value::Bool(a >= b)),
            (BinaryOp::Gte, Value::Float(a), Value::Float(b)) => Ok(Value::Bool(a >= b)),
            (BinaryOp::Gte, Value::Int(a), Value::Float(b)) => Ok(Value::Bool((a as f64) >= b)),
            (BinaryOp::Gte, Value::Float(a), Value::Int(b)) => Ok(Value::Bool(a >= b as f64)),

            (BinaryOp::Eq, a, b) => Ok(Value::Bool(a == b)),
            (BinaryOp::NotEq, a, b) => Ok(Value::Bool(a != b)),
            (BinaryOp::And, a, b) => Ok(Value::Bool(a.is_truthy() && b.is_truthy())),
            (BinaryOp::Or, a, b) => Ok(Value::Bool(a.is_truthy() || b.is_truthy())),

            (op, left, right) => Err(RuntimeError::TypeError {
                expected: format!("operands for {}", op.as_str()),
                got: format!("{} and {}", left.type_name(), right.type_name()),
            }),
        }
    }

    fn eval_unary(&mut self, op: UnaryOp, operand: Value) -> Result<Value, RuntimeError> {
        match (op, operand) {
            (UnaryOp::Neg, Value::Int(i)) => Ok(Value::Int(-i)),
            (UnaryOp::Neg, Value::Float(f)) => Ok(Value::Float(-f)),
            (UnaryOp::Not, b) => Ok(Value::Bool(!b.is_truthy())),
            (op, _) => Err(RuntimeError::TypeError {
                expected: format!("valid operand for {}", op.as_str()),
                got: "unsupported type".to_string(),
            }),
        }
    }
}

#[cfg(test)]
mod interpreter_tests {
    use super::*;

    fn interp() -> Interpreter {
        let mut i = Interpreter::new();
        i.env.define("x", Value::Int(42));
        i.env.define("name", Value::Str("hi".to_string()));
        i
    }

    fn s() -> Span {
        Span::dummy()
    }

    fn int_lit(v: i64) -> Expr {
        Expr::Int(IntLit {
            value: v,
            span: s(),
        })
    }
    fn float_lit(v: f64) -> Expr {
        Expr::Float(FloatLit {
            value: v,
            span: s(),
        })
    }
    fn str_lit(v: &str) -> Expr {
        Expr::Str(StrLit {
            value: v.to_string(),
            span: s(),
        })
    }
    fn bool_lit(v: bool) -> Expr {
        Expr::Bool(BoolLit {
            value: v,
            span: s(),
        })
    }
    fn char_lit(v: char) -> Expr {
        Expr::Char(CharLit {
            value: v,
            span: s(),
        })
    }
    fn ident(name: &str) -> Expr {
        Expr::Identifier(IdentExpr {
            name: name.to_string(),
            span: s(),
        })
    }
    fn binary(op: BinaryOp, left: Expr, right: Expr) -> Expr {
        Expr::Binary(BinaryExpr {
            op,
            left: Box::new(left),
            right: Box::new(right),
            span: s(),
        })
    }
    fn unary(op: UnaryOp, operand: Expr) -> Expr {
        Expr::Unary(UnaryExpr {
            op,
            operand: Box::new(operand),
            span: s(),
        })
    }

    #[test]
    fn test_int_literal() {
        assert_eq!(interp().eval_expr(&int_lit(42)).unwrap(), Value::Int(42));
    }

    #[test]
    fn test_float_literal() {
        assert_eq!(
            interp().eval_expr(&float_lit(3.5)).unwrap(),
            Value::Float(3.5)
        );
    }

    #[test]
    fn test_str_literal() {
        assert_eq!(
            interp().eval_expr(&str_lit("hi")).unwrap(),
            Value::Str("hi".to_string())
        );
    }

    #[test]
    fn test_bool_literal() {
        assert_eq!(
            interp().eval_expr(&bool_lit(true)).unwrap(),
            Value::Bool(true)
        );
    }

    #[test]
    fn test_char_literal() {
        assert_eq!(
            interp().eval_expr(&char_lit('z')).unwrap(),
            Value::Char('z')
        );
    }

    #[test]
    fn test_null_literal() {
        assert_eq!(
            interp()
                .eval_expr(&Expr::Null(NullLit { span: s() }))
                .unwrap(),
            Value::Null
        );
    }

    #[test]
    fn test_array_literal() {
        let e = Expr::Array(ArrayLit {
            elements: vec![int_lit(1), int_lit(2)],
            span: s(),
        });
        assert_eq!(
            interp().eval_expr(&e).unwrap(),
            Value::Array(vec![Value::Int(1), Value::Int(2)])
        );
    }

    #[test]
    fn test_struct_init() {
        let e = Expr::StructInit(StructInitExpr {
            name: "Point".to_string(),
            fields: vec![("x".to_string(), int_lit(1)), ("y".to_string(), int_lit(2))],
            span: s(),
        });
        let got = interp().eval_expr(&e).unwrap();
        assert_eq!(got.to_display_string(), "Point { x: 1, y: 2 }");
        assert_eq!(got.type_name(), "struct");
    }

    #[test]
    fn test_identifier_lookup() {
        assert_eq!(interp().eval_expr(&ident("x")).unwrap(), Value::Int(42));
        assert_eq!(
            interp().eval_expr(&ident("name")).unwrap(),
            Value::Str("hi".to_string())
        );
    }

    #[test]
    fn test_undefined_name_error() {
        let mut i = interp();
        assert!(matches!(
            i.eval_expr(&ident("nope")),
            Err(RuntimeError::UndefinedName { .. })
        ));
    }

    #[test]
    fn test_add_ints() {
        assert_eq!(
            interp()
                .eval_expr(&binary(BinaryOp::Add, int_lit(2), int_lit(3)))
                .unwrap(),
            Value::Int(5)
        );
    }

    #[test]
    fn test_add_floats() {
        assert_eq!(
            interp()
                .eval_expr(&binary(BinaryOp::Add, float_lit(1.5), float_lit(2.5)))
                .unwrap(),
            Value::Float(4.0)
        );
    }

    #[test]
    fn test_add_mixed() {
        assert_eq!(
            interp()
                .eval_expr(&binary(BinaryOp::Add, int_lit(1), float_lit(2.5)))
                .unwrap(),
            Value::Float(3.5)
        );
        assert_eq!(
            interp()
                .eval_expr(&binary(BinaryOp::Add, float_lit(2.5), int_lit(1)))
                .unwrap(),
            Value::Float(3.5)
        );
    }

    #[test]
    fn test_add_str_concat() {
        assert_eq!(
            interp()
                .eval_expr(&binary(BinaryOp::Add, str_lit("Hello "), str_lit("World")))
                .unwrap(),
            Value::Str("Hello World".to_string())
        );
    }

    #[test]
    fn test_sub_mul_div() {
        assert_eq!(
            interp()
                .eval_expr(&binary(BinaryOp::Sub, int_lit(10), int_lit(4)))
                .unwrap(),
            Value::Int(6)
        );
        assert_eq!(
            interp()
                .eval_expr(&binary(BinaryOp::Mul, int_lit(6), int_lit(7)))
                .unwrap(),
            Value::Int(42)
        );
        assert_eq!(
            interp()
                .eval_expr(&binary(BinaryOp::Div, int_lit(10), int_lit(2)))
                .unwrap(),
            Value::Int(5)
        );
    }

    #[test]
    fn test_div_by_zero() {
        let mut i = interp();
        assert!(matches!(
            i.eval_expr(&binary(BinaryOp::Div, int_lit(1), int_lit(0))),
            Err(RuntimeError::DivisionByZero { .. })
        ));
        assert!(matches!(
            i.eval_expr(&binary(BinaryOp::Div, float_lit(1.0), float_lit(0.0))),
            Err(RuntimeError::DivisionByZero { .. })
        ));
    }

    #[test]
    fn test_mod() {
        assert_eq!(
            interp()
                .eval_expr(&binary(BinaryOp::Mod, int_lit(10), int_lit(3)))
                .unwrap(),
            Value::Int(1)
        );
    }

    #[test]
    fn test_comparisons() {
        assert_eq!(
            interp()
                .eval_expr(&binary(BinaryOp::Lt, int_lit(1), int_lit(2)))
                .unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            interp()
                .eval_expr(&binary(BinaryOp::Gte, int_lit(3), int_lit(3)))
                .unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            interp()
                .eval_expr(&binary(BinaryOp::Lt, int_lit(1), float_lit(1.5)))
                .unwrap(),
            Value::Bool(true)
        );
    }

    #[test]
    fn test_equality() {
        assert_eq!(
            interp()
                .eval_expr(&binary(BinaryOp::Eq, int_lit(1), int_lit(1)))
                .unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            interp()
                .eval_expr(&binary(BinaryOp::NotEq, int_lit(1), int_lit(2)))
                .unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            interp()
                .eval_expr(&binary(BinaryOp::Eq, str_lit("a"), str_lit("a")))
                .unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            interp()
                .eval_expr(&binary(BinaryOp::Eq, int_lit(1), float_lit(1.0)))
                .unwrap(),
            Value::Bool(false)
        );
    }

    #[test]
    fn test_and_or() {
        assert_eq!(
            interp()
                .eval_expr(&binary(BinaryOp::And, bool_lit(true), bool_lit(false)))
                .unwrap(),
            Value::Bool(false)
        );
        assert_eq!(
            interp()
                .eval_expr(&binary(BinaryOp::Or, bool_lit(true), bool_lit(false)))
                .unwrap(),
            Value::Bool(true)
        );
    }

    #[test]
    fn test_unary_neg() {
        assert_eq!(
            interp()
                .eval_expr(&unary(UnaryOp::Neg, int_lit(5)))
                .unwrap(),
            Value::Int(-5)
        );
        assert_eq!(
            interp()
                .eval_expr(&unary(UnaryOp::Neg, float_lit(2.5)))
                .unwrap(),
            Value::Float(-2.5)
        );
    }

    #[test]
    fn test_unary_not() {
        assert_eq!(
            interp()
                .eval_expr(&unary(UnaryOp::Not, bool_lit(true)))
                .unwrap(),
            Value::Bool(false)
        );
        assert_eq!(
            interp()
                .eval_expr(&unary(UnaryOp::Not, int_lit(0)))
                .unwrap(),
            Value::Bool(true)
        );
    }

    #[test]
    fn test_type_mismatch_error() {
        let mut i = interp();
        assert!(matches!(
            i.eval_expr(&binary(BinaryOp::Sub, str_lit("x"), int_lit(1))),
            Err(RuntimeError::TypeError { .. })
        ));
    }

    #[test]
    fn test_not_implemented_for_call() {
        let mut i = interp();
        let e = Expr::Call(CallExpr {
            callee: Box::new(ident("print")),
            args: vec![],
            span: s(),
        });
        assert!(matches!(
            i.eval_expr(&e),
            Err(RuntimeError::NotImplemented { .. })
        ));
    }
}
