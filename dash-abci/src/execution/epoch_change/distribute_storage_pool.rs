use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal_macros::dec;
use crate::error::Error;
use crate::platform::Platform;
use rs_drive::drive::batch::GroveDbOpBatch;
use crate::execution::constants;
use rs_drive::fee_pools::epochs::Epoch;
use rs_drive::fee_pools::update_storage_fee_distribution_pool_operation;
use rs_drive::grovedb::TransactionArg;
use crate::error::execution::ExecutionError;
use crate::execution::constants::{EPOCHS_PER_YEAR, EPOCHS_PER_YEAR_DEC};

impl Platform {
    pub fn distribute_storage_fee_distribution_pool_to_epochs_operations(
        &self,
        epoch_index: u16,
        transaction: TransactionArg,
        batch: &mut GroveDbOpBatch,
    ) -> Result<(), Error> {
        let storage_distribution_fees = self.drive.get_aggregate_storage_fees_in_current_distribution_pool(transaction)?;
        let storage_distribution_fees = Decimal::new(storage_distribution_fees as i64, 0);

        // a separate buffer from which we withdraw to correctly calculate fee share
        let mut storage_distribution_fees_buffer = storage_distribution_fees;

        if storage_distribution_fees == dec!(0.0) {
            return Ok(());
        }

        for year in 0..50u16 {
            let distribution_for_that_year_ratio = constants::FEE_DISTRIBUTION_TABLE[year as usize];

            let year_fee_share = storage_distribution_fees * distribution_for_that_year_ratio;
            let epoch_fee_share_dec = (year_fee_share / EPOCHS_PER_YEAR_DEC).floor();
            let epoch_fee_share=
                epoch_fee_share_dec.to_u64().ok_or(
                    Error::Execution(ExecutionError::Overflow(
                        "storage distribution fees are not fitting in a u64",
                    ))
                )?;


            let starting_epoch_index = epoch_index + EPOCHS_PER_YEAR * year;

            for index in starting_epoch_index..starting_epoch_index + 20 {
                let epoch_pool = Epoch::new(index);

                let storage_fee = self.drive.get_epoch_storage_credits_for_distribution(&epoch_pool, transaction)?;

                batch.push(epoch_pool
                    .update_storage_credits_for_distribution_operation(storage_fee + epoch_fee_share));

                storage_distribution_fees_buffer -= epoch_fee_share_dec;
            }
        }

        let storage_distribution_fees_buffer =
            storage_distribution_fees_buffer.to_u64().ok_or(
                Error::Execution(ExecutionError::Overflow(
                    "storage distribution fees are not fitting in a u64",
                ))
            )?;

        batch.push(update_storage_fee_distribution_pool_operation(storage_distribution_fees_buffer));

        Ok(())
    }
}

#[cfg(test)]
mod tests {

    mod distribute_storage_fee_distribution_pool {
        use crate::common::helpers::setup::setup_platform_with_initial_state_structure;
        use rs_drive::error::drive::DriveError;
        use rs_drive::fee_pools::epochs::Epoch;
        use rust_decimal::Decimal;
        use rust_decimal_macros::dec;
        use rs_drive::common::helpers::epoch::get_storage_credits_for_distribution_for_epochs_in_range;
        use rs_drive::drive::batch::GroveDbOpBatch;
        use rs_drive::fee_pools::update_storage_fee_distribution_pool_operation;
        #[test]
        fn test_nothing_to_distribute() {
            let platform = setup_platform_with_initial_state_structure();
            let transaction = platform.drive.grove.start_transaction();

            let epoch_index = 0;

            // Storage fee distribution pool is 0 after fee pools initialization

            let mut batch = GroveDbOpBatch::new();

            platform
                .distribute_storage_fee_distribution_pool_to_epochs_operations(
                    epoch_index,
                    Some(&transaction),
                    &mut batch,
                )
                .expect("should distribute storage fee pool");

            match platform
                .drive
                .grove_apply_batch(batch, false, Some(&transaction))
            {
                Ok(()) => assert!(false, "should return BatchIsEmpty error"),
                Err(e) => match e {
                    rs_drive::error::Error::Drive(DriveError::BatchIsEmpty()) => assert!(true),
                    _ => assert!(false, "invalid error type"),
                },
            }

            let storage_fees = get_storage_credits_for_distribution_for_epochs_in_range(
                &platform.drive,
                epoch_index..1000,
                Some(&transaction),
            );

            let reference_fees: Vec<u64> = (0..1000).map(|_| 0u64).collect();

            assert_eq!(storage_fees, reference_fees);
        }

        #[test]
        fn test_distribution_overflow() {
            let platform = setup_platform_with_initial_state_structure();
            let transaction = platform.drive.grove.start_transaction();

            let storage_pool = u64::MAX;
            let epoch_index = 0;

            let mut batch = GroveDbOpBatch::new();

            batch.push(update_storage_fee_distribution_pool_operation(storage_pool));

            // Apply storage fee distribution pool update
            platform
                .drive
                .grove_apply_batch(batch, false, Some(&transaction))
                .expect("should apply batch");

            let mut batch = GroveDbOpBatch::new();

            platform
                .distribute_storage_fee_distribution_pool_to_epochs_operations(
                    epoch_index,
                    Some(&transaction),
                    &mut batch,
                )
                .expect("should distribute storage fee pool");

            platform
                .drive
                .grove_apply_batch(batch, false, Some(&transaction))
                .expect("should apply batch");

            // check leftover
            let storage_fee_pool_leftover = platform.drive
                .get_aggregate_storage_fees_in_current_distribution_pool(Some(&transaction))
                .expect("should get storage fee pool");

            assert_eq!(storage_fee_pool_leftover, 0);
        }

        #[test]
        fn test_deterministic_distribution() {
            let platform = setup_platform_with_initial_state_structure();
            let transaction = platform.drive.grove.start_transaction();

            let storage_pool = 1000;
            let epoch_index = 42;

            let mut batch = GroveDbOpBatch::new();

            // init additional epochs pools as it will be done in epoch_change
            for i in 1000..=1000 + epoch_index {
                let epoch = Epoch::new(i);
                epoch
                    .add_init_empty_operations(&mut batch);
            }

            batch.push(update_storage_fee_distribution_pool_operation(storage_pool));

            // Apply storage fee distribution pool update
            platform
                .drive
                .grove_apply_batch(batch, false, Some(&transaction))
                .expect("should apply batch");

            let mut batch = GroveDbOpBatch::new();

            platform
                .distribute_storage_fee_distribution_pool_to_epochs_operations(
                    epoch_index,
                    Some(&transaction),
                    &mut batch,
                )
                .expect("should distribute storage fee pool");

            platform
                .drive
                .grove_apply_batch(batch, false, Some(&transaction))
                .expect("should apply batch");

            // check leftover
            let storage_fee_pool_leftover = platform.drive
                .get_aggregate_storage_fees_in_current_distribution_pool(Some(&transaction))
                .expect("should get storage fee pool");

            assert_eq!(storage_fee_pool_leftover, 0);

            // collect all the storage fee values of the 1000 epochs pools
            let storage_fees =
                get_storage_credits_for_distribution_for_epochs_in_range(&platform.drive, epoch_index..epoch_index+1000, Some(&transaction));

            // compare them with reference table
            #[rustfmt::skip]
                let reference_fees: [u64; 1] = [1

            ];

            assert_eq!(storage_fees, reference_fees);

            /*

            Repeat distribution to ensure deterministic results

             */

            let mut batch = GroveDbOpBatch::new();

            // refill storage fee pool once more
            batch.push(update_storage_fee_distribution_pool_operation(storage_pool));

            // Apply storage fee distribution pool update
            platform.drive
                .grove_apply_batch(batch, false, Some(&transaction))
                .expect("should apply batch");

            let mut batch = GroveDbOpBatch::new();

            // distribute fees once more
            platform
                .distribute_storage_fee_distribution_pool_to_epochs_operations(
                    epoch_index,
                    Some(&transaction),
                    &mut batch
                )
                .expect("should distribute storage fee pool");

            platform.drive
                .grove_apply_batch(batch, false, Some(&transaction))
                .expect("should apply batch");

            // collect all the storage fee values of the 1000 epochs pools again
            let storage_fees =
                get_storage_credits_for_distribution_for_epochs_in_range(&platform.drive, epoch_index..epoch_index+1000, Some(&transaction));

            // assert that all the values doubled meaning that distribution is reproducible
            assert_eq!(
                storage_fees,
                reference_fees
                    .iter()
                    .map(|val| val * 2)
                    .collect::<Vec<u64>>()
            );
        }
    }

    mod update_storage_fee_distribution_pool {
        use rs_drive::drive::batch::GroveDbOpBatch;
        use rs_drive::grovedb;
        use rs_drive::error::Error as DriveError;
        use rs_drive::fee_pools::update_storage_fee_distribution_pool_operation;
        use crate::common::helpers::setup::{setup_platform, setup_platform_with_initial_state_structure};

        #[test]
        fn test_error_if_pool_is_not_initiated() {
            let platform = setup_platform();
            let transaction = platform.drive.grove.start_transaction();

            let storage_fee = 42;

            let mut batch = GroveDbOpBatch::new();

            batch.push(update_storage_fee_distribution_pool_operation(storage_fee));

            match platform.drive.grove_apply_batch(batch, false, Some(&transaction)) {
                Ok(_) => assert!(
                    false,
                    "should not be able to update genesis time on uninit fee pools"
                ),
                Err(e) => match e {
                    DriveError::GroveDB(grovedb::Error::PathKeyNotFound(_)) => {
                        assert!(true)
                    }
                    _ => assert!(false, "invalid error type"),
                },
            }
        }

        #[test]
        fn test_update_and_get_value() {
            let platform = setup_platform_with_initial_state_structure();
            let transaction = platform.drive.grove.start_transaction();

            let storage_fee = 42;

            let mut batch = GroveDbOpBatch::new();

            batch.push(update_storage_fee_distribution_pool_operation(storage_fee));

            platform.drive
                .grove_apply_batch(batch, false, Some(&transaction))
                .expect("should apply batch");

            let stored_storage_fee = platform.drive
                .get_aggregate_storage_fees_in_current_distribution_pool(Some(&transaction))
                .expect("should get storage fee pool");

            assert_eq!(storage_fee, stored_storage_fee);
        }
    }

    mod get_storage_fee_distribution_pool_fees {
        use rs_drive::drive::batch::GroveDbOpBatch;
        use rs_drive::drive::fee_pools::pools_vec_path;
        use rs_drive::grovedb;
        use rs_drive::error::Error as DriveError;
        use rs_drive::error::fee::FeeError;
        use rs_drive::fee_pools::epochs_root_tree_key_constants::KEY_STORAGE_FEE_POOL;
        use rs_drive::grovedb::Element;
        use crate::common::helpers::setup::{setup_platform, setup_platform_with_initial_state_structure};

        #[test]
        fn test_error_if_pool_is_not_initiated() {
            let platform = setup_platform();
            let transaction = platform.drive.grove.start_transaction();

            match platform.drive
                .get_aggregate_storage_fees_in_current_distribution_pool(Some(&transaction)) {
                Ok(_) => assert!(
                    false,
                    "should not be able to get genesis time on uninit fee pools"
                ),
                Err(e) => match e {
                    DriveError::GroveDB(grovedb::Error::PathNotFound(_)) => assert!(true),
                    _ => assert!(false, "invalid error type"),
                },
            }
        }

        #[test]
        fn test_error_if_wrong_value_encoded() {
            let platform = setup_platform_with_initial_state_structure();
            let transaction = platform.drive.grove.start_transaction();

            let mut batch = GroveDbOpBatch::new();

            batch
                .add_insert(
                    pools_vec_path(),
                    KEY_STORAGE_FEE_POOL.to_vec(),
                    Element::Item(u128::MAX.to_be_bytes().to_vec(), None),
                );

            platform.drive
                .grove_apply_batch(batch, false, Some(&transaction))
                .expect("should apply batch");

            match platform.drive
                .get_aggregate_storage_fees_in_current_distribution_pool(Some(&transaction)) {
                Ok(_) => assert!(false, "should not be able to decode stored value"),
                Err(e) => match e {
                    DriveError::Fee(
                        FeeError::CorruptedStorageFeePoolInvalidItemLength(_),
                    ) => {
                        assert!(true)
                    }
                    _ => assert!(false, "invalid error type"),
                },
            }
        }
    }
}
