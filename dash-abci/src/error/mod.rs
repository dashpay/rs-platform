use crate::error::execution::ExecutionError;

mod execution;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("storage: {0}")]
    Storage(#[from] rs_drive::Error),
    #[error("execution: {0}")]
    Execution(#[from] ExecutionError),
}