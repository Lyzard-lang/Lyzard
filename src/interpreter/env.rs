use std::collections::HashMap;

use super::error::RuntimeError;
use super::value::Value;

/// Maximum nested function-call depth before we declare a stack overflow.
pub const MAX_CALL_DEPTH: usize = 1000;

/// One lexical scope: a set of name → value bindings.
#[derive(Debug, Clone, Default)]
pub struct Scope {
    bindings: HashMap<String, Value>,
}

impl Scope {
    pub fn new() -> Self {
        Self {
            bindings: HashMap::new(),
        }
    }

    /// Insert a binding in this scope, returning any previous value.
    pub fn define(&mut self, name: &str, value: Value) -> Option<Value> {
        self.bindings.insert(name.to_string(), value)
    }

    pub fn lookup(&self, name: &str) -> Option<&Value> {
        self.bindings.get(name)
    }

    /// Update an existing binding; returns `false` if the name is unknown.
    pub fn update(&mut self, name: &str, value: Value) -> bool {
        if self.bindings.contains_key(name) {
            self.bindings.insert(name.to_string(), value);
            true
        } else {
            false
        }
    }

    pub fn contains(&self, name: &str) -> bool {
        self.bindings.contains_key(name)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &Value)> {
        self.bindings.iter()
    }
}

/// A stack of scopes plus call-depth tracking for recursion limits.
#[derive(Debug, Clone)]
pub struct Environment {
    scopes: Vec<Scope>,
    call_depth: usize,
}

impl Default for Environment {
    fn default() -> Self {
        Self {
            scopes: vec![Scope::new()],
            call_depth: 0,
        }
    }
}

impl Environment {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push_scope(&mut self) {
        self.scopes.push(Scope::new());
    }

    /// Pop the innermost scope, but never the global scope.
    pub fn pop_scope(&mut self) -> Option<Scope> {
        if self.scopes.len() > 1 {
            self.scopes.pop()
        } else {
            None
        }
    }

    pub fn scope_count(&self) -> usize {
        self.scopes.len()
    }

    /// Define a name in the current (innermost) scope.
    pub fn define(&mut self, name: &str, value: Value) -> Option<Value> {
        self.scopes.last_mut()?.define(name, value)
    }

    /// Update an existing binding, searching outer scopes outward.
    pub fn set(&mut self, name: &str, value: Value) -> bool {
        for scope in self.scopes.iter_mut().rev() {
            if scope.contains(name) {
                return scope.update(name, value);
            }
        }
        false
    }

    /// Look up a name from innermost to outermost scope.
    pub fn lookup(&self, name: &str) -> Option<&Value> {
        self.scopes.iter().rev().find_map(|s| s.lookup(name))
    }

    pub fn is_defined(&self, name: &str) -> bool {
        self.lookup(name).is_some()
    }

    /// Names currently bound in the innermost scope only.
    pub fn current_bindings(&self) -> Vec<(String, Value)> {
        self.scopes
            .last()
            .map(|s| s.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default()
    }

    /// Enter a function call: bump depth and open a fresh scope.
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

    /// Leave a function call: decrement depth and drop the call scope.
    pub fn pop_call(&mut self) {
        self.call_depth = self.call_depth.saturating_sub(1);
        self.pop_scope();
    }

    pub fn call_depth(&self) -> usize {
        self.call_depth
    }
}

#[cfg(test)]
mod env_tests {
    use super::*;

    fn env_with(a: &str, v: Value) -> Environment {
        let mut e = Environment::new();
        e.define(a, v);
        e
    }

    #[test]
    fn test_define_lookup_roundtrip() {
        let mut e = Environment::new();
        assert!(e.define("x", Value::Int(10)).is_none());
        assert_eq!(e.lookup("x"), Some(&Value::Int(10)));
    }

    #[test]
    fn test_define_overwrites_in_same_scope() {
        let mut e = Environment::new();
        e.define("x", Value::Int(1));
        let old = e.define("x", Value::Int(2));
        assert_eq!(old, Some(Value::Int(1)));
        assert_eq!(e.lookup("x"), Some(&Value::Int(2)));
    }

    #[test]
    fn test_lookup_in_outer_scope() {
        let mut e = Environment::new();
        e.define("x", Value::Int(1));
        e.push_scope();
        assert_eq!(e.lookup("x"), Some(&Value::Int(1)));
    }

    #[test]
    fn test_shadowing() {
        let mut e = Environment::new();
        e.define("x", Value::Int(1));
        e.push_scope();
        e.define("x", Value::Int(2));
        assert_eq!(e.lookup("x"), Some(&Value::Int(2)));
        e.pop_scope();
        assert_eq!(e.lookup("x"), Some(&Value::Int(1)));
    }

    #[test]
    fn test_set_updates_existing_binding() {
        let mut e = Environment::new();
        e.define("x", Value::Int(1));
        e.push_scope();
        assert!(e.set("x", Value::Int(99)));
        assert_eq!(e.lookup("x"), Some(&Value::Int(99)));
    }

    #[test]
    fn test_set_fails_for_missing_name() {
        let mut e = Environment::new();
        assert!(!e.set("missing", Value::Int(1)));
    }

    #[test]
    fn test_pop_scope_removes_bindings() {
        let mut e = Environment::new();
        e.define("x", Value::Int(1));
        e.push_scope();
        e.define("y", Value::Int(2));
        assert_eq!(e.scope_count(), 2);
        let popped = e.pop_scope().unwrap();
        assert!(popped.contains("y"));
        assert_eq!(e.scope_count(), 1);
        assert!(e.lookup("y").is_none());
        assert_eq!(e.lookup("x"), Some(&Value::Int(1)));
    }

    #[test]
    fn test_global_scope_cannot_be_popped() {
        let mut e = Environment::new();
        assert!(e.pop_scope().is_none());
        assert_eq!(e.scope_count(), 1);
    }

    #[test]
    fn test_undefined_lookup() {
        let e = env_with("x", Value::Int(1));
        assert!(e.lookup("nope").is_none());
        assert!(!e.is_defined("nope"));
        assert!(e.is_defined("x"));
    }

    #[test]
    fn test_push_pop_call_depth() {
        let mut e = Environment::new();
        assert_eq!(e.call_depth(), 0);
        e.push_call("main").unwrap();
        e.push_call("inner").unwrap();
        assert_eq!(e.call_depth(), 2);
        assert_eq!(e.scope_count(), 3);
        e.pop_call();
        e.pop_call();
        assert_eq!(e.call_depth(), 0);
        assert_eq!(e.scope_count(), 1);
    }

    #[test]
    fn test_stack_overflow_detection() {
        let mut e = Environment::new();
        let result = (0..=MAX_CALL_DEPTH).try_for_each(|_| e.push_call("recurse"));
        match result {
            Err(RuntimeError::StackOverflow { fn_name }) => assert_eq!(fn_name, "recurse"),
            _ => panic!("expected stack overflow"),
        }
    }

    #[test]
    fn test_current_bindings() {
        let mut e = Environment::new();
        e.define("x", Value::Int(1));
        let mut names: Vec<String> = e
            .current_bindings()
            .iter()
            .map(|(k, _)| k.clone())
            .collect();
        names.sort();
        assert_eq!(names, vec!["x".to_string()]);
    }
}
