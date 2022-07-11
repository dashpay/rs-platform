use grovedb::{Element, TransactionArg};
use crate::drive::Drive;
use crate::error::fee::FeeError;
use crate::error::Error;
use crate::fee_pools::epochs::EpochPool;

use crate::fee_pools::epochs::tree_key_constants;

impl Drive {
    pub fn get_epoch_pool_start_block_height(&self, epoch_pool: &EpochPool, transaction: TransactionArg) -> Result<u64, Error> {
        let element = self
            .grove
            .get(
                epoch_pool.get_path(),
                tree_key_constants::KEY_START_BLOCK_HEIGHT.as_slice(),
                transaction,
            )
            .unwrap()
            .map_err(Error::GroveDB)?;

        if let Element::Item(item, _) = element {
            Ok(u64::from_be_bytes(item.as_slice().try_into().map_err(
                |_| Error::Fee(FeeError::CorruptedStartBlockHeightItemLength()),
            )?))
        } else {
            Err(Error::Fee(FeeError::CorruptedStartBlockHeightNotItem()))
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use grovedb::Element;
    use rust_decimal_macros::dec;
    use crate::drive::batch::GroveDbOpBatch;
    use crate::common::tests::helpers::setup::{setup_drive, setup_fee_pools};

    use crate::error;
    use crate::error::fee::FeeError;

    use super::EpochPool;

    #[test]
    fn test_update_start_block_height() {
        let drive = setup_drive();

        let (transaction, _) = setup_fee_pools(&drive, None);

        let epoch_pool = EpochPool::new(0);

        let start_block_height = 1;

        let mut batch = GroveDbOpBatch::new();

        epoch_pool
            .add_update_start_block_height_operations(&mut batch, start_block_height)
            .expect("should update start block height");

        drive
            .grove_apply_batch(batch, false, Some(&transaction))
            .expect("should apply batch");

        let actual_start_block_height = epoch_pool
            .get_start_block_height(Some(&transaction))
            .expect("should get start block height");

        assert_eq!(start_block_height, actual_start_block_height);
    }

    mod get_start_block_height {
        #[test]
        fn test_error_if_epoch_pool_is_not_initiated() {
            let drive = super::setup_drive();

            let (transaction, _) = super::setup_fee_pools(&drive, None);

            let non_initiated_epoch_pool = super::EpochPool::new(7000, &drive);

            match non_initiated_epoch_pool.get_start_block_height(Some(&transaction)) {
                Ok(_) => assert!(
                    false,
                    "should not be able to get start block height on uninit epochs pool"
                ),
                Err(e) => match e {
                    super::error::Error::GroveDB(grovedb::Error::PathNotFound(_)) => assert!(true),
                    _ => assert!(false, "invalid error type"),
                },
            }
        }

        #[test]
        fn test_error_if_value_is_not_set() {
            let drive = super::setup_drive();

            let (transaction, _) = super::setup_fee_pools(&drive, None);

            let epoch_pool = super::EpochPool::new(0, &drive);

            match epoch_pool.get_start_block_height(Some(&transaction)) {
                Ok(_) => assert!(false, "must be an error"),
                Err(e) => match e {
                    super::error::Error::GroveDB(_) => assert!(true),
                    _ => assert!(false, "invalid error type"),
                },
            }
        }

        #[test]
        fn test_error_if_value_has_invalid_length() {
            let drive = super::setup_drive();

            let (transaction, _) = super::setup_fee_pools(&drive, None);

            let epoch_pool = super::EpochPool::new(0, &drive);

            drive
                .grove
                .insert(
                    epoch_pool.get_path(),
                    super::constants::KEY_START_BLOCK_HEIGHT.as_slice(),
                    super::Element::Item(u128::MAX.to_be_bytes().to_vec(), None),
                    Some(&transaction),
                )
                .unwrap()
                .expect("should insert invalid data");

            match epoch_pool.get_start_block_height(Some(&transaction)) {
                Ok(_) => assert!(false, "should not be able to decode stored value"),
                Err(e) => match e {
                    super::error::Error::Fee(
                        super::FeeError::CorruptedStartBlockHeightItemLength(),
                    ) => {
                        assert!(true)
                    }
                    _ => assert!(false, "invalid error type"),
                },
            }
        }

        #[test]
        fn test_error_if_element_has_invalid_type() {
            let drive = super::setup_drive();

            let (transaction, _) = super::setup_fee_pools(&drive, None);

            let epoch_pool = super::EpochPool::new(0, &drive);

            drive
                .grove
                .insert(
                    epoch_pool.get_path(),
                    super::constants::KEY_START_BLOCK_HEIGHT.as_slice(),
                    super::Element::empty_tree(),
                    Some(&transaction),
                )
                .unwrap()
                .expect("should insert invalid data");

            match epoch_pool.get_start_block_height(Some(&transaction)) {
                Ok(_) => assert!(false, "should not be able to decode stored value"),
                Err(e) => match e {
                    super::error::Error::Fee(
                        super::FeeError::CorruptedStartBlockHeightNotItem(),
                    ) => {
                        assert!(true)
                    }
                    _ => assert!(false, "invalid error type"),
                },
            }
        }
    }

    mod init_empty {
        use crate::drive::storage::batch::GroveDbOpBatch;

        #[test]
        fn test_error_if_fee_pools_not_initialized() {
            let drive = super::setup_drive();
            let (transaction, _) = super::setup_fee_pools(
                &drive,
                Some(super::SetupFeePoolsOptions {
                    apply_fee_pool_structure: false,
                }),
            );

            let epoch = super::EpochPool::new(1042, &drive);

            let mut batch = GroveDbOpBatch::new(&drive);

            epoch
                .add_init_empty_operations(&mut batch)
                .expect("should init empty pool");

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
            let drive = super::setup_drive();
            let (transaction, _) = super::setup_fee_pools(&drive, None);

            let epoch = super::EpochPool::new(1042, &drive);

            let mut batch = GroveDbOpBatch::new(&drive);

            epoch
                .add_init_empty_operations(&mut batch)
                .expect("should init an epochs pool");

            drive
                .grove_apply_batch(batch, false, Some(&transaction))
                .expect("should apply batch");

            let storage_fee = epoch
                .get_storage_fee(Some(&transaction))
                .expect("should get storage fee");

            assert_eq!(storage_fee, super::dec!(0.0));
        }
    }

    mod init_current {
        use crate::drive::storage::batch::GroveDbOpBatch;

        #[test]
        fn test_values_are_set() {
            let drive = super::setup_drive();
            let (transaction, _) = super::setup_fee_pools(&drive, None);

            let epoch = super::EpochPool::new(1042, &drive);

            let multiplier = 42;
            let start_time = 1;
            let start_block_height = 2;

            let mut batch = GroveDbOpBatch::new(&drive);

            epoch
                .add_init_empty_operations(&mut batch)
                .expect("should init empty epochs pool");

            epoch
                .add_init_current_operations(multiplier, start_block_height, start_time, &mut batch)
                .expect("should init an epochs pool");

            drive
                .grove_apply_batch(batch, false, Some(&transaction))
                .expect("should apply batch");

            let stored_multiplier = epoch
                .get_fee_multiplier(Some(&transaction))
                .expect("should get multiplier");

            assert_eq!(stored_multiplier, multiplier);

            let stored_start_time = epoch
                .get_start_time(Some(&transaction))
                .expect("should get start time");

            assert_eq!(stored_start_time, start_time);

            let stored_block_height = epoch
                .get_start_block_height(Some(&transaction))
                .expect("should get start block height");

            assert_eq!(stored_block_height, start_block_height);

            let stored_processing_fee = epoch
                .get_processing_fee(Some(&transaction))
                .expect("should get processing fee");

            assert_eq!(stored_processing_fee, 0);

            let proposers = epoch
                .get_proposers(1, Some(&transaction))
                .expect("should get proposers");

            assert_eq!(proposers, vec!());
        }
    }

    mod mark_as_paid {
        use crate::drive::storage::batch::GroveDbOpBatch;

        #[test]
        fn test_values_are_deleted() {
            let drive = super::setup_drive();
            let (transaction, _) = super::setup_fee_pools(&drive, None);

            let epoch = super::EpochPool::new(0, &drive);

            let mut batch = GroveDbOpBatch::new(&drive);

            epoch
                .add_init_current_operations(1, 2, 3, &mut batch)
                .expect("should init an epochs pool");

            // Apply init current
            drive
                .grove_apply_batch(batch, false, Some(&transaction))
                .expect("should apply batch");

            let mut batch = GroveDbOpBatch::new(&drive);

            epoch
                .add_mark_as_paid_operations(&mut batch, Some(&transaction))
                .expect("should mark epochs as paid");

            drive
                .grove_apply_batch(batch, false, Some(&transaction))
                .expect("should apply batch");

            match drive
                .grove
                .get(
                    epoch.get_path(),
                    super::constants::KEY_PROPOSERS.as_slice(),
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

            match epoch.get_processing_fee(Some(&transaction)) {
                Ok(_) => assert!(false, "should not be able to get processing fee"),
                Err(e) => match e {
                    super::error::Error::GroveDB(grovedb::Error::PathKeyNotFound(_)) => {
                        assert!(true)
                    }
                    _ => assert!(false, "invalid error type"),
                },
            }

            match epoch.get_storage_fee(Some(&transaction)) {
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
