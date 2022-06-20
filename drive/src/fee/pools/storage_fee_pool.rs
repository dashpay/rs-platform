use grovedb::{Element, TransactionArg};

use crate::error::fee::FeeError;
use crate::error::Error;
use crate::fee::pools::fee_pools::FeePools;

use super::constants;

impl<'f> FeePools<'f> {
    pub fn update_storage_fee_pool(
        &self,
        storage_fee: f64,
        transaction: TransactionArg,
    ) -> Result<(), Error> {
        self.drive
            .grove
            .insert(
                FeePools::get_path(),
                constants::KEY_STORAGE_FEE_POOL.as_bytes(),
                Element::Item(storage_fee.to_le_bytes().to_vec(), None),
                transaction,
            )
            .map_err(Error::GroveDB)
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

        if let Element::Item(item, _) = element {
            let fee = f64::from_le_bytes(item.as_slice().try_into().map_err(|_| {
                Error::Fee(FeeError::CorruptedStorageFeePoolInvalidItemLength(
                    "fee pools storage fee pool item have an invalid length",
                ))
            })?);

            Ok(fee)
        } else {
            Err(Error::Fee(FeeError::CorruptedStorageFeePoolNotItem(
                "fee pools storage fee pool must be an item",
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use crate::{drive::Drive, fee::pools::fee_pools::FeePools};

    #[test]
    fn test_fee_pools_update_and_get_storage_fee_pool() {
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

        let storage_fee: f64 = 0.42;

        fee_pools
            .update_storage_fee_pool(storage_fee, Some(&transaction))
            .expect("to update storage fee pool");

        let stored_storage_fee = fee_pools
            .get_storage_fee_pool(Some(&transaction))
            .expect("to get storage fee pool");

        assert_eq!(storage_fee, stored_storage_fee);
    }
}
