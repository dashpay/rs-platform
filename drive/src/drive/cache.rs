use crate::drive::contract::ContractFetchInfo;
use crate::drive::TransactionPointerAddress;
use grovedb::{Transaction, TransactionArg};
use moka::sync::Cache;
use std::collections::HashMap;
use std::sync::Arc;

/// Drive cache struct
pub struct DriveCache {
    /// Cached contracts
    pub cached_contracts: DataContractCache,
    /// Genesis time in ms
    pub genesis_time_ms: Option<u64>,
}

/// Data Contract cache that handle both non transactional and transactional data
pub struct DataContractCache {
    global_cache: Cache<[u8; 32], Arc<ContractFetchInfo>>,
    transactional_cache: DataContractTransactionalCache,
}

impl DataContractCache {
    /// Create a new Data Contract cache instance
    pub fn new(global_cache_max_capacity: u64, transactional_cache_max_capacity: u64) -> Self {
        Self {
            global_cache: Cache::new(global_cache_max_capacity),
            transactional_cache: DataContractTransactionalCache::new(
                transactional_cache_max_capacity,
            ),
        }
    }

    /// Inserts Data Contract to transactional cache if present
    /// otherwise to goes to global cache
    pub fn insert(&mut self, fetch_info: Arc<ContractFetchInfo>, transaction: TransactionArg) {
        if let Some(tx) = transaction {
            self.transactional_cache.insert(tx, fetch_info);
        } else {
            self.global_cache
                .insert(fetch_info.contract.id().to_buffer(), fetch_info);
        }
    }

    /// Tries to get a data contract from transaction cache if present
    /// if transactional cache doesn't have the contract or transaction is not present
    /// then it tries get the contract from global cache
    pub fn get(
        &self,
        contract_id: [u8; 32],
        transaction: TransactionArg,
    ) -> Option<Arc<ContractFetchInfo>> {
        transaction
            .and_then(|tx| self.transactional_cache.get(tx, contract_id))
            .or_else(|| self.global_cache.get(&contract_id))
    }

    /// Merge transactional cache to global cache if present
    pub fn merge_transactional_cache(&self, transaction: &Transaction) {
        if let Some(cache) = self.transactional_cache.get_cache(transaction) {
            for (contract_id, fetch_info) in cache {
                self.global_cache.insert(*contract_id, fetch_info);
            }
        }
    }

    /// Clear cache for specific transaction
    pub fn clear_transactional_cache(&mut self, transaction: &Transaction) {
        self.transactional_cache.clear(transaction);
    }

    /// Clear all transactional cache
    pub fn clear_all_transactional_cache(&mut self) {
        self.transactional_cache.clear_all();
    }
}

/// Transactional Cache contains data contracts cache per transaction
/// and provide convenient methods to insert and get data contracts from the cache
pub struct DataContractTransactionalCache {
    cache_map: HashMap<TransactionPointerAddress, Cache<[u8; 32], Arc<ContractFetchInfo>>>,
    max_capacity: u64,
}

impl DataContractTransactionalCache {
    /// Creates new transactional cache
    pub fn new(max_capacity: u64) -> Self {
        Self {
            cache_map: HashMap::new(),
            max_capacity,
        }
    }

    /// Insert a data contract with fetch info to cache
    pub fn insert(&mut self, transaction: &Transaction, fetch_info: Arc<ContractFetchInfo>) {
        let transaction_pointer_address = self.retrieve_transaction_pointer_address(transaction);

        let cache = self
            .cache_map
            .entry(transaction_pointer_address)
            .or_insert_with(|| Cache::new(self.max_capacity));

        cache.insert(fetch_info.contract.id.to_buffer(), fetch_info);
    }

    /// Returns a data contract from cache if present
    pub fn get(
        &self,
        transaction: &Transaction,
        data_contract_id: [u8; 32],
    ) -> Option<Arc<ContractFetchInfo>> {
        self.get_cache(transaction)
            .and_then(|cache| cache.get(&data_contract_id))
    }

    /// Clear cache for specific transaction
    fn clear(&mut self, transaction: &Transaction) {
        let transaction_pointer_address = self.retrieve_transaction_pointer_address(transaction);

        self.cache_map.remove(&transaction_pointer_address);
    }

    /// Clear all transactional cache
    fn clear_all(&mut self) {
        self.cache_map.clear();
    }

    /// Returns cache for transaction or error if not present
    fn get_cache(
        &self,
        transaction: &Transaction,
    ) -> Option<&Cache<[u8; 32], Arc<ContractFetchInfo>>> {
        let transaction_pointer_address = self.retrieve_transaction_pointer_address(transaction);

        self.cache_map.get(&transaction_pointer_address)
    }

    /// Get transaction pointer address from transaction reference
    fn retrieve_transaction_pointer_address(
        &self,
        transaction: &Transaction,
    ) -> TransactionPointerAddress {
        let transaction_raw_pointer = transaction as *const Transaction;

        transaction_raw_pointer as TransactionPointerAddress
    }
}
