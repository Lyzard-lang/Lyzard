use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;

use crate::parser::ast::{FnBody, Param};

use super::env::Environment;
use super::error::RuntimeError;

/// Runtime value produced and consumed by the tree-walking interpreter.
#[derive(Debug, Clone)]
pub enum Value {
    Int(i64),
    Float(f64),
    Str(String),
    Bool(bool),
    Char(char),
    Null,
    Void,
    Ok(Box<Value>),
    Err(Box<Value>),
    Return(Box<Value>),
    Break,
    Continue,
    Array(Vec<Value>),
    Struct(String, Vec<(String, Value)>),
    Function {
        name: String,
        params: Vec<Param>,
        body: FnBody,
        closure: Rc<RefCell<Environment>>,
    },
    Builtin {
        name: &'static str,
        func: fn(Vec<Value>) -> Result<Value, RuntimeError>,
    },
}

impl Value {
    /// Truthiness used by `if`, `while`, and `!`:
    /// null, void, break, continue, 0, 0.0, "", empty arrays are falsy.
    pub fn is_truthy(&self) -> bool {
        match self {
            Value::Null | Value::Void | Value::Break | Value::Continue => false,
            Value::Bool(b) => *b,
            Value::Int(i) => *i != 0,
            Value::Float(f) => *f != 0.0,
            Value::Str(s) => !s.is_empty(),
            Value::Char(c) => *c != '\0',
            Value::Array(a) => !a.is_empty(),
            _ => true,
        }
    }

    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Int(_) => "int",
            Value::Float(_) => "float",
            Value::Str(_) => "string",
            Value::Bool(_) => "bool",
            Value::Char(_) => "char",
            Value::Null => "null",
            Value::Void => "void",
            Value::Ok(_) => "ok",
            Value::Err(_) => "err",
            Value::Return(_) => "return",
            Value::Break => "break",
            Value::Continue => "continue",
            Value::Array(_) => "array",
            Value::Struct(_, _) => "struct",
            Value::Function { .. } => "function",
            Value::Builtin { .. } => "builtin",
        }
    }

    pub fn as_int(&self) -> Result<i64, RuntimeError> {
        match self {
            Value::Int(i) => Ok(*i),
            Value::Float(f) => Ok(*f as i64),
            other => Err(RuntimeError::TypeError {
                expected: "int".to_string(),
                got: other.type_name().to_string(),
            }),
        }
    }

    pub fn as_float(&self) -> Result<f64, RuntimeError> {
        match self {
            Value::Float(f) => Ok(*f),
            Value::Int(i) => Ok(*i as f64),
            other => Err(RuntimeError::TypeError {
                expected: "float".to_string(),
                got: other.type_name().to_string(),
            }),
        }
    }

    pub fn as_bool(&self) -> Result<bool, RuntimeError> {
        match self {
            Value::Bool(b) => Ok(*b),
            other => Err(RuntimeError::TypeError {
                expected: "bool".to_string(),
                got: other.type_name().to_string(),
            }),
        }
    }

    pub fn as_str(&self) -> Result<String, RuntimeError> {
        match self {
            Value::Str(s) => Ok(s.clone()),
            other => Err(RuntimeError::TypeError {
                expected: "string".to_string(),
                got: other.type_name().to_string(),
            }),
        }
    }

    /// Control-flow signals that must propagate out of blocks.
    pub fn is_signal(&self) -> bool {
        matches!(self, Value::Return(_) | Value::Break | Value::Continue)
    }

    pub fn to_display_string(&self) -> String {
        match self {
            Value::Int(i) => i.to_string(),
            Value::Float(f) => {
                if f.fract() == 0.0 && f.abs() < 1e15 {
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
            Value::Ok(v) => format!("ok({v})"),
            Value::Err(v) => format!("err({v})"),
            Value::Array(items) => {
                let parts: Vec<String> = items.iter().map(|v| v.to_display_string()).collect();
                format!("[{}]", parts.join(", "))
            }
            Value::Struct(name, fields) => {
                let parts: Vec<String> = fields
                    .iter()
                    .map(|(k, v)| format!("{k}: {}", v.to_display_string()))
                    .collect();
                format!("{name} {{ {} }}", parts.join(", "))
            }
            Value::Function { name, .. } => format!("<fn {name}>"),
            Value::Builtin { name, .. } => format!("<builtin {name}>"),
            Value::Return(v) => v.to_display_string(),
            Value::Break => "break".to_string(),
            Value::Continue => "continue".to_string(),
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
            (Value::Ok(a), Value::Ok(b)) => a == b,
            (Value::Err(a), Value::Err(b)) => a == b,
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

    #[test]
    fn test_int_type_name() {
        assert_eq!(Value::Int(3).type_name(), "int");
    }

    #[test]
    fn test_float_type_name() {
        assert_eq!(Value::Float(1.5).type_name(), "float");
    }

    #[test]
    fn test_str_type_name() {
        assert_eq!(Value::Str("hi".to_string()).type_name(), "string");
    }

    #[test]
    fn test_bool_type_name() {
        assert_eq!(Value::Bool(true).type_name(), "bool");
    }

    #[test]
    fn test_char_type_name() {
        assert_eq!(Value::Char('x').type_name(), "char");
    }

    #[test]
    fn test_null_void_type_names() {
        assert_eq!(Value::Null.type_name(), "null");
        assert_eq!(Value::Void.type_name(), "void");
    }

    #[test]
    fn test_ok_err_type_names() {
        assert_eq!(Value::Ok(Box::new(Value::Int(1))).type_name(), "ok");
        assert_eq!(Value::Err(Box::new(Value::Int(1))).type_name(), "err");
    }

    #[test]
    fn test_signal_type_names() {
        assert_eq!(Value::Break.type_name(), "break");
        assert_eq!(Value::Continue.type_name(), "continue");
        assert_eq!(Value::Return(Box::new(Value::Void)).type_name(), "return");
    }

    #[test]
    fn test_array_and_callable_type_names() {
        assert_eq!(Value::Array(vec![]).type_name(), "array");
        assert_eq!(Value::Struct("P".to_string(), vec![]).type_name(), "struct");
        assert_eq!(fn_value().type_name(), "function");
        assert_eq!(builtin_value().type_name(), "builtin");
    }

    #[test]
    fn test_truthiness() {
        assert!(!Value::Null.is_truthy());
        assert!(!Value::Void.is_truthy());
        assert!(!Value::Break.is_truthy());
        assert!(!Value::Continue.is_truthy());
        assert!(Value::Int(1).is_truthy());
        assert!(!Value::Int(0).is_truthy());
        assert!(Value::Float(0.5).is_truthy());
        assert!(!Value::Float(0.0).is_truthy());
        assert!(Value::Str("a".to_string()).is_truthy());
        assert!(!Value::Str(String::new()).is_truthy());
        assert!(!Value::Array(vec![]).is_truthy());
        assert!(Value::Array(vec![Value::Int(1)]).is_truthy());
    }

    #[test]
    fn test_as_int_ok() {
        assert_eq!(Value::Int(42).as_int().unwrap(), 42);
    }

    #[test]
    fn test_as_int_from_float() {
        assert_eq!(Value::Float(3.9).as_int().unwrap(), 3);
    }

    #[test]
    fn test_as_int_error() {
        match Value::Str("x".to_string()).as_int() {
            Err(RuntimeError::TypeError { expected, got }) => {
                assert_eq!(expected, "int");
                assert_eq!(got, "string");
            }
            _ => panic!("expected type error"),
        }
    }

    #[test]
    fn test_as_float_from_int() {
        assert_eq!(Value::Int(5).as_float().unwrap(), 5.0);
    }

    #[test]
    fn test_as_float_error() {
        match Value::Bool(true).as_float() {
            Err(RuntimeError::TypeError { expected, got }) => {
                assert_eq!(expected, "float");
                assert_eq!(got, "bool");
            }
            _ => panic!("expected type error"),
        }
    }

    #[test]
    fn test_as_bool_ok() {
        assert!(!Value::Bool(false).as_bool().unwrap());
    }

    #[test]
    fn test_as_bool_error() {
        match Value::Null.as_bool() {
            Err(RuntimeError::TypeError { expected, got }) => {
                assert_eq!(expected, "bool");
                assert_eq!(got, "null");
            }
            _ => panic!("expected type error"),
        }
    }

    #[test]
    fn test_as_str_ok() {
        assert_eq!(Value::Str("hey".to_string()).as_str().unwrap(), "hey");
    }

    #[test]
    fn test_as_str_error() {
        match Value::Int(1).as_str() {
            Err(RuntimeError::TypeError { expected, got }) => {
                assert_eq!(expected, "string");
                assert_eq!(got, "int");
            }
            _ => panic!("expected type error"),
        }
    }

    #[test]
    fn test_display_int() {
        assert_eq!(Value::Int(7).to_display_string(), "7");
    }

    #[test]
    fn test_display_float_integral() {
        assert_eq!(Value::Float(3.0).to_display_string(), "3.0");
    }

    #[test]
    fn test_display_float_fraction() {
        assert_eq!(Value::Float(3.5).to_display_string(), "3.5");
    }

    #[test]
    fn test_display_str() {
        assert_eq!(Value::Str("hi".to_string()).to_display_string(), "hi");
    }

    #[test]
    fn test_display_array_and_struct() {
        assert_eq!(
            Value::Array(vec![Value::Int(1), Value::Int(2)]).to_display_string(),
            "[1, 2]"
        );
        assert_eq!(
            Value::Struct("Point".to_string(), vec![("x".to_string(), Value::Int(1))])
                .to_display_string(),
            "Point { x: 1 }"
        );
    }

    #[test]
    fn test_display_function_builtin() {
        assert_eq!(fn_value().to_display_string(), "<fn add>");
        assert_eq!(builtin_value().to_display_string(), "<builtin print>");
    }

    #[test]
    fn test_display_ok_err() {
        assert_eq!(
            Value::Ok(Box::new(Value::Int(5))).to_display_string(),
            "ok(5)"
        );
        assert_eq!(
            Value::Err(Box::new(Value::Str("boom".to_string()))).to_display_string(),
            "err(boom)"
        );
    }

    #[test]
    fn test_signal_detection() {
        assert!(Value::Return(Box::new(Value::Void)).is_signal());
        assert!(Value::Break.is_signal());
        assert!(Value::Continue.is_signal());
        assert!(!Value::Int(1).is_signal());
        assert!(!Value::Null.is_signal());
    }

    #[test]
    fn test_partial_eq() {
        assert_eq!(Value::Int(1), Value::Int(1));
        assert_ne!(Value::Int(1), Value::Float(1.0));
        assert_eq!(Value::Str("a".to_string()), Value::Str("a".to_string()));
        assert_eq!(Value::Null, Value::Null);
        assert_ne!(Value::Null, Value::Void);
        assert_eq!(
            Value::Ok(Box::new(Value::Int(1))),
            Value::Ok(Box::new(Value::Int(1)))
        );
        assert_ne!(
            Value::Ok(Box::new(Value::Int(1))),
            Value::Err(Box::new(Value::Int(1)))
        );
        assert_ne!(fn_value(), fn_value());
        assert_ne!(
            Value::Struct("P".to_string(), vec![]),
            Value::Struct("P".to_string(), vec![])
        );
    }

    #[test]
    fn test_fmt_display() {
        assert_eq!(format!("{}", Value::Int(3)), "3");
        assert_eq!(format!("{}", Value::Bool(true)), "true");
    }
}
