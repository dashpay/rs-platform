use grovedb::{Element, TransactionArg};

use crate::{
    error::{fee::FeeError, Error},
    fee::pools::epoch::epoch_pool::EpochPool,
};

use super::constants;

impl<'e> EpochPool<'e> {
    pub fn get_storage_fee(&self, transaction: TransactionArg) -> Result<f64, Error> {
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
            Ok(f64::from_le_bytes(item.as_slice().try_into().map_err(
                |_| {
                    Error::Fee(FeeError::CorruptedStorageFeeInvalidItemLength(
                        "epoch storage fee item have an invalid length",
                    ))
                },
            )?))
        } else {
            Err(Error::Fee(FeeError::CorruptedStorageFeeNotItem(
                "epoch storage fee must be an item",
            )))
        }
    }

    pub fn get_processing_fee(&self, transaction: TransactionArg) -> Result<f64, Error> {
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
            Ok(f64::from_le_bytes(item.as_slice().try_into().map_err(
                |_| {
                    Error::Fee(FeeError::CorruptedProcessingFeeInvalidItemLength(
                        "epoch processing fee item have an invalid length",
                    ))
                },
            )?))
        } else {
            Err(Error::Fee(FeeError::CorruptedProcessingFeeNotItem(
                "epoch processing fee must be an item",
            )))
        }
    }

    pub fn update_processing_fee(
        &self,
        processing_fee: f64,
        transaction: TransactionArg,
    ) -> Result<(), Error> {
        let element = self
            .drive
            .grove
            .get(
                self.get_path(),
                constants::KEY_PROCESSING_FEE.as_bytes(),
                transaction,
            )
            .or_else(|e| match e {
                grovedb::Error::PathKeyNotFound(_) => {
                    Ok(Element::Item(0f64.to_le_bytes().to_vec(), None))
                }
                _ => Err(Error::GroveDB(e)),
            })?;

        if let Element::Item(item, _) = element {
            let fee = f64::from_le_bytes(item.as_slice().try_into().map_err(|_| {
                Error::Fee(FeeError::CorruptedProcessingFeeInvalidItemLength(
                    "epoch processing fee item have an invalid length",
                ))
            })?);

            // in case fee is set updated it
            self.drive
                .grove
                .insert(
                    self.get_path(),
                    constants::KEY_PROCESSING_FEE.as_bytes(),
                    Element::Item((fee + processing_fee).to_le_bytes().to_vec(), None),
                    transaction,
                )
                .map_err(Error::GroveDB)?;

            Ok(())
        } else {
            Err(Error::Fee(FeeError::CorruptedProcessingFeeNotItem(
                "epoch processing fee must be an item",
            )))
        }
    }

    pub fn update_storage_fee(
        &self,
        storage_fee: f64,
        transaction: TransactionArg,
    ) -> Result<(), Error> {
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
            let fee = f64::from_le_bytes(item.as_slice().try_into().map_err(|_| {
                Error::Fee(FeeError::CorruptedStorageFeeInvalidItemLength(
                    "epoch storage fee item have an invalid length",
                ))
            })?);

            // in case fee is set updated it
            self.drive
                .grove
                .insert(
                    self.get_path(),
                    constants::KEY_STORAGE_FEE.as_bytes(),
                    Element::Item((fee + storage_fee).to_le_bytes().to_vec(), None),
                    transaction,
                )
                .map_err(Error::GroveDB)?;

            Ok(())
        } else {
            Err(Error::Fee(FeeError::CorruptedStorageFeeNotItem(
                "epoch storage fee must be an item",
            )))
        }
    }

    pub fn get_combined_fee(&self, transaction: TransactionArg) -> Result<f64, Error> {
        let storage_credit = self.get_storage_fee(transaction)?;

        let processing_credit = self.get_processing_fee(transaction)?;

        Ok(storage_credit + processing_credit)
    }
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use crate::{
        drive::Drive,
        error::Error,
        fee::pools::{epoch::epoch_pool::EpochPool, fee_pools::FeePools},
    };

    #[test]
    fn test_epoch_pool_update_and_get_storage_fee() {
        let tmp_dir = TempDir::new().unwrap();
        let drive: Drive = Drive::open(tmp_dir).expect("expected to open Drive successfully");

        drive
            .create_root_tree(None)
            .expect("expected to create root tree successfully");

        let transaction = drive.grove.start_transaction();

        let fee_pools = FeePools::new(&drive);

        fee_pools
            .init(Some(&transaction))
            .expect("fee pools to init");

        let epoch = EpochPool::new(0, &drive);

        let stored_storage_fee = epoch
            .get_storage_fee(Some(&transaction))
            .expect("to get storage fee");

        assert_eq!(stored_storage_fee, 0f64);

        let storage_fee: f64 = 0.42;

        epoch
            .update_storage_fee(storage_fee, Some(&transaction))
            .expect("to update storage fee");

        let stored_storage_fee = epoch
            .get_storage_fee(Some(&transaction))
            .expect("to get storage fee");

        assert_eq!(stored_storage_fee, storage_fee);
    }

    #[test]
    fn test_epoch_pool_update_and_get_processing_fee() {
        let tmp_dir = TempDir::new().unwrap();
        let drive: Drive = Drive::open(tmp_dir).expect("expected to open Drive successfully");

        drive
            .create_root_tree(None)
            .expect("expected to create root tree successfully");

        let transaction = drive.grove.start_transaction();

        let fee_pools = FeePools::new(&drive);

        fee_pools
            .init(Some(&transaction))
            .expect("fee pools to init");

        let epoch = EpochPool::new(0, &drive);

        if let Err(e) = epoch.get_processing_fee(Some(&transaction)) {
            match e {
                Error::GroveDB(grovedb::Error::PathKeyNotFound(_)) => {}
                _ => assert!(false, "invalid error type"),
            }
        } else {
            assert!(false, "processing fee is not set yet");
        }

        let processing_fee: f64 = 0.42;

        epoch
            .update_processing_fee(processing_fee, Some(&transaction))
            .expect("to update processing fee");

        let stored_processing_fee = epoch
            .get_processing_fee(Some(&transaction))
            .expect("to get processing fee");

        assert_eq!(stored_processing_fee, processing_fee);
    }

    #[test]
    fn test_epoch_pool_get_combined_fee() {
        let tmp_dir = TempDir::new().unwrap();
        let drive: Drive = Drive::open(tmp_dir).expect("expected to open Drive successfully");

        drive
            .create_root_tree(None)
            .expect("expected to create root tree successfully");

        let transaction = drive.grove.start_transaction();

        let fee_pools = FeePools::new(&drive);

        fee_pools
            .init(Some(&transaction))
            .expect("fee pools to init");

        let processing_fee: f64 = 0.42;
        let storage_fee: f64 = 0.05678;

        let epoch = EpochPool::new(0, &drive);

        epoch
            .update_processing_fee(processing_fee, Some(&transaction))
            .expect("to update processing fee");

        epoch
            .update_storage_fee(storage_fee, Some(&transaction))
            .expect("to update storage fee");

        let combined_fee = epoch
            .get_combined_fee(Some(&transaction))
            .expect("to get combined fee");

        assert_eq!(combined_fee, processing_fee + storage_fee);
    }
}
