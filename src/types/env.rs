use std::collections::HashMap;

use super::ResolvedType;

/// Maps names to their resolved types in nested scopes
#[derive(Debug, Clone)]
pub struct TypeEnvironment {
    scopes: Vec<HashMap<String, ResolvedType>>,
    /// The return type of the function we are currently inside
    current_fn_return: Option<ResolvedType>,
    /// Are we currently inside a loop?
    in_loop: bool,
    /// Are we currently inside an impl block?
    in_impl: bool,
    /// The type of `self` in the current impl block
    self_type: Option<ResolvedType>,
}

impl TypeEnvironment {
    pub fn new() -> Self {
        let mut env = TypeEnvironment {
            scopes: vec![HashMap::new()],
            current_fn_return: None,
            in_loop: false,
            in_impl: false,
            self_type: None,
        };
        env.register_builtins();
        env
    }

    pub fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    pub fn pop_scope(&mut self) {
        if self.scopes.len() > 1 {
            self.scopes.pop();
        }
    }

    pub fn define(&mut self, name: String, ty: ResolvedType) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name, ty);
        }
    }

    pub fn lookup(&self, name: &str) -> Option<&ResolvedType> {
        for scope in self.scopes.iter().rev() {
            if let Some(ty) = scope.get(name) {
                return Some(ty);
            }
        }
        None
    }

    pub fn is_defined(&self, name: &str) -> bool {
        self.lookup(name).is_some()
    }

    // ── FUNCTION CONTEXT ─────────────────────────────────────

    pub fn enter_function(&mut self, return_type: ResolvedType) {
        self.current_fn_return = Some(return_type);
    }
    pub fn exit_function(&mut self) {
        self.current_fn_return = None;
    }
    pub fn expected_return_type(&self) -> Option<&ResolvedType> {
        self.current_fn_return.as_ref()
    }

    // ── LOOP CONTEXT ─────────────────────────────────────────

    pub fn enter_loop(&mut self) {
        self.in_loop = true;
    }
    pub fn exit_loop(&mut self) {
        self.in_loop = false;
    }
    pub fn in_loop(&self) -> bool {
        self.in_loop
    }

    // ── IMPL CONTEXT ─────────────────────────────────────────

    pub fn enter_impl(&mut self, self_ty: ResolvedType) {
        self.in_impl = true;
        self.self_type = Some(self_ty);
    }
    pub fn exit_impl(&mut self) {
        self.in_impl = false;
        self.self_type = None;
    }
    pub fn self_type(&self) -> Option<&ResolvedType> {
        self.self_type.as_ref()
    }

    // ── BUILTIN TYPES ────────────────────────────────────────

    fn register_builtins(&mut self) {
        use ResolvedType::*;
        let builtins: &[(&str, ResolvedType)] = &[
            (
                "print",
                Function {
                    params: vec![Unknown],
                    return_type: Box::new(Void),
                },
            ),
            (
                "println",
                Function {
                    params: vec![Unknown],
                    return_type: Box::new(Void),
                },
            ),
            (
                "len",
                Function {
                    params: vec![Unknown],
                    return_type: Box::new(Int),
                },
            ),
            (
                "parseInt",
                Function {
                    params: vec![Str],
                    return_type: Box::new(Int),
                },
            ),
            (
                "parseFloat",
                Function {
                    params: vec![Str],
                    return_type: Box::new(Float),
                },
            ),
            (
                "toString",
                Function {
                    params: vec![Unknown],
                    return_type: Box::new(Str),
                },
            ),
            (
                "range",
                Function {
                    params: vec![Int, Int],
                    return_type: Box::new(Array(Box::new(Int))),
                },
            ),
            (
                "assert",
                Function {
                    params: vec![Bool],
                    return_type: Box::new(Void),
                },
            ),
            (
                "panic",
                Function {
                    params: vec![Str],
                    return_type: Box::new(Never),
                },
            ),
            (
                "typeOf",
                Function {
                    params: vec![Unknown],
                    return_type: Box::new(Str),
                },
            ),
            (
                "abs",
                Function {
                    params: vec![Unknown],
                    return_type: Box::new(Unknown),
                },
            ),
            (
                "min",
                Function {
                    params: vec![Unknown, Unknown],
                    return_type: Box::new(Unknown),
                },
            ),
            (
                "max",
                Function {
                    params: vec![Unknown, Unknown],
                    return_type: Box::new(Unknown),
                },
            ),
        ];
        for (name, ty) in builtins {
            self.define(name.to_string(), ty.clone());
        }
    }
}

impl Default for TypeEnvironment {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod type_env_tests {
    use super::*;
    use crate::types::ResolvedType;

    #[test]
    fn test_define_lookup() {
        let mut e = TypeEnvironment::new();
        e.define("x".to_string(), ResolvedType::Int);
        assert_eq!(e.lookup("x"), Some(&ResolvedType::Int));
    }

    #[test]
    fn test_undefined_none() {
        assert_eq!(TypeEnvironment::new().lookup("nope"), None);
    }

    #[test]
    fn test_inner_sees_outer() {
        let mut e = TypeEnvironment::new();
        e.define("x".to_string(), ResolvedType::Int);
        e.push_scope();
        assert_eq!(e.lookup("x"), Some(&ResolvedType::Int));
    }

    #[test]
    fn test_outer_not_see_inner() {
        let mut e = TypeEnvironment::new();
        e.push_scope();
        e.define("y".to_string(), ResolvedType::Str);
        e.pop_scope();
        assert_eq!(e.lookup("y"), None);
    }

    #[test]
    fn test_shadowing() {
        let mut e = TypeEnvironment::new();
        e.define("x".to_string(), ResolvedType::Int);
        e.push_scope();
        e.define("x".to_string(), ResolvedType::Str);
        assert_eq!(e.lookup("x"), Some(&ResolvedType::Str));
        e.pop_scope();
        assert_eq!(e.lookup("x"), Some(&ResolvedType::Int));
    }

    #[test]
    fn test_fn_return_context() {
        let mut e = TypeEnvironment::new();
        e.enter_function(ResolvedType::Int);
        assert_eq!(e.expected_return_type(), Some(&ResolvedType::Int));
        e.exit_function();
        assert!(e.expected_return_type().is_none());
    }

    #[test]
    fn test_loop_context() {
        let mut e = TypeEnvironment::new();
        assert!(!e.in_loop());
        e.enter_loop();
        assert!(e.in_loop());
        e.exit_loop();
        assert!(!e.in_loop());
    }

    #[test]
    fn test_impl_context() {
        let mut e = TypeEnvironment::new();
        e.enter_impl(ResolvedType::Struct("P".to_string()));
        assert!(e.self_type().is_some());
        e.exit_impl();
        assert!(e.self_type().is_none());
    }

    #[test]
    fn test_builtins_registered() {
        let e = TypeEnvironment::new();
        assert!(e.is_defined("print"));
        assert!(e.is_defined("len"));
        assert!(e.is_defined("panic"));
    }
}
