use contract::ContractError;
use drive::DriveError;
use query::QueryError;
pub mod contract;
pub mod drive;
pub mod query;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Query(#[from] QueryError),
    #[error(transparent)]
    Drive(#[from] DriveError),
    #[error(transparent)]
    GroveDB(#[from] grovedb::Error),
    #[error(transparent)]
    Contract(#[from] ContractError),
}
