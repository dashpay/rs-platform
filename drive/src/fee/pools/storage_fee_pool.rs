use grovedb::{Element, TransactionArg};

use crate::error::drive::DriveError;
use crate::error::Error;
use crate::fee::pools::fee_pools::FeePools;

use super::constants;

impl<'f> FeePools<'f> {
    pub fn update_storage_fee_pool(
        &self,
        storage_fee: f64,
        transaction: TransactionArg,
    ) -> Result<(), Error> {
        let element = self
            .drive
            .grove
            .get(
                FeePools::get_path(),
                constants::KEY_STORAGE_FEE_POOL.as_bytes(),
                transaction,
            )
            .map_err(Error::GroveDB)?;

        if let Element::Item(item) = element {
            let value =
                f64::from_le_bytes(item.as_slice().try_into().expect("invalid item length"));

            // in case credit is set update it
            self.drive
                .grove
                .insert(
                    FeePools::get_path(),
                    constants::KEY_STORAGE_FEE_POOL.as_bytes(),
                    Element::Item((value + storage_fee).to_le_bytes().to_vec()),
                    transaction,
                )
                .map_err(Error::GroveDB)?;

            Ok(())
        } else {
            Err(Error::Drive(DriveError::CorruptedEpochElement(
                "fee pools storage fee pool must be an item",
            )))
        }
    }

    pub fn get_storage_fee_pool(&self, transaction: TransactionArg) -> Result<f64, Error> {
        let element = self
            .drive
            .grove
            .get(
                FeePools::get_path(),
                constants::KEY_STORAGE_FEE_POOL.as_bytes(),
                transaction,
            )
            .map_err(Error::GroveDB)?;

        if let Element::Item(item) = element {
            let credit =
                f64::from_le_bytes(item.as_slice().try_into().expect("invalid item length"));

            Ok(credit)
        } else {
            Err(Error::Drive(DriveError::CorruptedEpochElement(
                "fee pools storage fee pool must be an item",
            )))
        }
    }
}
