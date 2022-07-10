use grovedb::batch::GroveDbOp;
use grovedb::batch::Op::Insert;
use grovedb::Element;
use crate::drive::batch::GroveDbOpBatch;
use crate::drive::fee_pools::constants::KEY_STORAGE_FEE_POOL;
use crate::fee_pools::epoch_pool::EpochPool;

pub mod epoch_pool;

pub fn add_create_fee_pool_trees_operations(
    batch: &mut GroveDbOpBatch,
) {
    // init fee pool subtree
    batch.insert_empty_tree(vec![], FeePools::get_path()[0]);

    // Update storage credit pool
    batch.insert(FeePools::get_path(),
        KEY_STORAGE_FEE_POOL.to_vec(),
        Element::Item(0i64.to_le_bytes().to_vec(), None),
    );

    // We need to insert 50 years worth of epochs,
    // with 20 epochs per year that's 1000 epochs
    for i in 0..1000 {
        let epoch = EpochPool::new(i);
        epoch.add_init_empty_operations(batch)?;
    }
}

pub fn update_storage_fee_distribution_pool_operation(
    storage_fee: i64,
) -> GroveDbOp {
    GroveDbOp {
        path: FeePools::get_path(),
        key: KEY_STORAGE_FEE_POOL.to_vec(),
        op: Insert { element: Element::new_item(storage_fee.to_le_bytes().to_vec())}
    }
}