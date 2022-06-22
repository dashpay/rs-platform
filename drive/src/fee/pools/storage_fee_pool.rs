use crate::drive::Drive;
use grovedb::{Element, TransactionArg};

use crate::error::fee::FeeError;
use crate::error::Error;
use crate::fee::pools::epoch::epoch_pool::EpochPool;
use crate::fee::pools::fee_pools::FeePools;

use super::constants;

fn get_fee_distribution_percent(epoch_index: u16, start_index: u16) -> f64 {
    let reset_epoch_index = epoch_index - start_index;

    let epoch_year = (reset_epoch_index as f64 / 20.0).trunc() as usize;

    constants::FEE_DISTRIBUTION_TABLE[epoch_year]
}

impl FeePools {
    pub fn distribute_storage_fee_pool(
        &self,
        drive: &Drive,
        epoch_index: u16,
        transaction: TransactionArg,
    ) -> Result<(), Error> {
        let mut fee_pool_value = self.get_storage_fee_pool(drive, transaction)?;

        // todo!("do nothing if empty, it's actually the case for epoch 0");

        for index in epoch_index..epoch_index + 1000 {
            let epoch_pool = EpochPool::new(index, drive);

            let distribution_percent = get_fee_distribution_percent(index, epoch_index);

            let fee_share = fee_pool_value * distribution_percent;

            let storage_fee = epoch_pool.get_storage_fee(transaction)?;

            epoch_pool.update_storage_fee(storage_fee + fee_share, transaction)?;

            fee_pool_value -= fee_share;
        }

        self.update_storage_fee_pool(drive, fee_pool_value, transaction)
    }

    pub fn update_storage_fee_pool(
        &self,
        drive: &Drive,
        storage_fee: f64,
        transaction: TransactionArg,
    ) -> Result<(), Error> {
        drive
            .grove
            .insert(
                FeePools::get_path(),
                constants::KEY_STORAGE_FEE_POOL.as_bytes(),
                Element::Item(storage_fee.to_le_bytes().to_vec(), None),
                transaction,
            )
            .map_err(Error::GroveDB)
    }

    pub fn get_storage_fee_pool(
        &self,
        drive: &Drive,
        transaction: TransactionArg,
    ) -> Result<f64, Error> {
        let element = drive
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
    use grovedb::Element;
    use tempfile::TempDir;

    use crate::fee::pools::epoch::epoch_pool::EpochPool;
    use crate::{
        drive::Drive,
        error::{self, fee::FeeError},
        fee::pools::{constants, fee_pools::FeePools},
    };

    #[test]
    fn test_fee_pools_distribute_storage_distribution_pool() {
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

        let storage_pool = 1000.0;
        let epoch_index = 42;

        // init additional epoch pools as it will be done in epoch_change
        for i in 1000..=1000 + epoch_index {
            let epoch = EpochPool::new(i, &drive);
            epoch
                .init_empty(Some(&transaction))
                .expect("to init additional epoch pool");
        }

        fee_pools
            .update_storage_fee_pool(&drive, storage_pool, Some(&transaction))
            .expect("to update storage fee pool");

        fee_pools
            .distribute_storage_fee_pool(&drive, epoch_index, Some(&transaction))
            .expect("to distribute storage fee pool");

        // check leftover
        let leftover_storage_fee_pool = fee_pools
            .get_storage_fee_pool(&drive, Some(&transaction))
            .expect("to get storage fee pool");

        assert_eq!(leftover_storage_fee_pool, 1.5260017107721069e-6);

        todo!("I guess it must be 0");

        // selectively check 1st and last item
        let first_epoch = EpochPool::new(epoch_index, &drive);

        let first_epoch_storage_fee = first_epoch
            .get_storage_fee(Some(&transaction))
            .expect("to get storage fee");

        assert_eq!(first_epoch_storage_fee, 50.0);

        let last_epoch = EpochPool::new(epoch_index + 999, &drive);

        let last_epoch_storage_fee = last_epoch
            .get_storage_fee(Some(&transaction))
            .expect("to get storage fee");

        assert_eq!(last_epoch_storage_fee, 1.909889563258572e-9);
    }

    #[test]
    fn test_fee_pools_update_and_get_storage_fee_pool() {
        let tmp_dir = TempDir::new().unwrap();
        let drive: Drive = Drive::open(tmp_dir).expect("expected to open Drive successfully");

        drive
            .create_root_tree(None)
            .expect("expected to create root tree successfully");

        let transaction = drive.grove.start_transaction();

        let storage_fee: f64 = 0.42;

        let fee_pools = FeePools::new();

        match fee_pools.get_storage_fee_pool(&drive, Some(&transaction)) {
            Ok(_) => assert!(
                false,
                "should not be able to get genesis time on uninit fee pools"
            ),
            Err(e) => match e {
                error::Error::GroveDB(grovedb::Error::PathNotFound(_)) => assert!(true),
                _ => assert!(false, "invalid error type"),
            },
        }

        match fee_pools.update_storage_fee_pool(&drive, storage_fee, Some(&transaction)) {
            Ok(_) => assert!(
                false,
                "should not be able to update genesis time on uninit fee pools"
            ),
            Err(e) => match e {
                error::Error::GroveDB(grovedb::Error::InvalidPath(_)) => assert!(true),
                _ => assert!(false, "invalid error type"),
            },
        }

        fee_pools
            .init(&drive, Some(&transaction))
            .expect("fee pools to init");

        fee_pools
            .update_storage_fee_pool(&drive, storage_fee, Some(&transaction))
            .expect("to update storage fee pool");

        let stored_storage_fee = fee_pools
            .get_storage_fee_pool(&drive, Some(&transaction))
            .expect("to get storage fee pool");

        assert_eq!(storage_fee, stored_storage_fee);

        drive
            .grove
            .insert(
                FeePools::get_path(),
                constants::KEY_STORAGE_FEE_POOL.as_bytes(),
                Element::Item(u128::MAX.to_le_bytes().to_vec(), None),
                Some(&transaction),
            )
            .expect("to insert invalid data");

        match fee_pools.get_storage_fee_pool(&drive, Some(&transaction)) {
            Ok(_) => assert!(false, "should not be able to decode stored value"),
            Err(e) => match e {
                error::Error::Fee(FeeError::CorruptedStorageFeePoolInvalidItemLength(_)) => {
                    assert!(true)
                }
                _ => assert!(false, "ivalid error type"),
            },
        }
    }
}
