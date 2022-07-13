use crate::drive::batch::GroveDbOpBatch;
use crate::drive::fee_pools::fee_pool_vec_path;
use crate::fee_pools::epochs::Epoch;
use grovedb::batch::GroveDbOp;
use grovedb::batch::Op::Insert;
use grovedb::Element;
use crate::fee_pools::epochs_root_tree_key_constants::KEY_STORAGE_FEE_POOL;

pub mod epochs;
pub mod epochs_root_tree_key_constants;

pub fn add_create_fee_pool_trees_operations(batch: &mut GroveDbOpBatch) {
    // Update storage credit pool
    batch.add_insert(
        fee_pool_vec_path(),
        KEY_STORAGE_FEE_POOL.to_vec(),
        Element::new_item(0u64.to_be_bytes().to_vec()),
    );

    // We need to insert 50 years worth of epochs,
    // with 20 epochs per year that's 1000 epochs
    for i in 0..1000 {
        let epoch = Epoch::new(i);
        epoch.add_init_empty_operations(batch);
    }
}

pub fn update_storage_fee_distribution_pool_operation(storage_fee: u64) -> GroveDbOp {
    GroveDbOp {
        path: fee_pool_vec_path(),
        key: KEY_STORAGE_FEE_POOL.to_vec(),
        op: Insert {
            element: Element::new_item(storage_fee.to_be_bytes().to_vec()),
        },
    }
}
