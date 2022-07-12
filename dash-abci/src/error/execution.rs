#[derive(Debug, thiserror::Error)]
pub enum ExecutionError {
    #[error("execution error key: {0}")]
    MissingRequiredKey(&'static str),

    #[error("overflow error: {0}")]
    Overflow(&'static str),
}
