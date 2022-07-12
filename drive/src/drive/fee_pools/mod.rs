use crate::drive::RootTree;

pub mod constants;
pub mod epochs;
pub mod fee_distribution;
pub mod fee_pools;
pub mod storage_fee_distribution_pool;

pub(crate) fn fee_pool_vec_path() -> Vec<Vec<u8>> {
    vec![vec![RootTree::Pools as u8]]
}
