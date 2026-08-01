pub mod env;
pub mod error;

/// The resolved type of a LYZARD value
/// "Resolved" = fully known after type inference, no unknowns
#[derive(Debug, Clone, PartialEq)]
pub enum ResolvedType {
    // ── PRIMITIVES ────────────────────────────────────────────
    Int,
    Float,
    Bool,
    Str,
    Char,
    Void,     // returned by functions with no return value
    Never,    // function that never returns (panic, infinite loop)

    // ── OPTIONAL ─────────────────────────────────────────────
    Optional(Box<ResolvedType>),     // T?   e.g. int? = int or null

    // ── COMPOUND ─────────────────────────────────────────────
    Array(Box<ResolvedType>),        // [T]  e.g. [int], [str]
    Tuple(Vec<ResolvedType>),        // (T, U, V)

    // ── USER DEFINED ─────────────────────────────────────────
    Struct(String),                  // struct Point, struct User
    Enum(String),                    // enum Color, enum Status

    // ── GENERIC ──────────────────────────────────────────────
    Generic {
        name: String,                // "Result", "Vec", "Option"
        args: Vec<ResolvedType>,     // [int, str] for Result<int, str>
    },

    // ── FUNCTION ─────────────────────────────────────────────
    Function {
        params: Vec<ResolvedType>,
        return_type: Box<ResolvedType>,
    },

    // ── TYPE PARAMETER ───────────────────────────────────────
    TypeParam(String),               // T in fn max<T>(a: T, b: T)

    // ── SPECIAL ──────────────────────────────────────────────
    Unknown,    // type not yet inferred (inference in progress)
    Error,      // a type error occurred — avoid cascading errors
}

impl ResolvedType {
    /// Human-readable name for error messages
    pub fn name(&self) -> String {
        match self {
            Self::Int               => "int".to_string(),
            Self::Float             => "float".to_string(),
            Self::Bool              => "bool".to_string(),
            Self::Str               => "str".to_string(),
            Self::Char              => "char".to_string(),
            Self::Void              => "void".to_string(),
            Self::Never             => "never".to_string(),
            Self::Optional(inner)   => format!("{}?", inner.name()),
            Self::Array(inner)      => format!("[{}]", inner.name()),
            Self::Tuple(types)      => format!("({})", types.iter().map(|t| t.name()).collect::<Vec<_>>().join(", ")),
            Self::Struct(name)      => name.clone(),
            Self::Enum(name)        => name.clone(),
            Self::Generic { name, args } => {
                if args.is_empty() { name.clone() }
                else { format!("{}< {} >", name, args.iter().map(|a| a.name()).collect::<Vec<_>>().join(", ")) }
            }
            Self::Function { params, return_type } => {
                format!("fn({}) -> {}", params.iter().map(|p| p.name()).collect::<Vec<_>>().join(", "), return_type.name())
            }
            Self::TypeParam(name)   => name.clone(),
            Self::Unknown           => "unknown".to_string(),
            Self::Error             => "<type error>".to_string(),
        }
    }

    /// Is this type numeric? (int or float)
    pub fn is_numeric(&self) -> bool {
        matches!(self, Self::Int | Self::Float)
    }

    /// Is this type an error sentinel? (avoid cascading errors)
    pub fn is_error(&self) -> bool {
        matches!(self, Self::Error | Self::Unknown)
    }

    /// Can this type be null? (only Optional types can)
    pub fn is_nullable(&self) -> bool {
        matches!(self, Self::Optional(_))
    }

    /// Is this an int?
    pub fn is_int(&self) -> bool { matches!(self, Self::Int) }

    /// Is this a bool?
    pub fn is_bool(&self) -> bool { matches!(self, Self::Bool) }

    /// Is this a str?
    pub fn is_str(&self) -> bool { matches!(self, Self::Str) }

    /// Can the value of type `other` be used where `self` is expected?
    /// Handles int→float coercion and Error passthrough
    pub fn is_assignable_from(&self, other: &ResolvedType) -> bool {
        if other.is_error() { return true; } // don't cascade errors
        if self == other { return true; }

        // int is assignable to float (implicit coercion)
        if matches!(self, Self::Float) && matches!(other, Self::Int) {
            return true;
        }

        // T is assignable to T?
        if let Self::Optional(inner) = self {
            if inner.as_ref() == other { return true; }
        }

        // null is assignable to T? (Optional)
        if matches!(self, Self::Optional(_)) && matches!(other, Self::Void) {
            return true;
        }

        false
    }

    /// The result type of a binary arithmetic op between two types
    pub fn arithmetic_result(left: &Self, right: &Self) -> Option<Self> {
        match (left, right) {
            (Self::Int,   Self::Int)   => Some(Self::Int),
            (Self::Float, Self::Float) => Some(Self::Float),
            (Self::Int,   Self::Float) => Some(Self::Float), // int + float = float
            (Self::Float, Self::Int)   => Some(Self::Float),
            (Self::Str,   Self::Str)   => Some(Self::Str),   // str + str = str (concat)
            _ => None,
        }
    }

    /// Inner type of an Optional — None if not Optional
    pub fn unwrap_optional(&self) -> Option<&ResolvedType> {
        match self {
            Self::Optional(inner) => Some(inner.as_ref()),
            _ => None,
        }
    }

    /// For Result<T, E> — returns Some((T, E)) if this is a Result
    pub fn as_result(&self) -> Option<(&ResolvedType, &ResolvedType)> {
        if let Self::Generic { name, args } = self {
            if name == "Result" && args.len() == 2 {
                return Some((&args[0], &args[1]));
            }
        }
        None
    }
}

impl std::fmt::Display for ResolvedType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

#[cfg(test)]
mod resolved_type_tests {
    use super::*;

    #[test]
    fn test_name_primitives() {
        assert_eq!(ResolvedType::Int.name(),   "int");
        assert_eq!(ResolvedType::Float.name(), "float");
        assert_eq!(ResolvedType::Bool.name(),  "bool");
        assert_eq!(ResolvedType::Str.name(),   "str");
        assert_eq!(ResolvedType::Void.name(),  "void");
    }

    #[test]
    fn test_name_optional() {
        let t = ResolvedType::Optional(Box::new(ResolvedType::Int));
        assert_eq!(t.name(), "int?");
    }

    #[test]
    fn test_name_array() {
        let t = ResolvedType::Array(Box::new(ResolvedType::Str));
        assert_eq!(t.name(), "[str]");
    }

    #[test]
    fn test_name_generic_result() {
        let t = ResolvedType::Generic {
            name: "Result".to_string(),
            args: vec![ResolvedType::Int, ResolvedType::Str],
        };
        assert!(t.name().contains("Result"));
        assert!(t.name().contains("int"));
        assert!(t.name().contains("str"));
    }

    #[test]
    fn test_is_numeric() {
        assert!(ResolvedType::Int.is_numeric());
        assert!(ResolvedType::Float.is_numeric());
        assert!(!ResolvedType::Str.is_numeric());
        assert!(!ResolvedType::Bool.is_numeric());
    }

    #[test]
    fn test_is_assignable_same_type() {
        assert!(ResolvedType::Int.is_assignable_from(&ResolvedType::Int));
        assert!(ResolvedType::Str.is_assignable_from(&ResolvedType::Str));
    }

    #[test]
    fn test_int_assignable_to_float() {
        assert!(ResolvedType::Float.is_assignable_from(&ResolvedType::Int));
        assert!(!ResolvedType::Int.is_assignable_from(&ResolvedType::Float));
    }

    #[test]
    fn test_int_assignable_to_optional_int() {
        let opt = ResolvedType::Optional(Box::new(ResolvedType::Int));
        assert!(opt.is_assignable_from(&ResolvedType::Int));
    }

    #[test]
    fn test_error_always_assignable() {
        // Error type never cascades
        assert!(ResolvedType::Int.is_assignable_from(&ResolvedType::Error));
        assert!(ResolvedType::Str.is_assignable_from(&ResolvedType::Error));
    }

    #[test]
    fn test_arithmetic_result_int_int() {
        let r = ResolvedType::arithmetic_result(&ResolvedType::Int, &ResolvedType::Int);
        assert_eq!(r, Some(ResolvedType::Int));
    }

    #[test]
    fn test_arithmetic_result_int_float() {
        let r = ResolvedType::arithmetic_result(&ResolvedType::Int, &ResolvedType::Float);
        assert_eq!(r, Some(ResolvedType::Float));
    }

    #[test]
    fn test_arithmetic_result_str_str() {
        let r = ResolvedType::arithmetic_result(&ResolvedType::Str, &ResolvedType::Str);
        assert_eq!(r, Some(ResolvedType::Str));
    }

    #[test]
    fn test_arithmetic_result_invalid() {
        let r = ResolvedType::arithmetic_result(&ResolvedType::Bool, &ResolvedType::Int);
        assert_eq!(r, None);
    }

    #[test]
    fn test_as_result() {
        let t = ResolvedType::Generic {
            name: "Result".to_string(),
            args: vec![ResolvedType::Int, ResolvedType::Str],
        };
        let (ok, err) = t.as_result().unwrap();
        assert_eq!(ok, &ResolvedType::Int);
        assert_eq!(err, &ResolvedType::Str);
    }

    #[test]
    fn test_display() {
        assert_eq!(format!("{}", ResolvedType::Int), "int");
        let opt = ResolvedType::Optional(Box::new(ResolvedType::Bool));
        assert_eq!(format!("{}", opt), "bool?");
    }
}
