use crate::error::execution::ExecutionError;
use crate::error::serialization::SerializationError;
use rs_drive::error::Error as DriveError;

/// Execution errors module
pub mod execution;

/// Serialization errors module
pub mod serialization;

/// Errors
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("storage: {0}")]
    Drive(#[from] DriveError),
    #[error("execution: {0}")]
    Execution(#[from] ExecutionError),
    #[error("serialization: {0}")]
    Serialization(#[from] SerializationError),
}
