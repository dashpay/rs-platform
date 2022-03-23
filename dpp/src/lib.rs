pub mod data_contract;
extern crate core;

pub mod document;
pub mod identifier;
pub mod identity;
pub mod metadata;
pub mod util;
pub mod version;

pub mod errors;

pub mod schema;
pub mod validation;

mod dash_platform_protocol;

pub use dash_platform_protocol::DashPlatformProtocol;
pub use errors::*;
pub mod mocks;

#[cfg(test)]
mod tests;
