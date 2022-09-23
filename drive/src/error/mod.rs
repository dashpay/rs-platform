use document::DocumentError;
use dpp::data_contract::extra::ContractError;
use drive::DriveError;
use fee::FeeError;
use identity::IdentityError;
use query::QueryError;
use structure::StructureError;

/// Document module
pub mod document;
/// Drive module
pub mod drive;
/// Fee module
pub mod fee;
/// Identity module
pub mod identity;
/// Query module
pub mod query;
/// Structure module
pub mod structure;

/// Errors
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("query: {0}")]
    Query(#[from] QueryError),
    #[error("drive: {0}")]
    Drive(#[from] DriveError),
    #[error("grovedb: {0}")]
    GroveDB(#[from] grovedb::Error),
    #[error("contract: {0}")]
    Contract(#[from] ContractError),
    #[error("identity: {0}")]
    Identity(#[from] IdentityError),
    #[error("structure: {0}")]
    Structure(#[from] StructureError),
    #[error("fee: {0}")]
    Fee(#[from] FeeError),
    #[error("document: {0}")]
    Document(#[from] DocumentError),
}
