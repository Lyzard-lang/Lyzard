use super::error::RuntimeError;
use super::value::Value;

pub type BuiltinFn = fn(Vec<Value>) -> Result<Value, RuntimeError>;

/// Register all built-in functions into an environment
pub fn all_builtins() -> Vec<(&'static str, usize, BuiltinFn)> {
    // (name, expected_arg_count, function)
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
        ("slice", 3, builtin_slice),
        ("toInt", 1, builtin_to_int),
        ("toFloat", 1, builtin_to_float),
        ("abs", 1, builtin_abs),
        ("min", 2, builtin_min),
        ("max", 2, builtin_max),
    ]
}

// ── I/O ─────────────────────────────────────────────────────

fn builtin_print(args: Vec<Value>) -> Result<Value, RuntimeError> {
    print!("{}", args[0].to_display_string());
    Ok(Value::Void)
}

fn builtin_println(args: Vec<Value>) -> Result<Value, RuntimeError> {
    println!("{}", args[0].to_display_string());
    Ok(Value::Void)
}

fn builtin_print_err(args: Vec<Value>) -> Result<Value, RuntimeError> {
    eprintln!("{}", args[0].to_display_string());
    Ok(Value::Void)
}

// ── TYPE CONVERSION ─────────────────────────────────────────

fn builtin_to_string(args: Vec<Value>) -> Result<Value, RuntimeError> {
    Ok(Value::Str(args[0].to_display_string()))
}

fn builtin_parse_int(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let s = args[0].as_str()?;
    s.trim()
        .parse::<i64>()
        .map(Value::Int)
        .map_err(|_| RuntimeError::TypeError {
            expected: "a string containing an integer".to_string(),
            got: format!("\"{}\"", s),
        })
}

fn builtin_parse_float(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let s = args[0].as_str()?;
    s.trim()
        .parse::<f64>()
        .map(Value::Float)
        .map_err(|_| RuntimeError::TypeError {
            expected: "a string containing a number".to_string(),
            got: format!("\"{}\"", s),
        })
}

fn builtin_to_int(args: Vec<Value>) -> Result<Value, RuntimeError> {
    match &args[0] {
        Value::Int(n) => Ok(Value::Int(*n)),
        Value::Float(f) => Ok(Value::Int(*f as i64)),
        Value::Str(s) => {
            s.trim()
                .parse::<i64>()
                .map(Value::Int)
                .map_err(|_| RuntimeError::TypeError {
                    expected: "convertible to int".to_string(),
                    got: format!("\"{}\"", s),
                })
        }
        other => Err(RuntimeError::TypeError {
            expected: "int, float, or str".to_string(),
            got: other.type_name().to_string(),
        }),
    }
}

fn builtin_to_float(args: Vec<Value>) -> Result<Value, RuntimeError> {
    match &args[0] {
        Value::Float(f) => Ok(Value::Float(*f)),
        Value::Int(n) => Ok(Value::Float(*n as f64)),
        Value::Str(s) => {
            s.trim()
                .parse::<f64>()
                .map(Value::Float)
                .map_err(|_| RuntimeError::TypeError {
                    expected: "convertible to float".to_string(),
                    got: format!("\"{}\"", s),
                })
        }
        other => Err(RuntimeError::TypeError {
            expected: "float, int, or str".to_string(),
            got: other.type_name().to_string(),
        }),
    }
}

fn builtin_type_of(args: Vec<Value>) -> Result<Value, RuntimeError> {
    Ok(Value::Str(args[0].type_name().to_string()))
}

// ── INTROSPECTION ────────────────────────────────────────────

fn builtin_len(args: Vec<Value>) -> Result<Value, RuntimeError> {
    match &args[0] {
        Value::Str(s) => Ok(Value::Int(s.chars().count() as i64)),
        Value::Array(a) => Ok(Value::Int(a.len() as i64)),
        other => Err(RuntimeError::TypeError {
            expected: "str or array".to_string(),
            got: other.type_name().to_string(),
        }),
    }
}

// ── RANGES AND ITERATORS ─────────────────────────────────────

fn builtin_range(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let start = args[0].as_int()?;
    let end = args[1].as_int()?;
    let arr: Vec<Value> = (start..end).map(Value::Int).collect();
    Ok(Value::Array(arr))
}

// ── ARRAY OPERATIONS ────────────────────────────────────────

fn builtin_push(args: Vec<Value>) -> Result<Value, RuntimeError> {
    match args.into_iter().collect::<Vec<_>>().as_slice() {
        [Value::Array(arr), item] => {
            let mut new_arr = arr.clone();
            new_arr.push(item.clone());
            Ok(Value::Array(new_arr))
        }
        [other, _] => Err(RuntimeError::TypeError {
            expected: "array".to_string(),
            got: other.type_name().to_string(),
        }),
        _ => unreachable!(),
    }
}

fn builtin_pop(args: Vec<Value>) -> Result<Value, RuntimeError> {
    match &args[0] {
        Value::Array(arr) => {
            if arr.is_empty() {
                Ok(Value::Null)
            } else {
                Ok(arr.last().cloned().unwrap_or(Value::Null))
            }
        }
        other => Err(RuntimeError::TypeError {
            expected: "array".to_string(),
            got: other.type_name().to_string(),
        }),
    }
}

fn builtin_first(args: Vec<Value>) -> Result<Value, RuntimeError> {
    match &args[0] {
        Value::Array(arr) => Ok(arr.first().cloned().unwrap_or(Value::Null)),
        other => Err(RuntimeError::TypeError {
            expected: "array".to_string(),
            got: other.type_name().to_string(),
        }),
    }
}

fn builtin_last(args: Vec<Value>) -> Result<Value, RuntimeError> {
    match &args[0] {
        Value::Array(arr) => Ok(arr.last().cloned().unwrap_or(Value::Null)),
        other => Err(RuntimeError::TypeError {
            expected: "array".to_string(),
            got: other.type_name().to_string(),
        }),
    }
}

fn builtin_contains(args: Vec<Value>) -> Result<Value, RuntimeError> {
    match &args[0] {
        Value::Array(arr) => Ok(Value::Bool(arr.contains(&args[1]))),
        Value::Str(s) => {
            let needle = args[1].as_str()?;
            Ok(Value::Bool(s.contains(needle)))
        }
        other => Err(RuntimeError::TypeError {
            expected: "array or str".to_string(),
            got: other.type_name().to_string(),
        }),
    }
}

fn builtin_slice(args: Vec<Value>) -> Result<Value, RuntimeError> {
    match &args[0] {
        Value::Str(s) => {
            let start = args[1].as_int()?;
            let end = args[2].as_int()?;
            let chars: Vec<char> = s.chars().collect();
            let len = chars.len() as i64;
            let start = start.clamp(0, len);
            let end = end.clamp(0, len);
            if start >= end {
                return Ok(Value::Str(String::new()));
            }
            let sub: String = chars[start as usize..end as usize].iter().collect();
            Ok(Value::Str(sub))
        }
        other => Err(RuntimeError::TypeError {
            expected: "str".to_string(),
            got: other.type_name().to_string(),
        }),
    }
}

// ── MATH ─────────────────────────────────────────────────────

fn builtin_abs(args: Vec<Value>) -> Result<Value, RuntimeError> {
    match &args[0] {
        Value::Int(n) => Ok(Value::Int(n.abs())),
        Value::Float(f) => Ok(Value::Float(f.abs())),
        other => Err(RuntimeError::TypeError {
            expected: "int or float".to_string(),
            got: other.type_name().to_string(),
        }),
    }
}

fn builtin_min(args: Vec<Value>) -> Result<Value, RuntimeError> {
    match (&args[0], &args[1]) {
        (Value::Int(a), Value::Int(b)) => Ok(Value::Int((*a).min(*b))),
        (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a.min(*b))),
        _ => Err(RuntimeError::TypeError {
            expected: "two ints or two floats".to_string(),
            got: format!("{} and {}", args[0].type_name(), args[1].type_name()),
        }),
    }
}

fn builtin_max(args: Vec<Value>) -> Result<Value, RuntimeError> {
    match (&args[0], &args[1]) {
        (Value::Int(a), Value::Int(b)) => Ok(Value::Int((*a).max(*b))),
        (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a.max(*b))),
        _ => Err(RuntimeError::TypeError {
            expected: "two ints or two floats".to_string(),
            got: format!("{} and {}", args[0].type_name(), args[1].type_name()),
        }),
    }
}

// ── CONTROL ──────────────────────────────────────────────────

fn builtin_assert(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if !args[0].is_truthy() {
        return Err(RuntimeError::AssertionFailed {
            message: Some("assert() called with a false value".to_string()),
            span: None,
        });
    }
    Ok(Value::Void)
}

fn builtin_panic(args: Vec<Value>) -> Result<Value, RuntimeError> {
    Err(RuntimeError::Panic {
        message: args[0].to_display_string(),
        span: None,
    })
}

#[cfg(test)]
mod builtin_tests {
    use super::*;
    use crate::interpreter::value::Value;

    fn call(name: &str, args: Vec<Value>) -> Result<Value, RuntimeError> {
        let builtins = all_builtins();
        let (_, _, func) = builtins.iter().find(|(n, _, _)| *n == name).unwrap();
        func(args)
    }

    #[test]
    fn test_len_str() {
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
        assert!(call("len", vec![Value::Int(5)]).is_err());
    }

    #[test]
    fn test_parse_int_ok() {
        assert_eq!(
            call("parseInt", vec![Value::Str("42".to_string())]).unwrap(),
            Value::Int(42)
        );
    }
    #[test]
    fn test_parse_int_err() {
        assert!(call("parseInt", vec![Value::Str("abc".to_string())]).is_err());
    }

    #[test]
    #[allow(clippy::approx_constant)]
    fn test_parse_float_ok() {
        assert_eq!(
            call("parseFloat", vec![Value::Str("3.14".to_string())]).unwrap(),
            Value::Float(3.14)
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
    fn test_type_of_int() {
        assert_eq!(
            call("typeOf", vec![Value::Int(1)]).unwrap(),
            Value::Str("int".to_string())
        );
    }
    #[test]
    fn test_type_of_str() {
        assert_eq!(
            call("typeOf", vec![Value::Str("x".to_string())]).unwrap(),
            Value::Str("str".to_string())
        );
    }

    #[test]
    fn test_range() {
        let result = call("range", vec![Value::Int(0), Value::Int(3)]).unwrap();
        assert_eq!(
            result,
            Value::Array(vec![Value::Int(0), Value::Int(1), Value::Int(2)])
        );
    }

    #[test]
    fn test_abs_int() {
        assert_eq!(call("abs", vec![Value::Int(-5)]).unwrap(), Value::Int(5));
    }
    #[test]
    fn test_abs_float() {
        assert_eq!(
            call("abs", vec![Value::Float(-2.5)]).unwrap(),
            Value::Float(2.5)
        );
    }

    #[test]
    fn test_min_int() {
        assert_eq!(
            call("min", vec![Value::Int(3), Value::Int(7)]).unwrap(),
            Value::Int(3)
        );
    }
    #[test]
    fn test_max_int() {
        assert_eq!(
            call("max", vec![Value::Int(3), Value::Int(7)]).unwrap(),
            Value::Int(7)
        );
    }

    #[test]
    fn test_first_array() {
        assert_eq!(
            call(
                "first",
                vec![Value::Array(vec![Value::Int(10), Value::Int(20)])]
            )
            .unwrap(),
            Value::Int(10)
        );
    }
    #[test]
    fn test_last_array() {
        assert_eq!(
            call(
                "last",
                vec![Value::Array(vec![Value::Int(10), Value::Int(20)])]
            )
            .unwrap(),
            Value::Int(20)
        );
    }
    #[test]
    fn test_first_empty() {
        assert_eq!(
            call("first", vec![Value::Array(vec![])]).unwrap(),
            Value::Null
        );
    }

    #[test]
    fn test_contains_array() {
        let arr = Value::Array(vec![Value::Int(1), Value::Int(2), Value::Int(3)]);
        assert_eq!(
            call("contains", vec![arr.clone(), Value::Int(2)]).unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            call("contains", vec![arr, Value::Int(9)]).unwrap(),
            Value::Bool(false)
        );
    }

    #[test]
    fn test_assert_true_ok() {
        assert!(call("assert", vec![Value::Bool(true)]).is_ok());
    }
    #[test]
    fn test_assert_false_err() {
        assert!(call("assert", vec![Value::Bool(false)]).is_err());
    }

    #[test]
    fn test_panic_always_err() {
        assert!(call("panic", vec![Value::Str("oops".to_string())]).is_err());
    }

    #[test]
    fn test_to_int_from_float() {
        assert_eq!(
            call("toInt", vec![Value::Float(3.9)]).unwrap(),
            Value::Int(3)
        );
    }
    #[test]
    fn test_to_float_from_int() {
        assert_eq!(
            call("toFloat", vec![Value::Int(5)]).unwrap(),
            Value::Float(5.0)
        );
    }
}

#[cfg(test)]
mod slice_builtin_tests {
    use super::*;
    use crate::interpreter::value::Value;

    fn call(name: &str, args: Vec<Value>) -> Result<Value, RuntimeError> {
        let builtins = all_builtins();
        let (_, _, func) = builtins.iter().find(|(n, _, _)| *n == name).unwrap();
        func(args)
    }

    #[test]
    fn test_slice_basic() {
        assert_eq!(
            call("slice", vec![
                Value::Str("hello world".to_string()),
                Value::Int(0),
                Value::Int(5),
            ]).unwrap(),
            Value::Str("hello".to_string())
        );
    }

    #[test]
    fn test_slice_mid() {
        assert_eq!(
            call("slice", vec![
                Value::Str("hello world".to_string()),
                Value::Int(6),
                Value::Int(11),
            ]).unwrap(),
            Value::Str("world".to_string())
        );
    }

    #[test]
    fn test_slice_full_range() {
        assert_eq!(
            call("slice", vec![
                Value::Str("hello".to_string()),
                Value::Int(0),
                Value::Int(5),
            ]).unwrap(),
            Value::Str("hello".to_string())
        );
    }

    #[test]
    fn test_slice_out_of_bounds_clamped() {
        assert_eq!(
            call("slice", vec![
                Value::Str("hello".to_string()),
                Value::Int(-10),
                Value::Int(100),
            ]).unwrap(),
            Value::Str("hello".to_string())
        );
    }

    #[test]
    fn test_slice_reversed_range_empty() {
        assert_eq!(
            call("slice", vec![
                Value::Str("hello".to_string()),
                Value::Int(3),
                Value::Int(1),
            ]).unwrap(),
            Value::Str("".to_string())
        );
    }

    #[test]
    fn test_slice_wrong_type() {
        assert!(call("slice", vec![
            Value::Int(5),
            Value::Int(0),
            Value::Int(1),
        ]).is_err());
    }
}
