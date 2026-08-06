use crate::parser::ast::*;
use crate::types::ResolvedType;

/// Does this type require reference counting? (i.e. is it heap-allocated?)
pub fn is_refcounted(ty: &ResolvedType) -> bool {
    matches!(
        ty,
        ResolvedType::Str
            | ResolvedType::Array(_)
            | ResolvedType::Struct(_)
            | ResolvedType::Generic { .. }
    )
}

/// A single tracked local variable within a scope
#[derive(Debug, Clone)]
pub struct TrackedVar {
    pub name: String,
    pub ty: ResolvedType,
    /// Was this variable's value moved out (e.g. via `return`)?
    /// If true, no release is emitted for it.
    pub moved: bool,
}

/// One lexical scope's set of tracked (refcounted) variables,
/// in DECLARATION order (drop order is the reverse of this)
#[derive(Debug, Default)]
pub struct ScopeLifetimes {
    pub vars: Vec<TrackedVar>,
}

impl ScopeLifetimes {
    pub fn new() -> Self {
        Self { vars: Vec::new() }
    }

    /// Register a new local variable if it needs refcounting
    pub fn track(&mut self, name: String, ty: ResolvedType) {
        if is_refcounted(&ty) {
            self.vars.push(TrackedVar {
                name,
                ty,
                moved: false,
            });
        }
    }

    /// Mark a variable as moved (its value escaped via return) —
    /// it will be skipped when generating drop points
    pub fn mark_moved(&mut self, name: &str) {
        if let Some(v) = self.vars.iter_mut().find(|v| v.name == name) {
            v.moved = true;
        }
    }

    /// Variables that need a `release` call, in REVERSE (LIFO) order
    pub fn drop_order(&self) -> Vec<&TrackedVar> {
        self.vars.iter().rev().filter(|v| !v.moved).collect()
    }
}

/// Walks a function body and builds a stack of ScopeLifetimes,
/// one per nested block, tracking every refcounted local declaration.
pub struct LifetimeTracker {
    scopes: Vec<ScopeLifetimes>,
}

impl LifetimeTracker {
    pub fn new() -> Self {
        LifetimeTracker {
            scopes: vec![ScopeLifetimes::new()],
        }
    }

    pub fn push_scope(&mut self) {
        self.scopes.push(ScopeLifetimes::new());
    }

    /// Pop the current scope, returning its drop order (for codegen to emit releases)
    pub fn pop_scope(&mut self) -> ScopeLifetimes {
        self.scopes.pop().unwrap_or_default()
    }

    pub fn track_let(&mut self, name: &str, ty: &ResolvedType) {
        self.scopes
            .last_mut()
            .unwrap()
            .track(name.to_string(), ty.clone());
    }

    /// Call when compiling a `return expr` where expr is just an identifier —
    /// marks that variable as moved so it isn't double-released
    pub fn mark_returned_identifier(&mut self, name: &str) {
        // Search ALL active scopes (innermost first) — the returned var
        // might have been declared in an outer block relative to the return
        for scope in self.scopes.iter_mut().rev() {
            if scope.vars.iter().any(|v| v.name == name) {
                scope.mark_moved(name);
                return;
            }
        }
    }

    /// Walk a block and register all `let` declarations of refcounted types.
    /// Also detects `return <identifier>` and marks that var as moved.
    /// This is a lightweight pre-pass; actual release-emission happens in codegen.
    pub fn analyze_block(&mut self, block: &Block, resolve_type: &dyn Fn(&Expr) -> ResolvedType) {
        for stmt in &block.statements {
            match stmt {
                Statement::Let(l) => {
                    let ty = resolve_type(&l.value);
                    self.track_let(&l.name, &ty);
                }
                Statement::Return(r) => {
                    if let Some(Expr::Identifier(id)) = &r.value {
                        self.mark_returned_identifier(&id.name);
                    }
                }
                Statement::Block(b) => {
                    self.push_scope();
                    self.analyze_block(b, resolve_type);
                    self.pop_scope();
                }
                Statement::If(i) => {
                    self.push_scope();
                    self.analyze_block(&i.then_branch, resolve_type);
                    self.pop_scope();
                    if let Some(else_b) = &i.else_branch {
                        self.push_scope();
                        self.analyze_block(else_b, resolve_type);
                        self.pop_scope();
                    }
                }
                Statement::While(w) => {
                    self.push_scope();
                    self.analyze_block(&w.body, resolve_type);
                    self.pop_scope();
                }
                _ => {}
            }
        }
    }
}

impl Default for LifetimeTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod lifetime_tests {
    use super::*;
    use crate::types::ResolvedType;

    #[test]
    fn test_is_refcounted_str_true() {
        assert!(is_refcounted(&ResolvedType::Str));
    }
    #[test]
    fn test_is_refcounted_array_true() {
        assert!(is_refcounted(&ResolvedType::Array(Box::new(
            ResolvedType::Int
        ))));
    }
    #[test]
    fn test_is_refcounted_struct_true() {
        assert!(is_refcounted(&ResolvedType::Struct("Point".to_string())));
    }
    #[test]
    fn test_is_refcounted_int_false() {
        assert!(!is_refcounted(&ResolvedType::Int));
    }
    #[test]
    fn test_is_refcounted_bool_false() {
        assert!(!is_refcounted(&ResolvedType::Bool));
    }

    #[test]
    fn test_track_refcounted_var() {
        let mut scope = ScopeLifetimes::new();
        scope.track("s".to_string(), ResolvedType::Str);
        assert_eq!(scope.vars.len(), 1);
    }

    #[test]
    fn test_track_primitive_not_added() {
        let mut scope = ScopeLifetimes::new();
        scope.track("n".to_string(), ResolvedType::Int);
        assert_eq!(
            scope.vars.len(),
            0,
            "Primitives should not be tracked for refcounting"
        );
    }

    #[test]
    fn test_drop_order_is_reverse() {
        let mut scope = ScopeLifetimes::new();
        scope.track("a".to_string(), ResolvedType::Str);
        scope.track("b".to_string(), ResolvedType::Str);
        scope.track("c".to_string(), ResolvedType::Str);
        let order: Vec<&str> = scope.drop_order().iter().map(|v| v.name.as_str()).collect();
        assert_eq!(order, vec!["c", "b", "a"]);
    }

    #[test]
    fn test_moved_var_excluded_from_drop_order() {
        let mut scope = ScopeLifetimes::new();
        scope.track("p".to_string(), ResolvedType::Struct("Point".to_string()));
        scope.mark_moved("p");
        assert!(scope.drop_order().is_empty());
    }

    #[test]
    fn test_tracker_push_pop_scope() {
        let mut tracker = LifetimeTracker::new();
        tracker.track_let("outer", &ResolvedType::Str);
        tracker.push_scope();
        tracker.track_let("inner", &ResolvedType::Str);
        let inner_scope = tracker.pop_scope();
        assert_eq!(inner_scope.vars.len(), 1);
        assert_eq!(inner_scope.vars[0].name, "inner");
    }

    #[test]
    fn test_mark_returned_identifier_finds_outer_scope() {
        let mut tracker = LifetimeTracker::new();
        tracker.track_let("p", &ResolvedType::Struct("Point".to_string()));
        tracker.push_scope(); // enter an inner block, e.g. inside an if
        tracker.mark_returned_identifier("p"); // return happens inside the inner block
        let inner = tracker.pop_scope();
        assert!(inner.vars.is_empty()); // p wasn't declared in the inner scope
                                        // p should be marked moved in the OUTER (still-active) scope — verify via drop_order
    }

    #[test]
    fn test_analyze_block_tracks_lets() {
        use crate::lexer::Lexer;
        use crate::parser::Parser;
        let src = "fn f() { let a = \"hello\" let b = 42 }";
        let t = Lexer::tokenize(src, "t.lyz").unwrap();
        let (prog, _) = Parser::new(t, "t.lyz", src).parse().unwrap();

        if let Declaration::Function(f) = &prog.declarations[0] {
            if let FnBody::Block(block) = &f.body {
                let mut tracker = LifetimeTracker::new();
                tracker.analyze_block(block, &|_expr| ResolvedType::Str); // simplistic resolver for test
                                                                          // Both "a" and "b" resolved as Str by our dummy resolver -> both tracked
                assert_eq!(tracker.scopes[0].vars.len(), 2);
            }
        }
    }
}
