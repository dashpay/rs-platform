use crate::drive::contract::ContractFetchInfo;
use moka::sync::Cache;
use std::sync::Arc;

/// Drive cache struct
pub struct DriveCache {
    /// Cached contracts
    pub cached_contracts: Cache<[u8; 32], Arc<ContractFetchInfo>>,
    /// Genesis time in ms
    pub genesis_time_ms: Option<u64>,
}
