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
            .map_err(Error::GroveDB)?;

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
