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
        TypeEnvironment {
            scopes: vec![HashMap::new()],
            current_fn_return: None,
            in_loop: false,
            in_impl: false,
            self_type: None,
        }
    }

    pub fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    pub fn pop_scope(&mut self) {
        if self.scopes.len() > 1 {
            self.scopes.pop();
        }
    }

    /// Define a name → type in the current scope
    pub fn define(&mut self, name: String, ty: ResolvedType) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name, ty);
        }
    }

    /// Look up a name from innermost scope outward
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

    // ── BUILTIN TYPES ─────────────────────────────────────────

    /// Register all built-in functions with their types
    pub fn register_builtins(&mut self) {
        use ResolvedType::*;
        let builtins: Vec<(&str, ResolvedType)> = vec![
            ("print", Function { params: vec![Unknown], return_type: Box::new(Void) }),
            ("println", Function { params: vec![Unknown], return_type: Box::new(Void) }),
            ("len", Function { params: vec![Unknown], return_type: Box::new(Int) }),
            ("parseInt", Function { params: vec![Str], return_type: Box::new(Int) }),
            ("parseFloat", Function { params: vec![Str], return_type: Box::new(Float) }),
            ("toString", Function { params: vec![Unknown], return_type: Box::new(Str) }),
            ("range", Function { params: vec![Int, Int], return_type: Box::new(Array(Box::new(Int))) }),
            ("assert", Function { params: vec![Bool], return_type: Box::new(Void) }),
            ("panic", Function { params: vec![Str], return_type: Box::new(Never) }),
            ("typeOf", Function { params: vec![Unknown], return_type: Box::new(Str) }),
            ("abs", Function { params: vec![Int], return_type: Box::new(Int) }),
            ("min", Function { params: vec![Int, Int], return_type: Box::new(Int) }),
            ("max", Function { params: vec![Int, Int], return_type: Box::new(Int) }),
        ];

        for (name, ty) in builtins {
            self.define(name.to_string(), ty);
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
    fn test_define_and_lookup() {
        let mut env = TypeEnvironment::new();
        env.define("x".to_string(), ResolvedType::Int);
        assert_eq!(env.lookup("x"), Some(&ResolvedType::Int));
    }

    #[test]
    fn test_lookup_undefined_none() {
        let env = TypeEnvironment::new();
        assert_eq!(env.lookup("missing"), None);
    }

    #[test]
    fn test_inner_scope_sees_outer() {
        let mut env = TypeEnvironment::new();
        env.define("x".to_string(), ResolvedType::Int);
        env.push_scope();
        assert_eq!(env.lookup("x"), Some(&ResolvedType::Int));
    }

    #[test]
    fn test_outer_does_not_see_inner() {
        let mut env = TypeEnvironment::new();
        env.push_scope();
        env.define("y".to_string(), ResolvedType::Str);
        env.pop_scope();
        assert_eq!(env.lookup("y"), None);
    }

    #[test]
    fn test_shadowing() {
        let mut env = TypeEnvironment::new();
        env.define("x".to_string(), ResolvedType::Int);
        env.push_scope();
        env.define("x".to_string(), ResolvedType::Str);
        assert_eq!(env.lookup("x"), Some(&ResolvedType::Str));
        env.pop_scope();
        assert_eq!(env.lookup("x"), Some(&ResolvedType::Int));
    }

    #[test]
    fn test_fn_return_context() {
        let mut env = TypeEnvironment::new();
        assert!(env.expected_return_type().is_none());
        env.enter_function(ResolvedType::Int);
        assert_eq!(env.expected_return_type(), Some(&ResolvedType::Int));
        env.exit_function();
        assert!(env.expected_return_type().is_none());
    }

    #[test]
    fn test_loop_context() {
        let mut env = TypeEnvironment::new();
        assert!(!env.in_loop());
        env.enter_loop();
        assert!(env.in_loop());
        env.exit_loop();
        assert!(!env.in_loop());
    }

    #[test]
    fn test_impl_context() {
        let mut env = TypeEnvironment::new();
        assert!(env.self_type().is_none());
        env.enter_impl(ResolvedType::Struct("Point".to_string()));
        assert_eq!(env.self_type(), Some(&ResolvedType::Struct("Point".to_string())));
        env.exit_impl();
        assert!(env.self_type().is_none());
    }

    #[test]
    fn test_builtins_registered() {
        let mut env = TypeEnvironment::new();
        env.register_builtins();
        assert!(env.is_defined("print"));
        assert!(env.is_defined("len"));
        assert!(env.is_defined("range"));
        assert!(env.is_defined("panic"));
    }
}
