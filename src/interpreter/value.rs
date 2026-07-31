use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;

use crate::parser::ast::{FnBody, Param};

use super::env::Environment;
use super::error::RuntimeError;

/// Runtime value produced and consumed by the tree-walking interpreter.
///
/// Note: function closures point at a *shared* `RefCell<Environment>` so that
/// self-recursive and mutually-recursive calls can resolve their own names
/// (a plain owned snapshot could never see itself).
#[derive(Debug, Clone)]
pub enum Value {
    // ── PRIMITIVES ───────────────────────────────────────────
    Int(i64),
    Float(f64),
    Str(String),
    Bool(bool),
    Char(char),
    Null,
    /// Returned by functions with no return value.
    Void,

    // ── COMPOUND ─────────────────────────────────────────────
    Array(Vec<Value>),
    Struct {
        name: String,
        fields: HashMap<String, Value>,
    },

    // ── CALLABLE ─────────────────────────────────────────────
    Function {
        name: String,
        params: Vec<Param>,
        body: FnBody,
        /// Captured variables (the lexical scope at definition time).
        closure: Rc<RefCell<Environment>>,
    },
    Builtin {
        name: &'static str,
        func: fn(Vec<Value>) -> Result<Value, RuntimeError>,
    },

    // ── CONTROL FLOW SIGNALS ─────────────────────────────────
    /// Wraps a return value — propagates up until call_function() catches it.
    Return(Box<Value>),
    /// Break signal — propagates up until while/for/loop catches it.
    Break,
    /// Continue signal — propagates up until while/for/loop catches it.
    Continue,

    // ── ERROR / RESULT ────────────────────────────────────────
    /// The success side of a result.
    Ok(Box<Value>),
    /// The failure side of a result.
    Err(Box<Value>),
}

impl Value {
    /// Is this value "truthy"? (used in if/while conditions)
    pub fn is_truthy(&self) -> bool {
        match self {
            Value::Bool(b) => *b,
            Value::Null => false,
            Value::Int(n) => *n != 0,
            Value::Float(f) => *f != 0.0,
            Value::Str(s) => !s.is_empty(),
            Value::Array(a) => !a.is_empty(),
            Value::Void => false,
            _ => true,
        }
    }

    /// Human-readable type name for error messages.
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Int(_) => "int",
            Value::Float(_) => "float",
            Value::Str(_) => "str",
            Value::Bool(_) => "bool",
            Value::Char(_) => "char",
            Value::Null => "null",
            Value::Void => "void",
            Value::Array(_) => "array",
            Value::Struct { .. } => "struct",
            Value::Function { .. } => "function",
            Value::Builtin { .. } => "builtin function",
            Value::Return(_) => "return signal",
            Value::Break => "break signal",
            Value::Continue => "continue signal",
            Value::Ok(_) => "Result::Ok",
            Value::Err(_) => "Result::Err",
        }
    }

    /// Is this a control-flow signal (Return/Break/Continue)?
    pub fn is_signal(&self) -> bool {
        matches!(self, Value::Return(_) | Value::Break | Value::Continue)
    }

    /// Unwrap Int or return a type error.
    pub fn as_int(&self) -> Result<i64, RuntimeError> {
        match self {
            Value::Int(n) => Ok(*n),
            Value::Float(f) => Ok(*f as i64),
            other => Err(RuntimeError::TypeError {
                expected: "int".to_string(),
                got: other.type_name().to_string(),
            }),
        }
    }

    /// Unwrap Float or return a type error; Int auto-coerces to float.
    pub fn as_float(&self) -> Result<f64, RuntimeError> {
        match self {
            Value::Float(f) => Ok(*f),
            Value::Int(n) => Ok(*n as f64),
            other => Err(RuntimeError::TypeError {
                expected: "float".to_string(),
                got: other.type_name().to_string(),
            }),
        }
    }

    /// Unwrap Bool or return a type error.
    pub fn as_bool(&self) -> Result<bool, RuntimeError> {
        match self {
            Value::Bool(b) => Ok(*b),
            other => Err(RuntimeError::TypeError {
                expected: "bool".to_string(),
                got: other.type_name().to_string(),
            }),
        }
    }

    /// Unwrap Str or return a type error.
    pub fn as_str(&self) -> Result<&str, RuntimeError> {
        match self {
            Value::Str(s) => Ok(s.as_str()),
            other => Err(RuntimeError::TypeError {
                expected: "str".to_string(),
                got: other.type_name().to_string(),
            }),
        }
    }

    /// Convert to a printable string.
    pub fn to_display_string(&self) -> String {
        match self {
            Value::Int(n) => n.to_string(),
            Value::Float(f) => {
                if f.fract() == 0.0 {
                    format!("{f:.1}")
                } else {
                    f.to_string()
                }
            }
            Value::Str(s) => s.clone(),
            Value::Bool(b) => b.to_string(),
            Value::Char(c) => c.to_string(),
            Value::Null => "null".to_string(),
            Value::Void => "void".to_string(),
            Value::Array(items) => {
                let parts: Vec<String> = items.iter().map(|v| v.to_display_string()).collect();
                format!("[{}]", parts.join(", "))
            }
            Value::Struct { name, fields } => {
                let mut pairs: Vec<String> = fields
                    .iter()
                    .map(|(k, v)| format!("{k}: {}", v.to_display_string()))
                    .collect();
                pairs.sort();
                format!("{name} {{ {} }}", pairs.join(", "))
            }
            Value::Function { name, .. } => format!("<fn {name}>"),
            Value::Builtin { name, .. } => format!("<builtin {name}>"),
            Value::Return(v) => format!("return({})", v.to_display_string()),
            Value::Break => "break".to_string(),
            Value::Continue => "continue".to_string(),
            Value::Ok(v) => format!("Ok({v})"),
            Value::Err(v) => format!("Err({v})"),
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_display_string())
    }
}

/// Structural equality. Structs, functions, builtins, and control-flow
/// signals never compare equal (functions and structs have identity).
impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Int(a), Value::Int(b)) => a == b,
            (Value::Float(a), Value::Float(b)) => a == b,
            (Value::Str(a), Value::Str(b)) => a == b,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Char(a), Value::Char(b)) => a == b,
            (Value::Null, Value::Null) => true,
            (Value::Void, Value::Void) => true,
            (Value::Array(a), Value::Array(b)) => a == b,
            _ => false,
        }
    }
}

#[cfg(test)]
mod value_tests {
    use super::*;
    use crate::lexer::Span;
    use crate::parser::ast::Block;

    fn s() -> Span {
        Span::dummy()
    }

    fn fn_value() -> Value {
        Value::Function {
            name: "add".to_string(),
            params: vec![],
            body: FnBody::Block(Block {
                statements: vec![],
                span: s(),
            }),
            closure: Rc::new(RefCell::new(Environment::default())),
        }
    }

    fn builtin_value() -> Value {
        Value::Builtin {
            name: "print",
            func: |_| Ok(Value::Void),
        }
    }

    fn struct_value(name: &str, fields: Vec<(&str, Value)>) -> Value {
        Value::Struct {
            name: name.to_string(),
            fields: fields
                .into_iter()
                .map(|(k, v)| (k.to_string(), v))
                .collect(),
        }
    }

    // ── TRUTHINESS ───────────────────────────────────────────

    #[test]
    fn test_int_truthy() {
        assert!(Value::Int(1).is_truthy());
    }

    #[test]
    fn test_int_zero_falsy() {
        assert!(!Value::Int(0).is_truthy());
    }

    #[test]
    fn test_bool_true() {
        assert!(Value::Bool(true).is_truthy());
    }

    #[test]
    fn test_bool_false() {
        assert!(!Value::Bool(false).is_truthy());
    }

    #[test]
    fn test_null_falsy() {
        assert!(!Value::Null.is_truthy());
    }

    #[test]
    fn test_empty_str_falsy() {
        assert!(!Value::Str(String::new()).is_truthy());
    }

    #[test]
    fn test_str_truthy() {
        assert!(Value::Str("hello".to_string()).is_truthy());
    }

    #[test]
    fn test_void_and_empty_array_falsy() {
        assert!(!Value::Void.is_truthy());
        assert!(!Value::Array(vec![]).is_truthy());
        assert!(Value::Array(vec![Value::Int(1)]).is_truthy());
        assert!(Value::Float(0.5).is_truthy());
        assert!(!Value::Float(0.0).is_truthy());
    }

    // ── TYPE NAMES ───────────────────────────────────────────

    #[test]
    fn test_type_names() {
        assert_eq!(Value::Int(0).type_name(), "int");
        assert_eq!(Value::Float(0.0).type_name(), "float");
        assert_eq!(Value::Str(String::new()).type_name(), "str");
        assert_eq!(Value::Bool(true).type_name(), "bool");
        assert_eq!(Value::Char('x').type_name(), "char");
        assert_eq!(Value::Null.type_name(), "null");
        assert_eq!(Value::Void.type_name(), "void");
    }

    #[test]
    fn test_compound_and_callable_type_names() {
        assert_eq!(Value::Array(vec![]).type_name(), "array");
        assert_eq!(struct_value("P", vec![]).type_name(), "struct");
        assert_eq!(fn_value().type_name(), "function");
        assert_eq!(builtin_value().type_name(), "builtin function");
    }

    #[test]
    fn test_signal_and_result_type_names() {
        assert_eq!(
            Value::Return(Box::new(Value::Void)).type_name(),
            "return signal"
        );
        assert_eq!(Value::Break.type_name(), "break signal");
        assert_eq!(Value::Continue.type_name(), "continue signal");
        assert_eq!(Value::Ok(Box::new(Value::Int(1))).type_name(), "Result::Ok");
        assert_eq!(
            Value::Err(Box::new(Value::Int(1))).type_name(),
            "Result::Err"
        );
    }

    // ── AS_* ACCESSORS ───────────────────────────────────────

    #[test]
    fn test_as_int_ok() {
        assert_eq!(Value::Int(42).as_int().unwrap(), 42);
    }

    #[test]
    fn test_as_int_from_float() {
        assert_eq!(Value::Float(3.9).as_int().unwrap(), 3);
    }

    #[test]
    fn test_as_int_err() {
        assert!(Value::Str("x".to_string()).as_int().is_err());
    }

    #[test]
    fn test_int_coerces_to_float() {
        assert_eq!(Value::Int(5).as_float().unwrap(), 5.0);
    }

    #[test]
    fn test_as_float_err() {
        assert!(Value::Bool(true).as_float().is_err());
    }

    #[test]
    fn test_as_bool_ok() {
        assert!(!Value::Bool(false).as_bool().unwrap());
    }

    #[test]
    fn test_as_bool_err() {
        assert!(Value::Null.as_bool().is_err());
    }

    #[test]
    fn test_as_str_ok() {
        assert_eq!(Value::Str("hey".to_string()).as_str().unwrap(), "hey");
    }

    #[test]
    fn test_as_str_err() {
        match Value::Int(1).as_str() {
            Err(RuntimeError::TypeError { expected, got }) => {
                assert_eq!(expected, "str");
                assert_eq!(got, "int");
            }
            _ => panic!("expected type error"),
        }
    }

    // ── DISPLAY ──────────────────────────────────────────────

    #[test]
    fn test_display_int() {
        assert_eq!(Value::Int(42).to_string(), "42");
    }

    #[test]
    fn test_display_float() {
        assert_eq!(Value::Float(3.25).to_string(), "3.25");
        assert_eq!(Value::Float(3.0).to_string(), "3.0");
    }

    #[test]
    fn test_display_str() {
        assert_eq!(Value::Str("hi".to_string()).to_string(), "hi");
    }

    #[test]
    fn test_display_bool() {
        assert_eq!(Value::Bool(true).to_string(), "true");
    }

    #[test]
    fn test_display_array() {
        let v = Value::Array(vec![Value::Int(1), Value::Int(2), Value::Int(3)]);
        assert_eq!(v.to_string(), "[1, 2, 3]");
    }

    #[test]
    fn test_display_struct_sorted() {
        let v = struct_value("Point", vec![("y", Value::Int(4)), ("x", Value::Int(1))]);
        assert_eq!(v.to_string(), "Point { x: 1, y: 4 }");
    }

    #[test]
    fn test_display_function_builtin() {
        assert_eq!(fn_value().to_display_string(), "<fn add>");
        assert_eq!(builtin_value().to_display_string(), "<builtin print>");
    }

    #[test]
    fn test_display_ok_err_return() {
        assert_eq!(
            Value::Ok(Box::new(Value::Int(5))).to_display_string(),
            "Ok(5)"
        );
        assert_eq!(
            Value::Err(Box::new(Value::Str("boom".to_string()))).to_display_string(),
            "Err(boom)"
        );
        assert_eq!(
            Value::Return(Box::new(Value::Int(7))).to_display_string(),
            "return(7)"
        );
    }

    // ── SIGNALS ──────────────────────────────────────────────

    #[test]
    fn test_is_signal() {
        assert!(Value::Return(Box::new(Value::Void)).is_signal());
        assert!(Value::Break.is_signal());
        assert!(Value::Continue.is_signal());
        assert!(!Value::Int(0).is_signal());
        assert!(!Value::Null.is_signal());
    }

    // ── PARTIAL EQUALITY ─────────────────────────────────────

    #[test]
    fn test_partial_eq() {
        assert_eq!(Value::Int(1), Value::Int(1));
        assert_ne!(Value::Int(1), Value::Int(2));
        assert_ne!(Value::Int(1), Value::Str("1".to_string()));
        assert_eq!(Value::Null, Value::Null);
        assert_ne!(Value::Null, Value::Void);
        assert_eq!(Value::Bool(true), Value::Bool(true));
        assert_eq!(Value::Str("a".to_string()), Value::Str("a".to_string()));
        assert_ne!(Value::Int(1), Value::Float(1.0));
    }

    #[test]
    fn test_structs_and_signals_never_equal() {
        assert_ne!(
            struct_value("P", vec![("x", Value::Int(1))]),
            struct_value("P", vec![("x", Value::Int(1))])
        );
        assert_ne!(
            Value::Ok(Box::new(Value::Int(1))),
            Value::Ok(Box::new(Value::Int(1)))
        );
        assert_ne!(fn_value(), fn_value());
        assert_ne!(Value::Break, Value::Continue);
    }
}
