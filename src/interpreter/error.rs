#[derive(Debug, Clone, PartialEq)]
pub enum RuntimeError {
    TypeError { expected: String, got: String },
}
