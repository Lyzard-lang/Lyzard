use std::collections::HashMap;

use super::error::RuntimeError;
use super::value::Value;

/// One level of scope
#[derive(Debug, Clone, Default)]
pub struct Scope {
    bindings: HashMap<String, Value>,
}

impl Scope {
    pub fn new() -> Self {
        Scope {
            bindings: HashMap::new(),
        }
    }

    pub fn get(&self, name: &str) -> Option<&Value> {
        self.bindings.get(name)
    }

    pub fn set(&mut self, name: &str, value: Value) -> bool {
        if self.bindings.contains_key(name) {
            self.bindings.insert(name.to_string(), value);
            true // found and updated
        } else {
            false // not in this scope
        }
    }

    pub fn define(&mut self, name: String, value: Value) {
        self.bindings.insert(name, value);
    }
}

/// The full environment — a stack of scopes
#[derive(Debug, Clone)]
pub struct Environment {
    scopes: Vec<Scope>,
    call_depth: usize, // track recursion depth
}

const MAX_CALL_DEPTH: usize = 1000; // prevent stack overflow

impl Environment {
    pub fn new() -> Self {
        Environment {
            scopes: vec![Scope::new()], // global scope
            call_depth: 0,
        }
    }

    /// Open a new scope (called on every { block })
    pub fn push_scope(&mut self) {
        self.scopes.push(Scope::new());
    }

    /// Close the current scope (called on every } block })
    pub fn pop_scope(&mut self) {
        if self.scopes.len() > 1 {
            self.scopes.pop();
        }
    }

    /// Track entering a function call (for recursion depth)
    pub fn push_call(&mut self, fn_name: &str) -> Result<(), RuntimeError> {
        self.call_depth += 1;
        if self.call_depth > MAX_CALL_DEPTH {
            return Err(RuntimeError::StackOverflow {
                fn_name: fn_name.to_string(),
            });
        }
        self.push_scope();
        Ok(())
    }

    /// Track exiting a function call
    pub fn pop_call(&mut self) {
        if self.call_depth > 0 {
            self.call_depth -= 1;
        }
        self.pop_scope();
    }

    /// Define a NEW variable in the CURRENT scope (for let)
    pub fn define(&mut self, name: String, value: Value) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.define(name, value);
        }
    }

    /// Get a variable — searches from innermost scope outward
    pub fn get(&self, name: &str) -> Option<Value> {
        for scope in self.scopes.iter().rev() {
            if let Some(val) = scope.get(name) {
                return Some(val.clone());
            }
        }
        None
    }

    /// Set an EXISTING variable — searches from innermost scope outward
    /// Returns error if the variable doesn't exist anywhere
    pub fn set(&mut self, name: &str, value: Value) -> Result<(), RuntimeError> {
        for scope in self.scopes.iter_mut().rev() {
            if scope.set(name, value.clone()) {
                return Ok(());
            }
        }
        Err(RuntimeError::UndefinedVariable {
            name: name.to_string(),
            span: None,
        })
    }

    /// Is this name defined anywhere in the scope chain?
    pub fn is_defined(&self, name: &str) -> bool {
        self.scopes.iter().rev().any(|s| s.get(name).is_some())
    }

    /// Current call stack depth
    pub fn call_depth(&self) -> usize {
        self.call_depth
    }

    /// Number of active scopes
    pub fn scope_depth(&self) -> usize {
        self.scopes.len()
    }
}

impl Default for Environment {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod env_tests {
    use super::*;
    use crate::interpreter::value::Value;

    #[test]
    fn test_define_and_get() {
        let mut env = Environment::new();
        env.define("x".to_string(), Value::Int(42));
        assert_eq!(env.get("x"), Some(Value::Int(42)));
    }

    #[test]
    fn test_get_undefined_returns_none() {
        let env = Environment::new();
        assert_eq!(env.get("unknown"), None);
    }

    #[test]
    fn test_inner_scope_sees_outer() {
        let mut env = Environment::new();
        env.define("x".to_string(), Value::Int(1));
        env.push_scope();
        assert_eq!(env.get("x"), Some(Value::Int(1)));
    }

    #[test]
    fn test_outer_scope_does_not_see_inner() {
        let mut env = Environment::new();
        env.push_scope();
        env.define("y".to_string(), Value::Int(2));
        env.pop_scope();
        assert_eq!(env.get("y"), None);
    }

    #[test]
    fn test_set_updates_correct_scope() {
        let mut env = Environment::new();
        env.define("x".to_string(), Value::Int(1));
        env.push_scope();
        env.set("x", Value::Int(99)).unwrap();
        env.pop_scope();
        // Change should persist in outer scope
        assert_eq!(env.get("x"), Some(Value::Int(99)));
    }

    #[test]
    fn test_set_undefined_returns_error() {
        let mut env = Environment::new();
        let result = env.set("missing", Value::Int(1));
        assert!(result.is_err());
    }

    #[test]
    fn test_shadowing_in_inner_scope() {
        let mut env = Environment::new();
        env.define("x".to_string(), Value::Int(1));
        env.push_scope();
        env.define("x".to_string(), Value::Int(99)); // shadow
        assert_eq!(env.get("x"), Some(Value::Int(99))); // sees inner
        env.pop_scope();
        assert_eq!(env.get("x"), Some(Value::Int(1))); // sees outer again
    }

    #[test]
    fn test_push_call_increments_depth() {
        let mut env = Environment::new();
        assert_eq!(env.call_depth(), 0);
        env.push_call("foo").unwrap();
        assert_eq!(env.call_depth(), 1);
        env.pop_call();
        assert_eq!(env.call_depth(), 0);
    }

    #[test]
    fn test_stack_overflow_detection() {
        let mut env = Environment::new();
        let mut result = Ok(());
        for _ in 0..1001 {
            result = env.push_call("recursive_fn");
            if result.is_err() {
                break;
            }
        }
        assert!(result.is_err());
        match result.unwrap_err() {
            RuntimeError::StackOverflow { fn_name } => {
                assert_eq!(fn_name, "recursive_fn");
            }
            other => panic!("Expected StackOverflow, got {:?}", other),
        }
    }

    #[test]
    fn test_global_scope_never_popped() {
        let mut env = Environment::new();
        env.define("global".to_string(), Value::Bool(true));
        env.pop_scope(); // should NOT pop global
        assert_eq!(env.get("global"), Some(Value::Bool(true)));
        assert_eq!(env.scope_depth(), 1);
    }
}
