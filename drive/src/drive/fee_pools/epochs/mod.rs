pub mod block_count;
pub mod credit_distribution_pools;
pub mod proposers;
pub mod start_block;
pub mod start_time;

#[cfg(test)]
mod tests {
    use crate::common::helpers::setup::{setup_drive, setup_drive_with_initial_state_structure};

    use crate::common::helpers::setup::SetupFeePoolsOptions;
    use crate::drive::batch::GroveDbOpBatch;
    use crate::error;
    use crate::fee_pools::epochs::epoch_key_constants;
    use crate::fee_pools::epochs::Epoch;

    mod init_empty {

        #[test]
        fn test_error_if_fee_pools_not_initialized() {
            let drive = super::setup_drive();
            let transaction = drive.grove.start_transaction();

            let epoch = super::Epoch::new(1042);

            let mut batch = super::GroveDbOpBatch::new();

            epoch.add_init_empty_operations(&mut batch);

            match drive.grove_apply_batch(batch, false, Some(&transaction)) {
                Ok(_) => assert!(false, "should not be able to init epochs without FeePools"),
                Err(e) => match e {
                    super::error::Error::GroveDB(grovedb::Error::PathKeyNotFound(_)) => {
                        assert!(true)
                    }
                    _ => assert!(false, "invalid error type"),
                },
            }
        }

        #[test]
        fn test_values_are_set() {
            let drive = super::setup_drive_with_initial_state_structure();
            let transaction = drive.grove.start_transaction();

            let epoch = super::Epoch::new(1042);

            let mut batch = super::GroveDbOpBatch::new();

            epoch.add_init_empty_operations(&mut batch);

            drive
                .grove_apply_batch(batch, false, Some(&transaction))
                .expect("should apply batch");

            let storage_fee = drive
                .get_epoch_storage_credits_for_distribution(&epoch, Some(&transaction))
                .expect("expected to get storage credits in epoch pool");

            assert_eq!(storage_fee, 0);
        }
    }

    mod init_current {

        #[test]
        fn test_values_are_set() {
            let drive = super::setup_drive_with_initial_state_structure();
            let transaction = drive.grove.start_transaction();

            let epoch = super::Epoch::new(1042);

            let multiplier = 42.0;
            let start_time = 1;
            let start_block_height = 2;

            let mut batch = super::GroveDbOpBatch::new();

            epoch.add_init_empty_operations(&mut batch);

            epoch.add_init_current_operations(
                multiplier,
                start_block_height,
                start_time,
                &mut batch,
            );

            drive
                .grove_apply_batch(batch, false, Some(&transaction))
                .expect("should apply batch");

            let stored_multiplier = drive
                .get_epoch_fee_multiplier(&epoch, Some(&transaction))
                .expect("should get multiplier");

            assert_eq!(stored_multiplier, multiplier);

            let stored_start_time = drive
                .get_epoch_start_time(&epoch, Some(&transaction))
                .expect("should get start time");

            assert_eq!(stored_start_time, start_time);

            let stored_block_height = drive
                .get_epoch_start_block_height(&epoch, Some(&transaction))
                .expect("should get start block height");

            assert_eq!(stored_block_height, start_block_height);

            drive
                .get_epoch_processing_credits_for_distribution(&epoch, Some(&transaction))
                .expect_err("should not get processing fee");

            let proposers = drive
                .get_epoch_proposers(&epoch, 1, Some(&transaction))
                .expect("should get proposers");

            assert_eq!(proposers, vec!());
        }
    }

    mod mark_as_paid {

        #[test]
        fn test_values_are_deleted() {
            let drive = super::setup_drive_with_initial_state_structure();
            let transaction = drive.grove.start_transaction();

            let epoch = super::Epoch::new(0);

            let mut batch = super::GroveDbOpBatch::new();

            epoch.add_init_current_operations(1.0, 2, 3, &mut batch);

            // Apply init current
            drive
                .grove_apply_batch(batch, false, Some(&transaction))
                .expect("should apply batch");

            let mut batch = super::GroveDbOpBatch::new();

            epoch.add_mark_as_paid_operations(&mut batch);

            drive
                .grove_apply_batch(batch, false, Some(&transaction))
                .expect("should apply batch");

            match drive
                .grove
                .get(
                    epoch.get_path(),
                    super::epoch_key_constants::KEY_PROPOSERS.as_slice(),
                    Some(&transaction),
                )
                .unwrap()
            {
                Ok(_) => assert!(false, "should not be able to get proposers"),
                Err(e) => match e {
                    grovedb::Error::PathKeyNotFound(_) => assert!(true),
                    _ => assert!(false, "invalid error type"),
                },
            }

            match drive.get_epoch_processing_credits_for_distribution(&epoch, Some(&transaction)) {
                Ok(_) => assert!(false, "should not be able to get processing fee"),
                Err(e) => match e {
                    super::error::Error::GroveDB(grovedb::Error::PathKeyNotFound(_)) => {
                        assert!(true)
                    }
                    _ => assert!(false, "invalid error type"),
                },
            }

            match drive.get_epoch_storage_credits_for_distribution(&epoch, Some(&transaction)) {
                Ok(_) => assert!(false, "should not be able to get storage fee"),
                Err(e) => match e {
                    super::error::Error::GroveDB(grovedb::Error::PathKeyNotFound(_)) => {
                        assert!(true)
                    }
                    _ => assert!(false, "invalid error type"),
                },
            }
        }
    }
}
