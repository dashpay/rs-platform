#![cfg_attr(docsrs, feature(doc_cfg))]
// Coding conventions
#![forbid(unsafe_code)]
#![deny(missing_docs)]

/// ABCI module
pub mod abci;

/// Block module
mod block;

/// Common functions module
pub mod common;

/// Contracts module
pub mod contracts;

/// Errors module
pub mod error;

/// Execution module
pub mod execution;

/// Platform module
pub mod platform;
