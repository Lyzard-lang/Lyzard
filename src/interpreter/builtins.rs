use crate::lexer::Span;

use super::error::RuntimeError;
use super::value::Value;

pub type BuiltinFn = fn(Vec<Value>) -> Result<Value, RuntimeError>;

/// Every builtin, as `(name, arity, implementation)`.
pub fn all_builtins() -> Vec<(&'static str, usize, BuiltinFn)> {
    vec![
        ("print", 1, builtin_print),
        ("println", 1, builtin_println),
        ("printErr", 1, builtin_print_err),
        ("len", 1, builtin_len),
        ("parseInt", 1, builtin_parse_int),
        ("parseFloat", 1, builtin_parse_float),
        ("toString", 1, builtin_to_string),
        ("range", 2, builtin_range),
        ("assert", 1, builtin_assert),
        ("panic", 1, builtin_panic),
        ("typeOf", 1, builtin_type_of),
        ("push", 2, builtin_push),
        ("pop", 1, builtin_pop),
        ("first", 1, builtin_first),
        ("last", 1, builtin_last),
        ("contains", 2, builtin_contains),
        ("toInt", 1, builtin_to_int),
        ("toFloat", 1, builtin_to_float),
        ("abs", 1, builtin_abs),
        ("min", 2, builtin_min),
        ("max", 2, builtin_max),
    ]
}

fn builtin_print(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if let Some(v) = args.first() {
        print!("{}", v.to_display_string());
    }
    Ok(Value::Void)
}

fn builtin_println(args: Vec<Value>) -> Result<Value, RuntimeError> {
    match args.first() {
        Some(v) => println!("{}", v.to_display_string()),
        None => println!(),
    }
    Ok(Value::Void)
}

fn builtin_print_err(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if let Some(v) = args.first() {
        eprintln!("{}", v.to_display_string());
    }
    Ok(Value::Void)
}

fn builtin_len(args: Vec<Value>) -> Result<Value, RuntimeError> {
    match args.first() {
        Some(Value::Str(s)) => Ok(Value::Int(s.chars().count() as i64)),
        Some(Value::Array(a)) => Ok(Value::Int(a.len() as i64)),
        Some(other) => Err(RuntimeError::TypeError {
            expected: "string or array".to_string(),
            got: other.type_name().to_string(),
        }),
        None => Err(RuntimeError::TypeError {
            expected: "string or array".to_string(),
            got: "nothing".to_string(),
        }),
    }
}

fn builtin_parse_int(args: Vec<Value>) -> Result<Value, RuntimeError> {
    match args.first() {
        Some(Value::Str(s)) => match s.trim().parse::<i64>() {
            Ok(i) => Ok(Value::Int(i)),
            Err(_) => Ok(Value::Null),
        },
        _ => Ok(Value::Null),
    }
}

fn builtin_parse_float(args: Vec<Value>) -> Result<Value, RuntimeError> {
    match args.first() {
        Some(Value::Str(s)) => match s.trim().parse::<f64>() {
            Ok(f) => Ok(Value::Float(f)),
            Err(_) => Ok(Value::Null),
        },
        _ => Ok(Value::Null),
    }
}

fn builtin_to_string(args: Vec<Value>) -> Result<Value, RuntimeError> {
    match args.first() {
        Some(v) => Ok(Value::Str(v.to_display_string())),
        None => Ok(Value::Str(String::new())),
    }
}

fn builtin_range(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let start = args.first().and_then(|v| v.as_int().ok()).unwrap_or(0);
    let end = args.get(1).and_then(|v| v.as_int().ok()).unwrap_or(start);
    let step = args
        .get(2)
        .and_then(|v| v.as_int().ok())
        .filter(|&s| s != 0)
        .unwrap_or(1);
    let mut items = Vec::new();
    let mut i = start;
    if step > 0 {
        while i < end {
            items.push(Value::Int(i));
            i += step;
        }
    } else {
        while i > end {
            items.push(Value::Int(i));
            i += step;
        }
    }
    Ok(Value::Array(items))
}

fn builtin_assert(args: Vec<Value>) -> Result<Value, RuntimeError> {
    match args.first() {
        Some(Value::Bool(true)) => Ok(Value::Void),
        Some(Value::Bool(false)) => Err(RuntimeError::InvalidOperation {
            message: "assertion failed".to_string(),
            span: Span::dummy(),
        }),
        Some(other) => Err(RuntimeError::TypeError {
            expected: "bool".to_string(),
            got: other.type_name().to_string(),
        }),
        None => Ok(Value::Void),
    }
}

fn builtin_panic(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let message = args
        .first()
        .map(|v| v.to_display_string())
        .unwrap_or_else(|| "panic".to_string());
    Err(RuntimeError::InvalidOperation {
        message,
        span: Span::dummy(),
    })
}

fn builtin_type_of(args: Vec<Value>) -> Result<Value, RuntimeError> {
    match args.first() {
        Some(v) => Ok(Value::Str(v.type_name().to_string())),
        None => Ok(Value::Str("null".to_string())),
    }
}

fn builtin_push(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let values: Vec<Value> = args.into_iter().collect();
    match values.as_slice() {
        [Value::Array(arr), item] => {
            let mut new_arr = arr.clone();
            new_arr.push(item.clone());
            Ok(Value::Array(new_arr))
        }
        _ => Err(RuntimeError::TypeError {
            expected: "array".to_string(),
            got: values
                .first()
                .map(|v| v.type_name().to_string())
                .unwrap_or_else(|| "nothing".to_string()),
        }),
    }
}

fn builtin_pop(args: Vec<Value>) -> Result<Value, RuntimeError> {
    match args.first() {
        Some(Value::Array(a)) => Ok(a.last().cloned().unwrap_or(Value::Null)),
        Some(other) => Err(RuntimeError::TypeError {
            expected: "array".to_string(),
            got: other.type_name().to_string(),
        }),
        None => Ok(Value::Null),
    }
}

fn builtin_first(args: Vec<Value>) -> Result<Value, RuntimeError> {
    match args.first() {
        Some(Value::Array(a)) => Ok(a.first().cloned().unwrap_or(Value::Null)),
        Some(other) => Err(RuntimeError::TypeError {
            expected: "array".to_string(),
            got: other.type_name().to_string(),
        }),
        None => Ok(Value::Null),
    }
}

fn builtin_last(args: Vec<Value>) -> Result<Value, RuntimeError> {
    match args.first() {
        Some(Value::Array(a)) => Ok(a.last().cloned().unwrap_or(Value::Null)),
        Some(other) => Err(RuntimeError::TypeError {
            expected: "array".to_string(),
            got: other.type_name().to_string(),
        }),
        None => Ok(Value::Null),
    }
}

fn builtin_contains(args: Vec<Value>) -> Result<Value, RuntimeError> {
    match args.first() {
        Some(Value::Array(a)) => {
            let item = args.get(1).cloned().unwrap_or(Value::Null);
            Ok(Value::Bool(a.iter().any(|v| v == &item)))
        }
        Some(Value::Str(s)) => match args.get(1) {
            Some(Value::Str(sub)) => Ok(Value::Bool(s.contains(sub.as_str()))),
            _ => Ok(Value::Bool(false)),
        },
        Some(other) => Err(RuntimeError::TypeError {
            expected: "array or string".to_string(),
            got: other.type_name().to_string(),
        }),
        None => Ok(Value::Bool(false)),
    }
}

fn builtin_to_int(args: Vec<Value>) -> Result<Value, RuntimeError> {
    match args.first() {
        Some(Value::Int(i)) => Ok(Value::Int(*i)),
        Some(Value::Float(f)) => Ok(Value::Int(*f as i64)),
        Some(Value::Bool(b)) => Ok(Value::Int(if *b { 1 } else { 0 })),
        Some(Value::Str(s)) => match s.trim().parse::<i64>() {
            Ok(i) => Ok(Value::Int(i)),
            Err(_) => Ok(Value::Null),
        },
        _ => Ok(Value::Null),
    }
}

fn builtin_to_float(args: Vec<Value>) -> Result<Value, RuntimeError> {
    match args.first() {
        Some(Value::Float(f)) => Ok(Value::Float(*f)),
        Some(Value::Int(i)) => Ok(Value::Float(*i as f64)),
        Some(Value::Str(s)) => match s.trim().parse::<f64>() {
            Ok(f) => Ok(Value::Float(f)),
            Err(_) => Ok(Value::Null),
        },
        _ => Ok(Value::Null),
    }
}

fn builtin_abs(args: Vec<Value>) -> Result<Value, RuntimeError> {
    match args.first() {
        Some(Value::Int(i)) => Ok(Value::Int(i.abs())),
        Some(Value::Float(f)) => Ok(Value::Float(f.abs())),
        Some(other) => Err(RuntimeError::TypeError {
            expected: "int or float".to_string(),
            got: other.type_name().to_string(),
        }),
        None => Err(RuntimeError::TypeError {
            expected: "int or float".to_string(),
            got: "nothing".to_string(),
        }),
    }
}

fn builtin_min(args: Vec<Value>) -> Result<Value, RuntimeError> {
    match (args.first(), args.get(1)) {
        (Some(Value::Int(a)), Some(Value::Int(b))) => Ok(Value::Int((*a).min(*b))),
        (Some(Value::Float(a)), Some(Value::Float(b))) => Ok(Value::Float(a.min(*b))),
        (Some(Value::Int(a)), Some(Value::Float(b))) => Ok(Value::Float((*a as f64).min(*b))),
        (Some(Value::Float(a)), Some(Value::Int(b))) => Ok(Value::Float(a.min(*b as f64))),
        _ => Ok(Value::Null),
    }
}

fn builtin_max(args: Vec<Value>) -> Result<Value, RuntimeError> {
    match (args.first(), args.get(1)) {
        (Some(Value::Int(a)), Some(Value::Int(b))) => Ok(Value::Int((*a).max(*b))),
        (Some(Value::Float(a)), Some(Value::Float(b))) => Ok(Value::Float(a.max(*b))),
        (Some(Value::Int(a)), Some(Value::Float(b))) => Ok(Value::Float((*a as f64).max(*b))),
        (Some(Value::Float(a)), Some(Value::Int(b))) => Ok(Value::Float(a.max(*b as f64))),
        _ => Ok(Value::Null),
    }
}

#[cfg(test)]
mod builtin_tests {
    use super::*;

    fn call(name: &str, args: Vec<Value>) -> Result<Value, RuntimeError> {
        for (n, _, f) in all_builtins() {
            if n == name {
                return f(args);
            }
        }
        panic!("no builtin '{}'", name);
    }

    #[test]
    fn test_print() {
        assert_eq!(call("print", vec![Value::Int(5)]).unwrap(), Value::Void);
    }

    #[test]
    fn test_println() {
        assert_eq!(
            call("println", vec![Value::Str("hi".to_string())]).unwrap(),
            Value::Void
        );
    }

    #[test]
    fn test_print_err() {
        assert_eq!(
            call("printErr", vec![Value::Bool(true)]).unwrap(),
            Value::Void
        );
    }

    #[test]
    fn test_len_string() {
        assert_eq!(
            call("len", vec![Value::Str("hello".to_string())]).unwrap(),
            Value::Int(5)
        );
    }

    #[test]
    fn test_len_array() {
        assert_eq!(
            call(
                "len",
                vec![Value::Array(vec![Value::Int(1), Value::Int(2)])]
            )
            .unwrap(),
            Value::Int(2)
        );
    }

    #[test]
    fn test_len_wrong_type() {
        assert!(call("len", vec![Value::Int(3)]).is_err());
    }

    #[test]
    fn test_parse_int() {
        assert_eq!(
            call("parseInt", vec![Value::Str("42".to_string())]).unwrap(),
            Value::Int(42)
        );
    }

    #[test]
    fn test_parse_int_fail() {
        assert_eq!(
            call("parseInt", vec![Value::Str("abc".to_string())]).unwrap(),
            Value::Null
        );
    }

    #[test]
    fn test_parse_float() {
        assert_eq!(
            call("parseFloat", vec![Value::Str("3.5".to_string())]).unwrap(),
            Value::Float(3.5)
        );
    }

    #[test]
    fn test_to_string() {
        assert_eq!(
            call("toString", vec![Value::Int(99)]).unwrap(),
            Value::Str("99".to_string())
        );
    }

    #[test]
    fn test_range() {
        assert_eq!(
            call("range", vec![Value::Int(1), Value::Int(4)]).unwrap(),
            Value::Array(vec![Value::Int(1), Value::Int(2), Value::Int(3)])
        );
    }

    #[test]
    fn test_range_descending() {
        assert_eq!(
            call("range", vec![Value::Int(5), Value::Int(2)]).unwrap(),
            Value::Array(vec![])
        );
    }

    #[test]
    fn test_assert_ok() {
        assert_eq!(
            call("assert", vec![Value::Bool(true)]).unwrap(),
            Value::Void
        );
    }

    #[test]
    fn test_assert_false_error() {
        assert!(call("assert", vec![Value::Bool(false)]).is_err());
    }

    #[test]
    fn test_panic_error() {
        match call("panic", vec![Value::Str("boom".to_string())]) {
            Err(RuntimeError::InvalidOperation { message, .. }) => assert_eq!(message, "boom"),
            _ => panic!("expected panic"),
        }
    }

    #[test]
    fn test_type_of() {
        assert_eq!(
            call("typeOf", vec![Value::Int(1)]).unwrap(),
            Value::Str("int".to_string())
        );
        assert_eq!(
            call("typeOf", vec![Value::Array(vec![])]).unwrap(),
            Value::Str("array".to_string())
        );
    }

    #[test]
    fn test_push() {
        assert_eq!(
            call(
                "push",
                vec![Value::Array(vec![Value::Int(1)]), Value::Int(2)]
            )
            .unwrap(),
            Value::Array(vec![Value::Int(1), Value::Int(2)])
        );
    }

    #[test]
    fn test_push_wrong_type() {
        assert!(call("push", vec![Value::Int(1), Value::Int(2)]).is_err());
    }

    #[test]
    fn test_pop() {
        assert_eq!(
            call(
                "pop",
                vec![Value::Array(vec![Value::Int(1), Value::Int(2)])]
            )
            .unwrap(),
            Value::Int(2)
        );
    }

    #[test]
    fn test_first() {
        assert_eq!(
            call(
                "first",
                vec![Value::Array(vec![Value::Int(1), Value::Int(2)])]
            )
            .unwrap(),
            Value::Int(1)
        );
    }

    #[test]
    fn test_last() {
        assert_eq!(
            call(
                "last",
                vec![Value::Array(vec![Value::Int(1), Value::Int(2)])]
            )
            .unwrap(),
            Value::Int(2)
        );
    }

    #[test]
    fn test_first_last_empty() {
        assert_eq!(
            call("first", vec![Value::Array(vec![])]).unwrap(),
            Value::Null
        );
        assert_eq!(
            call("last", vec![Value::Array(vec![])]).unwrap(),
            Value::Null
        );
    }

    #[test]
    fn test_contains() {
        assert_eq!(
            call(
                "contains",
                vec![
                    Value::Array(vec![Value::Int(1), Value::Int(2)]),
                    Value::Int(2)
                ]
            )
            .unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            call(
                "contains",
                vec![Value::Array(vec![Value::Int(1)]), Value::Int(9)]
            )
            .unwrap(),
            Value::Bool(false)
        );
    }

    #[test]
    fn test_to_int() {
        assert_eq!(
            call("toInt", vec![Value::Str("7".to_string())]).unwrap(),
            Value::Int(7)
        );
        assert_eq!(
            call("toInt", vec![Value::Float(3.9)]).unwrap(),
            Value::Int(3)
        );
    }

    #[test]
    fn test_to_float() {
        assert_eq!(
            call("toFloat", vec![Value::Str("2.5".to_string())]).unwrap(),
            Value::Float(2.5)
        );
        assert_eq!(
            call("toFloat", vec![Value::Int(4)]).unwrap(),
            Value::Float(4.0)
        );
    }

    #[test]
    fn test_abs() {
        assert_eq!(call("abs", vec![Value::Int(-5)]).unwrap(), Value::Int(5));
        assert_eq!(
            call("abs", vec![Value::Float(-2.5)]).unwrap(),
            Value::Float(2.5)
        );
    }

    #[test]
    fn test_min_max() {
        assert_eq!(
            call("min", vec![Value::Int(3), Value::Int(1)]).unwrap(),
            Value::Int(1)
        );
        assert_eq!(
            call("max", vec![Value::Int(3), Value::Int(1)]).unwrap(),
            Value::Int(3)
        );
    }

    #[test]
    fn test_all_builtins_registered() {
        let names: Vec<&str> = all_builtins().iter().map(|(n, _, _)| *n).collect();
        assert_eq!(names.len(), 21);
        for n in [
            "print",
            "println",
            "printErr",
            "len",
            "parseInt",
            "parseFloat",
            "toString",
            "range",
            "assert",
            "panic",
            "typeOf",
            "push",
            "pop",
            "first",
            "last",
            "contains",
            "toInt",
            "toFloat",
            "abs",
            "min",
            "max",
        ] {
            assert!(names.contains(&n));
        }
    }
}
