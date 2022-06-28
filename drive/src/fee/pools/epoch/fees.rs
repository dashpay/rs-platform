use grovedb::{Element, TransactionArg};
use rust_decimal::Decimal;
use std::str::FromStr;

use crate::{
    error::{fee::FeeError, Error},
    fee::pools::epoch::epoch_pool::EpochPool,
};

use super::constants;

impl<'e> EpochPool<'e> {
    pub fn get_storage_fee(&self, transaction: TransactionArg) -> Result<Decimal, Error> {
        let element = self
            .drive
            .grove
            .get(
                self.get_path(),
                constants::KEY_STORAGE_FEE.as_bytes(),
                transaction,
            )
            .map_err(Error::GroveDB)?;

        if let Element::Item(item, _) = element {
            Ok(Decimal::deserialize(item.try_into().map_err(|_| {
                Error::Fee(FeeError::CorruptedStorageFeeInvalidItemLength(
                    "epoch storage fee item have an invalid length",
                ))
            })?))
        } else {
            Err(Error::Fee(FeeError::CorruptedStorageFeeNotItem(
                "epoch storage fee must be an item",
            )))
        }
    }

    pub fn get_processing_fee(&self, transaction: TransactionArg) -> Result<u64, Error> {
        let element = self
            .drive
            .grove
            .get(
                self.get_path(),
                constants::KEY_PROCESSING_FEE.as_bytes(),
                transaction,
            )
            .map_err(Error::GroveDB)?;

        if let Element::Item(item, _) = element {
            Ok(u64::from_le_bytes(item.as_slice().try_into().map_err(
                |_| {
                    Error::Fee(FeeError::CorruptedProcessingFeeInvalidItemLength(
                        "epoch processing fee is not u64",
                    ))
                },
            )?))
        } else {
            Err(Error::Fee(FeeError::CorruptedProcessingFeeNotItem(
                "epoch processing fee must be an item",
            )))
        }
    }

    pub fn get_fee_multiplier(&self, transaction: TransactionArg) -> Result<u64, Error> {
        let element = self
            .drive
            .grove
            .get(
                self.get_path(),
                constants::KEY_FEE_MULTIPLIER.as_bytes(),
                transaction,
            )
            .map_err(Error::GroveDB)?;

        if let Element::Item(item, _) = element {
            Ok(u64::from_le_bytes(item.as_slice().try_into().map_err(
                |_| {
                    Error::Fee(FeeError::CorruptedMultiplierInvalidItemLength(
                        "epoch multiplier item have an invalid length",
                    ))
                },
            )?))
        } else {
            Err(Error::Fee(FeeError::CorruptedMultiplierNotItem(
                "epoch multiplier must be an item",
            )))
        }
    }

    pub fn update_fee_multiplier(
        &self,
        multiplier: u64,
        transaction: TransactionArg,
    ) -> Result<(), Error> {
        // setting up multiplier
        self.drive
            .grove
            .insert(
                self.get_path(),
                constants::KEY_FEE_MULTIPLIER.as_bytes(),
                Element::Item(multiplier.to_le_bytes().to_vec(), None),
                transaction,
            )
            .map_err(Error::GroveDB)?;

        Ok(())
    }

    pub fn update_processing_fee(
        &self,
        processing_fee: u64,
        transaction: TransactionArg,
    ) -> Result<(), Error> {
        self.drive
            .grove
            .insert(
                self.get_path(),
                constants::KEY_PROCESSING_FEE.as_bytes(),
                Element::Item(processing_fee.to_le_bytes().to_vec(), None),
                transaction,
            )
            .map_err(Error::GroveDB)?;

        Ok(())
    }

    pub fn delete_processing_fee(&self, transaction: TransactionArg) -> Result<(), Error> {
        self.drive
            .grove
            .delete(
                self.get_path(),
                constants::KEY_PROCESSING_FEE.as_bytes(),
                transaction,
            )
            .map_err(Error::GroveDB)?;

        Ok(())
    }

    pub fn update_storage_fee(
        &self,
        storage_fee: Decimal,
        transaction: TransactionArg,
    ) -> Result<(), Error> {
        self.drive
            .grove
            .insert(
                self.get_path(),
                constants::KEY_STORAGE_FEE.as_bytes(),
                Element::Item(storage_fee.serialize().to_vec(), None),
                transaction,
            )
            .map_err(Error::GroveDB)?;

        Ok(())
    }

    pub fn delete_storage_fee(&self, transaction: TransactionArg) -> Result<(), Error> {
        self.drive
            .grove
            .delete(
                self.get_path(),
                constants::KEY_STORAGE_FEE.as_bytes(),
                transaction,
            )
            .map_err(Error::GroveDB)?;

        Ok(())
    }

    pub fn get_total_fees(&self, transaction: TransactionArg) -> Result<Decimal, Error> {
        let storage_fee = self.get_storage_fee(transaction)?;

        let processing_fee = self.get_processing_fee(transaction)?;

        let processing_fee =
            Decimal::from_str(processing_fee.to_string().as_str()).map_err(|_| {
                Error::Fee(FeeError::DecimalConversion(
                    "can't convert processing_fee to Decimal",
                ))
            })?;

        Ok(storage_fee + processing_fee)
    }
}

#[cfg(test)]
mod tests {
    use grovedb::Element;
    use rust_decimal::Decimal;
    use rust_decimal_macros::dec;
    use tempfile::TempDir;

    use crate::{
        drive::Drive,
        error::{self, fee::FeeError, Error},
        fee::pools::{
            epoch::{constants, epoch_pool::EpochPool},
            fee_pools::FeePools,
            tests::helpers::setup::{setup_drive, setup_fee_pools, SetupFeePoolsOptions},
        },
    };

    #[test]
    fn test_update_and_get_storage_fee() {
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

        let epoch = EpochPool::new(7000, &drive);

        match epoch.get_storage_fee(Some(&transaction)) {
            Ok(_) => assert!(
                false,
                "should not be able to get storage fee on uninit epoch pool"
            ),
            Err(e) => match e {
                error::Error::GroveDB(grovedb::Error::PathNotFound(_)) => assert!(true),
                _ => assert!(false, "invalid error type"),
            },
        }

        match epoch.update_storage_fee(dec!(42.0), Some(&transaction)) {
            Ok(_) => assert!(
                false,
                "should not be able to update storage fee on uninit epoch pool"
            ),
            Err(e) => match e {
                error::Error::GroveDB(grovedb::Error::InvalidPath(_)) => assert!(true),
                _ => assert!(false, "invalid error type"),
            },
        }

        let epoch = EpochPool::new(0, &drive);

        let stored_storage_fee = epoch
            .get_storage_fee(Some(&transaction))
            .expect("to get storage fee");

        assert_eq!(stored_storage_fee, dec!(0.0));

        let storage_fee = dec!(42.0);

        epoch
            .update_storage_fee(storage_fee, Some(&transaction))
            .expect("to update storage fee");

        let stored_storage_fee = epoch
            .get_storage_fee(Some(&transaction))
            .expect("to get storage fee");

        assert_eq!(stored_storage_fee, storage_fee);

        drive
            .grove
            .insert(
                epoch.get_path(),
                constants::KEY_STORAGE_FEE.as_bytes(),
                Element::Item(u128::MAX.to_le_bytes().to_vec(), None),
                Some(&transaction),
            )
            .expect("to insert invalid data");

        match epoch.get_storage_fee(Some(&transaction)) {
            Ok(_) => assert!(false, "should not be able to decode stored value"),
            Err(e) => match e {
                error::Error::Fee(FeeError::CorruptedStorageFeeInvalidItemLength(_)) => {
                    assert!(true)
                }
                _ => assert!(false, "ivalid error type"),
            },
        }
    }

    #[test]
    fn test_update_and_get_processing_fee() {
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

        let epoch = EpochPool::new(7000, &drive);

        match epoch.update_processing_fee(42, Some(&transaction)) {
            Ok(_) => assert!(
                false,
                "should not be able to update processing fee on uninit epoch pool"
            ),
            Err(e) => match e {
                error::Error::GroveDB(grovedb::Error::InvalidPath(_)) => assert!(true),
                _ => assert!(false, "invalid error type"),
            },
        }

        let epoch = EpochPool::new(0, &drive);

        if let Err(e) = epoch.get_processing_fee(Some(&transaction)) {
            match e {
                Error::GroveDB(grovedb::Error::PathKeyNotFound(_)) => {}
                _ => assert!(false, "invalid error type"),
            }
        } else {
            assert!(false, "processing fee is not set yet");
        }

        let processing_fee: u64 = 42;

        epoch
            .update_processing_fee(processing_fee, Some(&transaction))
            .expect("to update processing fee");

        let stored_processing_fee = epoch
            .get_processing_fee(Some(&transaction))
            .expect("to get processing fee");

        assert_eq!(stored_processing_fee, processing_fee);

        drive
            .grove
            .insert(
                epoch.get_path(),
                constants::KEY_PROCESSING_FEE.as_bytes(),
                Element::Item(u128::MAX.to_le_bytes().to_vec(), None),
                Some(&transaction),
            )
            .expect("to insert invalid data");

        match epoch.get_processing_fee(Some(&transaction)) {
            Ok(_) => assert!(false, "should not be able to decode stored value"),
            Err(e) => match e {
                error::Error::Fee(FeeError::CorruptedProcessingFeeInvalidItemLength(_)) => {
                    assert!(true)
                }
                _ => assert!(false, "ivalid error type"),
            },
        }
    }

    #[test]
    fn test_get_total_fees() {
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

        let processing_fee: u64 = 42;
        let storage_fee = dec!(1000);

        let epoch = EpochPool::new(0, &drive);

        epoch
            .update_processing_fee(processing_fee, Some(&transaction))
            .expect("to update processing fee");

        epoch
            .update_storage_fee(storage_fee, Some(&transaction))
            .expect("to update storage fee");

        let combined_fee = epoch
            .get_total_fees(Some(&transaction))
            .expect("to get combined fee");

        assert_eq!(
            combined_fee,
            Decimal::new(processing_fee as i64, 0) + storage_fee
        );
    }

    #[test]
    fn test_update_and_get_fee_multiplier() {
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

        let epoch = EpochPool::new(7000, &drive);

        match epoch.get_fee_multiplier(Some(&transaction)) {
            Ok(_) => assert!(
                false,
                "should not be able to get multiplier on uninit epoch pool"
            ),
            Err(e) => match e {
                error::Error::GroveDB(grovedb::Error::InvalidPath(_)) => assert!(true),
                _ => assert!(false, "invalid error type"),
            },
        }

        let epoch = EpochPool::new(0, &drive);

        let stored_multiplier = epoch
            .get_fee_multiplier(Some(&transaction))
            .expect("to get multiplier");

        assert_eq!(stored_multiplier, 1);

        drive
            .grove
            .insert(
                epoch.get_path(),
                constants::KEY_FEE_MULTIPLIER.as_bytes(),
                Element::Item(u128::MAX.to_le_bytes().to_vec(), None),
                Some(&transaction),
            )
            .expect("to insert invalid data");

        match epoch.get_fee_multiplier(Some(&transaction)) {
            Ok(_) => assert!(false, "should not be able to decode stored value"),
            Err(e) => match e {
                error::Error::Fee(FeeError::CorruptedMultiplierInvalidItemLength(_)) => {
                    assert!(true)
                }
                _ => assert!(false, "ivalid error type"),
            },
        }
    }

    mod fee_multiplier {
        #[test]
        fn test_error_if_epoch_pool_is_not_initiated() {
            let drive = super::setup_drive();
            let (transaction, _) = super::setup_fee_pools(&drive, None);

            let epoch = super::EpochPool::new(7000, &drive);

            match epoch.get_fee_multiplier(Some(&transaction)) {
                Ok(_) => assert!(
                    false,
                    "should not be able to get multiplier on uninit epoch pool"
                ),
                Err(e) => match e {
                    super::error::Error::GroveDB(grovedb::Error::InvalidPath(_)) => assert!(true),
                    _ => assert!(false, "invalid error type"),
                },
            }
        }

        #[test]
        fn test_error_if_value_has_invalid_length() {
            let drive = super::setup_drive();
            let (transaction, _) = super::setup_fee_pools(&drive, None);

            let epoch = super::EpochPool::new(0, &drive);

            drive
                .grove
                .insert(
                    epoch.get_path(),
                    super::constants::KEY_FEE_MULTIPLIER.as_bytes(),
                    super::Element::Item(u128::MAX.to_le_bytes().to_vec(), None),
                    Some(&transaction),
                )
                .expect("to insert invalid data");

            match epoch.get_fee_multiplier(Some(&transaction)) {
                Ok(_) => assert!(false, "should not be able to decode stored value"),
                Err(e) => match e {
                    super::error::Error::Fee(
                        super::FeeError::CorruptedMultiplierInvalidItemLength(_),
                    ) => {
                        assert!(true)
                    }
                    _ => assert!(false, "ivalid error type"),
                },
            }
        }

        #[test]
        fn test_value_is_set() {
            let drive = super::setup_drive();
            let (transaction, _) = super::setup_fee_pools(&drive, None);

            let epoch = super::EpochPool::new(0, &drive);

            let multiplier = 42;
            epoch.init_empty(Some(&transaction)).expect("to init empty");
            epoch
                .init_current(multiplier, 1, 1, Some(&transaction))
                .expect("to init current");

            let stored_multiplier = epoch
                .get_fee_multiplier(Some(&transaction))
                .expect("to get multiplier");

            assert_eq!(stored_multiplier, multiplier);
        }
    }

    mod overflow {
        use std::str::FromStr;

        use rust_decimal::Decimal;

        #[test]
        fn test_u64_fee_conversion() {
            let processing_fee = u64::MAX;

            let decimal = Decimal::from_str(processing_fee.to_string().as_str())
                .expect("to convert u64::MAX to Decimal");

            let converted_to_u64: u64 = decimal
                .try_into()
                .expect("to convert Decimal back to u64::MAX");

            assert_eq!(processing_fee, converted_to_u64);
        }
    }
}
