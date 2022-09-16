#![cfg_attr(docsrs, feature(doc_cfg))]
// Coding conventions
#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod abci;
mod block;
pub mod common;
pub mod contracts;
pub mod error;
pub mod execution;
pub mod platform;
