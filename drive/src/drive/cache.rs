use crate::contract::Contract;
use crate::drive::flags::StorageFlags;
use crate::fee::FeeResult;
use costs::OperationCost;
use moka::sync::Cache;
use std::sync::Arc;

/// Drive cache struct
pub struct DriveCache {
    /// Cached contracts
    pub cached_contracts: Cache<[u8; 32], Arc<ContractFetchInfo>>,
    /// Genesis time in ms
    pub genesis_time_ms: Option<u64>,
}

/// Contract and fetch information
pub struct ContractFetchInfo {
    /// The contract
    pub contract: Contract,
    /// The contract's potential storage flags
    pub storage_flags: Option<StorageFlags>,
    /// These are the operations that are used to fetch a contract
    /// This is only used on epoch change
    pub(crate) cost: OperationCost,
    /// The fee is updated every epoch based on operation costs
    /// Except if protocol version has changed in which case all the cache is cleared
    pub fee: Option<FeeResult>,
}
