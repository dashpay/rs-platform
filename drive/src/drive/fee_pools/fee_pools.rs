#[cfg(test)]
mod tests {
    use crate::common::tests::helpers::setup::setup_drive;
    use crate::common::tests::helpers::setup::setup_fee_pools;
    use crate::fee_pools::epochs::Epoch;
    use crate::error;
    use crate::drive::batch::GroveDbOpBatch;

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
                let epoch = super::Epoch::new(epoch_index);

                let storage_fee = drive
                    .get_epoch_storage_credits_for_distribution(&epoch, Some(&transaction))
                    .expect("should get storage fee");

                assert_eq!(storage_fee, 0);
            }

            let epoch = super::Epoch::new(1000); // 1001th epochs pool

            match drive.get_epoch_storage_credits_for_distribution(&epoch, Some(&transaction)) {
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

            let current_epoch_pool = super::Epoch::new(0);

            let start_block_height = 10;
            let start_block_time = 1655396517912;
            let multiplier = 42;

            let mut batch = super::GroveDbOpBatch::new();

            fee_pools.add_shift_current_epoch_pool_operations(
                &current_epoch_pool,
                start_block_height,
                start_block_time,
                multiplier,
                &mut batch,
            );

            drive
                .grove_apply_batch(batch, false, Some(&transaction))
                .expect("should apply batch");

            let next_thousandth_epoch = super::Epoch::new(1000);

            let storage_fee_pool = drive
                .get_epoch_storage_credits_for_distribution(&next_thousandth_epoch, Some(&transaction))
                .expect("should get storage fee");

            assert_eq!(storage_fee_pool, 0);

            let stored_start_block_height = drive
                .get_epoch_start_block_height(&current_epoch_pool, Some(&transaction))
                .expect("should get start block height");

            assert_eq!(stored_start_block_height, start_block_height);

            let stored_start_block_time = drive
                .get_epoch_start_time(&current_epoch_pool, Some(&transaction))
                .expect("should get start time");

            assert_eq!(stored_start_block_time, start_block_time);

            let stored_multiplier = drive
                .get_epoch_fee_multiplier(&current_epoch_pool,Some(&transaction))
                .expect("should get fee multiplier");

            assert_eq!(stored_multiplier, multiplier);
        }
    }
}
