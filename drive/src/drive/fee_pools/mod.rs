use crate::drive::fee_pools::constants::KEY_STORAGE_FEE_POOL;
use crate::drive::RootTree;

pub mod constants;
pub mod epochs;
pub mod fee_distribution;
pub mod fee_pools;
pub mod storage_fee_distribution_pool;

pub(crate) fn fee_pool_vec_path() -> Vec<Vec<u8>> {
    vec![vec![RootTree::Pools as u8]]
}

pub(crate) fn aggregate_storage_fees_distribution_pool_path() -> [&'static [u8]; 2] {
    [
        Into::<&[u8; 1]>::into(RootTree::Pools),
        KEY_STORAGE_FEE_POOL,
    ]
}


pub(crate) fn aggregate_storage_fees_distribution_pool_vec_path() -> Vec<Vec<u8>> {
    vec![vec![RootTree::Pools as u8], KEY_STORAGE_FEE_POOL.to_vec()]
}
