use std::collections::HashMap;

use super::value::Value;

#[derive(Debug, Clone, Default)]
pub struct Scope {
    pub bindings: HashMap<String, Value>,
}

#[derive(Debug, Clone)]
pub struct Environment {
    pub scopes: Vec<Scope>,
    pub call_depth: usize,
}

impl Default for Environment {
    fn default() -> Self {
        Self {
            scopes: vec![Scope::default()],
            call_depth: 0,
        }
    }
}
