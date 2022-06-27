use grovedb::{Element, TransactionArg};

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

    pub fn init_empty(&self, transaction: TransactionArg) -> Result<(), Error> {
        // init epoch tree
        self.drive
            .grove
            .insert(
                FeePools::get_path(),
                &self.key,
                Element::empty_tree(),
                transaction,
            )
            .map_err(Error::GroveDB)?;

        // init storage fee item to 0
        self.update_storage_fee(0i64, transaction)?;

        Ok(())
    }

    pub fn init_current(
        &self,
        multiplier: u64,
        start_block_height: u64,
        start_time: i64,
        transaction: TransactionArg,
    ) -> Result<(), Error> {
        self.update_start_block_height(start_block_height, transaction)?;

        self.update_processing_fee(0u64, transaction)?;

        self.init_proposers(transaction)?;

        self.update_fee_multiplier(multiplier, transaction)?;

        self.update_start_time(start_time, transaction)?;

        Ok(())
    }

    pub fn mark_as_paid(&self, transaction: TransactionArg) -> Result<(), Error> {
        self.delete_proposers(transaction)?;

        self.delete_storage_fee(transaction)?;

        self.delete_processing_fee(transaction)?;

        Ok(())
    }

    pub fn get_path(&self) -> [&[u8]; 2] {
        [FeePools::get_path()[0], &self.key]
    }

    pub fn get_proposers_path(&self) -> [&[u8]; 3] {
        [
            FeePools::get_path()[0],
            &self.key,
            constants::KEY_PROPOSERS.as_bytes(),
        ]
    }

    pub fn update_start_time(&self, time: i64, transaction: TransactionArg) -> Result<(), Error> {
        self.drive
            .grove
            .insert(
                self.get_path(),
                constants::KEY_START_TIME.as_bytes(),
                Element::Item(time.to_le_bytes().to_vec(), None),
                transaction,
            )
            .map_err(Error::GroveDB)?;

        Ok(())
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
            .map_err(Error::GroveDB)?;

        if let Element::Item(item, _) = element {
            Ok(u64::from_le_bytes(item.as_slice().try_into().map_err(
                |_| Error::Fee(FeeError::CorruptedStartBlockHeightItemLength()),
            )?))
        } else {
            Err(Error::Fee(FeeError::CorruptedStartBlockHeightNotItem()))
        }
    }

    pub fn update_start_block_height(
        &self,
        start_block_height: u64,
        transaction: TransactionArg,
    ) -> Result<(), Error> {
        self.drive
            .grove
            .insert(
                self.get_path(),
                constants::KEY_START_BLOCK_HEIGHT.as_bytes(),
                Element::Item(start_block_height.to_le_bytes().to_vec(), None),
                transaction,
            )
            .map_err(Error::GroveDB)
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use grovedb::Element;
    use tempfile::TempDir;

    use crate::error::fee::FeeError;
    use crate::fee::pools::epoch::constants;
    use crate::fee::pools::tests::helpers::setup::{setup_drive, setup_fee_pools};
    use crate::{drive::Drive, error, fee::pools::fee_pools::FeePools};

    use super::EpochPool;

    #[test]
    fn test_update_start_time() {
        let drive = setup_drive();

        let (transaction, fee_pools) = setup_fee_pools(&drive, None);

        let epoch_pool = super::EpochPool::new(0, &drive);

        let start_time: i64 = Utc::now().timestamp_millis();

        epoch_pool
            .update_start_time(start_time, Some(&transaction))
            .expect("should update start time");

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
                .expect("to insert invalid data");

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
                .expect("to insert invalid data");

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
            .update_start_block_height(start_block_height, Some(&transaction))
            .expect("should update start block height");

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
                .expect("to insert invalid data");

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
                .expect("to insert invalid data");

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

    #[test]
    fn test_init_empty() {
        todo!();

        let tmp_dir = TempDir::new().unwrap();
        let drive: Drive = Drive::open(tmp_dir).expect("expected to open Drive successfully");

        drive
            .create_root_tree(None)
            .expect("expected to create root tree successfully");

        let transaction = drive.grove.start_transaction();

        let epoch = EpochPool::new(1042, &drive);

        match epoch.init_empty(Some(&transaction)) {
            Ok(_) => assert!(false, "should not be able to init epoch without FeePools"),
            Err(e) => match e {
                error::Error::GroveDB(grovedb::Error::InvalidPath(_)) => assert!(true),
                _ => assert!(false, "ivalid error type"),
            },
        }

        let fee_pools = FeePools::new();

        fee_pools
            .init(&drive, Some(&transaction))
            .expect("fee pools to init");

        let epoch = EpochPool::new(1042, &drive);

        let multiplier = 42;

        epoch
            .init_empty(Some(&transaction))
            .expect("to init an epoch pool");

        let storage_fee = epoch
            .get_storage_fee(Some(&transaction))
            .expect("to get storage fee");

        assert_eq!(storage_fee, 0);

        let stored_multiplier = epoch
            .get_fee_multiplier(Some(&transaction))
            .expect("to get multiplier");

        assert_eq!(stored_multiplier, multiplier);
    }

    #[test]
    fn test_init_current() {
        todo!()
    }

    #[test]
    fn test_mark_as_paid() {
        todo!();

        let tmp_dir = TempDir::new().unwrap();
        let drive: Drive = Drive::open(tmp_dir).expect("expected to open Drive successfully");

        drive
            .create_root_tree(None)
            .expect("expected to create root tree successfully");

        let transaction = drive.grove.start_transaction();

        let fee_pools = FeePools::new();

        fee_pools
            .init(&drive, Some(&transaction))
            .expect("fee pools to init");

        let uninit_epoch_pool = EpochPool::new(7000, &drive);

        match uninit_epoch_pool.mark_as_paid(Some(&transaction)) {
            Ok(_) => assert!(false, "should not be able to delete uninit pool"),
            Err(e) => match e {
                error::Error::GroveDB(grovedb::Error::PathKeyNotFound(_)) => assert!(true),
                _ => assert!(false, "error type is wrong"),
            },
        }

        let epoch = EpochPool::new(42, &drive);

        epoch
            .mark_as_paid(Some(&transaction))
            .expect("to delete 42th epoch");

        match drive
            .grove
            .get(FeePools::get_path(), &epoch.key, Some(&transaction))
        {
            Ok(_) => assert!(false, "should not be able to get deleted epoch pool"),
            Err(e) => match e {
                grovedb::Error::PathKeyNotFound(_) => assert!(true),
                _ => assert!(false, "error should be of type PathKeyNotFound"),
            },
        }
    }
}
