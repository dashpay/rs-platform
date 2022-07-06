use grovedb::{Element, TransactionArg};
use rust_decimal_macros::dec;

use crate::drive::object_size_info::{KeyInfo, PathKeyElementInfo};
use crate::drive::Drive;
use crate::error::fee::FeeError;
use crate::error::Error;
use crate::fee::pools::fee_pools::FeePools;

use super::constants;

pub struct EpochPool<'e> {
    pub index: u16,
    pub key: [u8; 2],
    pub drive: &'e Drive,
}

impl<'e> EpochPool<'e> {
    pub fn new(index: u16, drive: &Drive) -> EpochPool {
        EpochPool {
            index,
            key: index.to_le_bytes(),
            drive,
        }
    }

    pub fn init_empty(&self) -> Result<(), Error> {
        self.drive.current_batch_insert_empty_tree(
            FeePools::get_path(),
            KeyInfo::KeyRef(&self.key),
            None,
        )?;

        // init storage fee item to 0
        self.update_storage_fee(dec!(0.0))?;

        Ok(())
    }

    pub fn init_current(
        &self,
        multiplier: u64,
        start_block_height: u64,
        start_time: i64,
    ) -> Result<(), Error> {
        self.update_start_block_height(start_block_height)?;

        self.update_processing_fee(0u64)?;

        self.init_proposers()?;

        self.update_fee_multiplier(multiplier)?;

        self.update_start_time(start_time)?;

        Ok(())
    }

    pub fn mark_as_paid(&self, transaction: TransactionArg) -> Result<(), Error> {
        self.delete_proposers_tree(transaction)?;

        self.delete_storage_fee(transaction)?;

        self.delete_processing_fee(transaction)?;

        Ok(())
    }

    pub fn get_path(&self) -> [&[u8]; 2] {
        [FeePools::get_path()[0], &self.key]
    }

    pub fn update_start_time(&self, time: i64) -> Result<(), Error> {
        self.drive
            .current_batch_insert(PathKeyElementInfo::PathFixedSizeKeyElement((
                self.get_path(),
                constants::KEY_START_TIME.as_bytes(),
                Element::Item(time.to_le_bytes().to_vec(), None),
            )))
    }

    pub fn get_start_time(&self, transaction: TransactionArg) -> Result<i64, Error> {
        let element = self
            .drive
            .grove
            .get(
                self.get_path(),
                constants::KEY_START_TIME.as_bytes(),
                transaction,
            )
            .unwrap()
            .map_err(Error::GroveDB)?;

        if let Element::Item(item, _) = element {
            Ok(i64::from_le_bytes(item.as_slice().try_into().map_err(
                |_| Error::Fee(FeeError::CorruptedStartTimeLength()),
            )?))
        } else {
            Err(Error::Fee(FeeError::CorruptedStartTimeNotItem()))
        }
    }

    pub fn get_start_block_height(&self, transaction: TransactionArg) -> Result<u64, Error> {
        let element = self
            .drive
            .grove
            .get(
                self.get_path(),
                constants::KEY_START_BLOCK_HEIGHT.as_bytes(),
                transaction,
            )
            .unwrap()
            .map_err(Error::GroveDB)?;

        if let Element::Item(item, _) = element {
            Ok(u64::from_le_bytes(item.as_slice().try_into().map_err(
                |_| Error::Fee(FeeError::CorruptedStartBlockHeightItemLength()),
            )?))
        } else {
            Err(Error::Fee(FeeError::CorruptedStartBlockHeightNotItem()))
        }
    }

    pub fn update_start_block_height(&self, start_block_height: u64) -> Result<(), Error> {
        self.drive
            .current_batch_insert(PathKeyElementInfo::PathFixedSizeKeyElement((
                self.get_path(),
                constants::KEY_START_BLOCK_HEIGHT.as_bytes(),
                Element::Item(start_block_height.to_le_bytes().to_vec(), None),
            )))
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use grovedb::Element;
    use rust_decimal_macros::dec;

    use crate::error;
    use crate::error::fee::FeeError;
    use crate::fee::pools::epoch::constants;
    use crate::fee::pools::tests::helpers::setup::SetupFeePoolsOptions;
    use crate::fee::pools::tests::helpers::setup::{setup_drive, setup_fee_pools};

    use super::EpochPool;

    #[test]
    fn test_update_start_time() {
        let drive = setup_drive();

        let (transaction, _) = setup_fee_pools(&drive, None);

        let epoch_pool = super::EpochPool::new(0, &drive);

        let start_time: i64 = Utc::now().timestamp_millis();

        epoch_pool
            .update_start_time(start_time)
            .expect("should update start time");

        drive
            .apply_current_batch(true, Some(&transaction))
            .expect("should apply batch");

        let actual_start_time = epoch_pool
            .get_start_time(Some(&transaction))
            .expect("should get start time");

        assert_eq!(start_time, actual_start_time);
    }

    mod get_start_time {
        #[test]
        fn test_error_if_epoch_pool_is_not_initiated() {
            let drive = super::setup_drive();

            let (transaction, _) = super::setup_fee_pools(&drive, None);

            let non_initiated_epoch_pool = super::EpochPool::new(7000, &drive);

            match non_initiated_epoch_pool.get_start_time(Some(&transaction)) {
                Ok(_) => assert!(
                    false,
                    "should not be able to get start time on uninit epoch pool"
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

            match epoch_pool.get_start_time(Some(&transaction)) {
                Ok(_) => assert!(false, "must be an error"),
                Err(e) => match e {
                    super::error::Error::GroveDB(_) => assert!(true),
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
                    super::constants::KEY_START_TIME.as_bytes(),
                    super::Element::empty_tree(),
                    Some(&transaction),
                )
                .unwrap()
                .expect("should insert invalid data");

            match epoch_pool.get_start_time(Some(&transaction)) {
                Ok(_) => assert!(false, "must be an error"),
                Err(e) => match e {
                    super::error::Error::Fee(super::FeeError::CorruptedStartTimeNotItem()) => {
                        assert!(true)
                    }
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
                    super::constants::KEY_START_TIME.as_bytes(),
                    super::Element::Item(u128::MAX.to_le_bytes().to_vec(), None),
                    Some(&transaction),
                )
                .unwrap()
                .expect("should insert invalid data");

            match epoch_pool.get_start_time(Some(&transaction)) {
                Ok(_) => assert!(false, "must be an error"),
                Err(e) => match e {
                    super::error::Error::Fee(super::FeeError::CorruptedStartTimeLength()) => {
                        assert!(true)
                    }
                    _ => assert!(false, "invalid error type"),
                },
            }
        }
    }

    #[test]
    fn test_update_start_block_height() {
        let drive = setup_drive();

        let (transaction, _) = setup_fee_pools(&drive, None);

        let epoch_pool = EpochPool::new(0, &drive);

        let start_block_height = 1;

        epoch_pool
            .update_start_block_height(start_block_height)
            .expect("should update start block height");

        drive
            .apply_current_batch(true, Some(&transaction))
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
                    "should not be able to get start block height on uninit epoch pool"
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
                    super::constants::KEY_START_BLOCK_HEIGHT.as_bytes(),
                    super::Element::Item(u128::MAX.to_le_bytes().to_vec(), None),
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
                    super::constants::KEY_START_BLOCK_HEIGHT.as_bytes(),
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

            epoch.init_empty().expect("should init empty pool");

            match drive.apply_current_batch(true, Some(&transaction)) {
                Ok(_) => assert!(false, "should not be able to init epoch without FeePools"),
                Err(e) => match e {
                    super::error::Error::GroveDB(grovedb::Error::InvalidPath(_)) => assert!(true),
                    _ => assert!(false, "invalid error type"),
                },
            }
        }

        #[test]
        fn test_values_are_set() {
            let drive = super::setup_drive();
            let (transaction, _) = super::setup_fee_pools(&drive, None);

            let epoch = super::EpochPool::new(1042, &drive);

            epoch.init_empty().expect("should init an epoch pool");

            drive
                .apply_current_batch(true, Some(&transaction))
                .expect("should apply batch");

            let storage_fee = epoch
                .get_storage_fee(Some(&transaction))
                .expect("should get storage fee");

            assert_eq!(storage_fee, super::dec!(0.0));
        }
    }

    mod init_current {
        #[test]
        fn test_values_are_set() {
            let drive = super::setup_drive();
            let (transaction, _) = super::setup_fee_pools(&drive, None);

            let epoch = super::EpochPool::new(1042, &drive);

            let multiplier = 42;
            let start_time = 1;
            let start_block_height = 2;

            epoch.init_empty().expect("should init empty epoch pool");

            epoch
                .init_current(multiplier, start_block_height, start_time)
                .expect("should init an epoch pool");

            drive
                .apply_current_batch(true, Some(&transaction))
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
        #[test]
        fn test_values_are_deleted() {
            let drive = super::setup_drive();
            let (transaction, _) = super::setup_fee_pools(&drive, None);

            let epoch = super::EpochPool::new(0, &drive);

            epoch
                .init_current(1, 2, 3)
                .expect("should init an epoch pool");

            // Apply init current
            drive
                .apply_current_batch(true, Some(&transaction))
                .expect("should apply batch");

            epoch
                .mark_as_paid(Some(&transaction))
                .expect("should mark epoch as paid");

            drive
                .apply_current_batch(true, Some(&transaction))
                .expect("should apply batch");

            match drive
                .grove
                .get(
                    epoch.get_path(),
                    super::constants::KEY_PROPOSERS.as_bytes(),
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
