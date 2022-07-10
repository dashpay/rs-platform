use grovedb::{Element, TransactionArg};

use crate::drive::{Drive, RootTree};
use crate::drive::batch::GroveDbOpBatch;
use crate::error::fee::FeeError;
use crate::error::Error;
use crate::fee::epoch::EpochInfo;
use crate::fee::fees_aggregate::FeesAggregate;

impl Drive {

    pub fn get_path<'a>() -> [&'a [u8]; 1] {
        [Into::<&[u8; 1]>::into(RootTree::Pools)]
    }


}

#[cfg(test)]
mod tests {
    use crate::{
        error,
        fee::pools::tests::helpers::setup::{setup_drive, setup_fee_pools},
    };

    use rust_decimal_macros::dec;

    use crate::drive::storage::batch::GroveDbOpBatch;
    use crate::fee::pools::epoch::epoch_pool::EpochPool;

    mod create_fee_pool_trees {
        #[test]
        fn test_values_are_set() {
            let drive = super::setup_drive();
            let (transaction, fee_pools) = super::setup_fee_pools(&drive, None);

            let storage_fee_pool = fee_pools
                .get_storage_fee_distribution_pool_fees(&drive, Some(&transaction))
                .expect("should get storage fee pool");

            assert_eq!(storage_fee_pool, 0i64);
        }

        #[test]
        fn test_epoch_pools_are_created() {
            let drive = super::setup_drive();
            let (transaction, _) = super::setup_fee_pools(&drive, None);

            for epoch_index in 0..1000 {
                let epoch_pool = super::EpochPool::new(epoch_index, &drive);

                let storage_fee = epoch_pool
                    .get_storage_fee(Some(&transaction))
                    .expect("should get storage fee");

                assert_eq!(storage_fee, super::dec!(0));
            }

            let epoch_pool = super::EpochPool::new(1000, &drive); // 1001th epoch pool

            match epoch_pool.get_storage_fee(Some(&transaction)) {
                Ok(_) => assert!(false, "must be an error"),
                Err(e) => match e {
                    super::error::Error::GroveDB(_) => assert!(true),
                    _ => assert!(false, "invalid error type"),
                },
            }
        }
    }

    mod shift_current_epoch_pool {
        #[test]
        fn test_values_are_set() {
            let drive = super::setup_drive();
            let (transaction, fee_pools) = super::setup_fee_pools(&drive, None);

            let current_epoch_pool = super::EpochPool::new(0, &drive);

            let start_block_height = 10;
            let start_block_time = 1655396517912;
            let multiplier = 42;

            let mut batch = super::GroveDbOpBatch::new(&drive);

            fee_pools
                .add_shift_current_epoch_pool_operations(
                    &current_epoch_pool,
                    start_block_height,
                    start_block_time,
                    multiplier,
                    &mut batch,
                );

            drive
                .apply_batch(batch, false, Some(&transaction))
                .expect("should apply batch");

            let next_thousandth_epoch = super::EpochPool::new(1000, &drive);

            let storage_fee_pool = next_thousandth_epoch
                .get_storage_fee(Some(&transaction))
                .expect("should get storage fee");

            assert_eq!(storage_fee_pool, super::dec!(0));

            let stored_start_block_height = current_epoch_pool
                .get_start_block_height(Some(&transaction))
                .expect("should get start block height");

            assert_eq!(stored_start_block_height, start_block_height);

            let stored_start_block_time = current_epoch_pool
                .get_start_time(Some(&transaction))
                .expect("should get start time");

            assert_eq!(stored_start_block_time, start_block_time);

            let stored_multiplier = current_epoch_pool
                .get_fee_multiplier(Some(&transaction))
                .expect("should get fee multiplier");

            assert_eq!(stored_multiplier, multiplier);
        }
    }
}
