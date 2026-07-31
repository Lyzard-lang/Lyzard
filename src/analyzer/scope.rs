use super::symbol::Symbol;
use std::collections::HashMap;

/// A single lexical scope (one level of nesting)
#[derive(Debug, Clone)]
pub struct Scope {
    pub bindings: HashMap<String, Symbol>,
    pub kind: ScopeKind,
}

/// What kind of scope this is (for context-checking)
#[derive(Debug, Clone, PartialEq)]
pub enum ScopeKind {
    Global,   // top-level
    Function, // inside fn body
    Block,    // plain { } block
    Loop,     // while/for/loop body
    Match,    // match arm
    Impl,     // impl block
}

impl Scope {
    pub fn new(kind: ScopeKind) -> Self {
        Scope {
            bindings: HashMap::new(),
            kind,
        }
    }

    pub fn define(&mut self, name: String, symbol: Symbol) {
        self.bindings.insert(name, symbol);
    }

    pub fn get(&self, name: &str) -> Option<&Symbol> {
        self.bindings.get(name)
    }

    pub fn contains(&self, name: &str) -> bool {
        self.bindings.contains_key(name)
    }
}

/// The symbol table — a stack of scopes
pub struct SymbolTable {
    scopes: Vec<Scope>,
}

impl SymbolTable {
    /// Create a new symbol table with an empty global scope
    pub fn new() -> Self {
        SymbolTable {
            scopes: vec![Scope::new(ScopeKind::Global)],
        }
    }

    /// Push a new scope onto the stack
    pub fn push(&mut self, kind: ScopeKind) {
        self.scopes.push(Scope::new(kind));
    }

    /// Pop the innermost scope (returns it for inspection in tests)
    pub fn pop(&mut self) -> Option<Scope> {
        if self.scopes.len() > 1 {
            self.scopes.pop()
        } else {
            None // never pop the global scope
        }
    }

    /// Define a symbol in the CURRENT (innermost) scope
    pub fn define(&mut self, name: String, symbol: Symbol) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.define(name, symbol);
        }
    }

    /// Look up a name — searches from innermost scope outward
    pub fn lookup(&self, name: &str) -> Option<&Symbol> {
        for scope in self.scopes.iter().rev() {
            if let Some(sym) = scope.get(name) {
                return Some(sym);
            }
        }
        None
    }

    /// Is this name defined in the CURRENT scope only? (for duplicate detection)
    pub fn defined_in_current_scope(&self, name: &str) -> bool {
        self.scopes
            .last()
            .map(|s| s.contains(name))
            .unwrap_or(false)
    }

    /// Is this name defined ANYWHERE in the scope chain?
    pub fn is_defined(&self, name: &str) -> bool {
        self.lookup(name).is_some()
    }

    /// Current scope depth (1 = global, 2 = one level in, ...)
    pub fn depth(&self) -> usize {
        self.scopes.len()
    }

    /// Are we currently inside a scope of a given kind?
    /// Searches up the scope chain.
    pub fn inside_scope(&self, kind: ScopeKind) -> bool {
        self.scopes.iter().rev().any(|s| s.kind == kind)
    }

    /// Are we inside a loop? (while, for, loop)
    pub fn inside_loop(&self) -> bool {
        self.inside_scope(ScopeKind::Loop)
    }

    /// Are we inside a function?
    pub fn inside_function(&self) -> bool {
        self.inside_scope(ScopeKind::Function)
    }

    /// Get all names visible from the current scope (for autocomplete/LSP later)
    pub fn all_visible_names(&self) -> Vec<String> {
        let mut names = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for scope in self.scopes.iter().rev() {
            for name in scope.bindings.keys() {
                if seen.insert(name.clone()) {
                    names.push(name.clone());
                }
            }
        }
        names
    }
}

impl Default for SymbolTable {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod scope_tests {
    use super::super::symbol::*;
    use super::*;
    use crate::lexer::Span;

    fn s() -> Span {
        Span::dummy()
    }

    fn var(name: &str) -> Symbol {
        Symbol::Variable(VariableSymbol {
            name: name.to_string(),
            mutable: false,
            type_annotation: None,
            defined_at: s(),
            is_param: false,
        })
    }

    #[test]
    fn test_define_and_lookup() {
        let mut table = SymbolTable::new();
        table.define("x".to_string(), var("x"));
        assert!(table.is_defined("x"));
        assert!(!table.is_defined("y"));
    }

    #[test]
    fn test_inner_scope_sees_outer() {
        let mut table = SymbolTable::new();
        table.define("x".to_string(), var("x"));
        table.push(ScopeKind::Block);
        // x defined in outer scope is visible in inner
        assert!(table.is_defined("x"));
    }

    #[test]
    fn test_outer_scope_does_not_see_inner() {
        let mut table = SymbolTable::new();
        table.push(ScopeKind::Block);
        table.define("y".to_string(), var("y"));
        table.pop();
        // y was in inner scope, now popped
        assert!(!table.is_defined("y"));
    }

    #[test]
    fn test_shadowing_inner_overrides_outer() {
        let mut table = SymbolTable::new();
        table.define("x".to_string(), var("x_outer"));
        table.push(ScopeKind::Block);
        table.define("x".to_string(), var("x_inner"));
        // Inner x shadows outer x — lookup returns inner
        let sym = table.lookup("x").unwrap();
        assert_eq!(sym.kind_name(), "variable");
    }

    #[test]
    fn test_defined_in_current_scope() {
        let mut table = SymbolTable::new();
        table.define("x".to_string(), var("x"));
        table.push(ScopeKind::Block);
        // x is visible but NOT in current scope
        assert!(!table.defined_in_current_scope("x"));
        table.define("y".to_string(), var("y"));
        // y IS in current scope
        assert!(table.defined_in_current_scope("y"));
    }

    #[test]
    fn test_depth() {
        let mut table = SymbolTable::new();
        assert_eq!(table.depth(), 1);
        table.push(ScopeKind::Function);
        assert_eq!(table.depth(), 2);
        table.push(ScopeKind::Block);
        assert_eq!(table.depth(), 3);
        table.pop();
        assert_eq!(table.depth(), 2);
    }

    #[test]
    fn test_inside_loop() {
        let mut table = SymbolTable::new();
        assert!(!table.inside_loop());
        table.push(ScopeKind::Function);
        assert!(!table.inside_loop());
        table.push(ScopeKind::Loop);
        assert!(table.inside_loop());
        table.pop();
        assert!(!table.inside_loop());
    }

    #[test]
    fn test_inside_function() {
        let mut table = SymbolTable::new();
        assert!(!table.inside_function());
        table.push(ScopeKind::Function);
        assert!(table.inside_function());
        table.push(ScopeKind::Block);
        // Still inside function even with nested blocks
        assert!(table.inside_function());
    }

    #[test]
    fn test_global_scope_never_popped() {
        let mut table = SymbolTable::new();
        table.define("global".to_string(), var("global"));
        let result = table.pop(); // trying to pop global
        assert!(result.is_none());
        // Global scope still intact
        assert!(table.is_defined("global"));
    }

    #[test]
    fn test_all_visible_names() {
        let mut table = SymbolTable::new();
        table.define("x".to_string(), var("x"));
        table.define("y".to_string(), var("y"));
        table.push(ScopeKind::Block);
        table.define("z".to_string(), var("z"));
        let names = table.all_visible_names();
        assert!(names.contains(&"x".to_string()));
        assert!(names.contains(&"y".to_string()));
        assert!(names.contains(&"z".to_string()));
    }
}
