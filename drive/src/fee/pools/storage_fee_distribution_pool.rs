use crate::drive::Drive;
use grovedb::{Element, TransactionArg};

use crate::error::fee::FeeError;
use crate::error::Error;
use crate::fee::pools::epoch::epoch_pool::EpochPool;
use crate::fee::pools::fee_pools::FeePools;

use super::constants;

pub struct StorageFeeDistributionPool {}

impl StorageFeeDistributionPool {
    fn get_fee_distribution_percent(epoch_index: u16, start_index: u16) -> f64 {
        let reset_epoch_index = epoch_index - start_index;

        let epoch_year = (reset_epoch_index as f64 / 20.0).trunc() as usize;

        constants::FEE_DISTRIBUTION_TABLE[epoch_year]
    }

    pub fn distribute(
        &self,
        drive: &Drive,
        epoch_index: u16,
        transaction: TransactionArg,
    ) -> Result<(), Error> {
        let mut storage_distribution_fees = self.value(drive, transaction)? as f64;

        if storage_distribution_fees == 0.0 {
            return Ok(());
        }

        let mut leftovers = 0.0;

        for index in epoch_index..epoch_index + 1000 {
            let epoch_pool = EpochPool::new(index, drive);

            let distribution_percent = Self::get_fee_distribution_percent(index, epoch_index);

            let fee_share = storage_distribution_fees * distribution_percent;

            let storage_fee = epoch_pool.get_storage_fee(transaction)? as f64;

            // store leftovers of the floor operation
            // and then if it is higher then 0 distribute it to current epoch pool
            leftovers += fee_share - fee_share.floor();

            let storage_fee = if leftovers.floor() > 0.0 {
                let leftovers_floored = leftovers.floor();

                leftovers -= leftovers_floored;

                (storage_fee + fee_share + leftovers_floored).floor() as i64
            } else {
                (storage_fee + fee_share).floor() as i64
            };

            epoch_pool.update_storage_fee(storage_fee, transaction)?;

            storage_distribution_fees -= fee_share;
        }

        self.update(drive, storage_distribution_fees.floor() as i64, transaction)
    }

    pub fn update(
        &self,
        drive: &Drive,
        storage_fee: i64,
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

    pub fn value(&self, drive: &Drive, transaction: TransactionArg) -> Result<i64, Error> {
        let element = drive
            .grove
            .get(
                FeePools::get_path(),
                constants::KEY_STORAGE_FEE_POOL.as_bytes(),
                transaction,
            )
            .map_err(Error::GroveDB)?;

        if let Element::Item(item, _) = element {
            let fee = i64::from_le_bytes(item.as_slice().try_into().map_err(|_| {
                Error::Fee(FeeError::CorruptedStorageFeePoolInvalidItemLength(
                    "fee pools storage fee pool is not i64",
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
    fn test_distribute_storage_distribution_pool() {
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

        let storage_pool = 1000;
        let epoch_index = 42;

        // init additional epoch pools as it will be done in epoch_change
        for i in 1000..=1000 + epoch_index {
            let epoch = EpochPool::new(i, &drive);
            epoch
                .init_empty(Some(&transaction))
                .expect("to init additional epoch pool");
        }

        fee_pools
            .storage_fee_distribution_pool
            .update(&drive, storage_pool, Some(&transaction))
            .expect("to update storage fee pool");

        fee_pools
            .storage_fee_distribution_pool
            .distribute(&drive, epoch_index, Some(&transaction))
            .expect("to distribute storage fee pool");

        // check leftover
        let leftover_storage_fee_pool = fee_pools
            .storage_fee_distribution_pool
            .value(&drive, Some(&transaction))
            .expect("to get storage fee pool");

        assert_eq!(leftover_storage_fee_pool, 0);

        // collect all the storage fee values of the 1000 epoch pools
        let storage_fees: Vec<i64> = (epoch_index..epoch_index + 1000)
            .map(|index| {
                let epoch_pool = EpochPool::new(index, &drive);
                epoch_pool
                    .get_storage_fee(Some(&transaction))
                    .expect("to get storage fee")
            })
            .collect();

        // compre them with reference table
        let reference_fees = [
            50, 47, 45, 43, 41, 38, 37, 35, 33, 32, 30, 28, 27, 26, 24, 23, 22, 21, 20, 19, 17, 17,
            15, 15, 14, 14, 12, 13, 11, 11, 11, 10, 9, 9, 9, 8, 8, 8, 7, 6, 7, 6, 5, 5, 6, 4, 5, 5,
            4, 4, 4, 3, 4, 3, 3, 3, 3, 3, 3, 2, 3, 2, 2, 2, 2, 2, 1, 2, 2, 1, 2, 1, 1, 2, 1, 1, 1,
            1, 1, 1, 1, 1, 1, 1, 0, 1, 1, 0, 1, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 0, 1, 0, 1, 0, 0, 0,
            1, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ];

        assert_eq!(storage_fees, reference_fees);

        // refill storage fee pool once more
        fee_pools
            .storage_fee_distribution_pool
            .update(&drive, storage_pool, Some(&transaction))
            .expect("to update storage fee pool");

        // distribute fees once more
        fee_pools
            .storage_fee_distribution_pool
            .distribute(&drive, epoch_index, Some(&transaction))
            .expect("to distribute storage fee pool");

        // collect all the storage fee values of the 1000 epoch pools again
        let storage_fees: Vec<i64> = (epoch_index..epoch_index + 1000)
            .map(|index| {
                let epoch_pool = EpochPool::new(index, &drive);
                epoch_pool
                    .get_storage_fee(Some(&transaction))
                    .expect("to get storage fee")
            })
            .collect();

        // assert that all the values doubled meaning that distribution is repoducable
        assert_eq!(
            storage_fees,
            reference_fees
                .iter()
                .map(|val| val * 2)
                .collect::<Vec<i64>>()
        );
    }

    #[test]
    fn test_update_and_get_storage_fee_pool() {
        let tmp_dir = TempDir::new().unwrap();
        let drive: Drive = Drive::open(tmp_dir).expect("expected to open Drive successfully");

        drive
            .create_root_tree(None)
            .expect("expected to create root tree successfully");

        let transaction = drive.grove.start_transaction();

        let storage_fee: i64 = 42;

        let fee_pools = FeePools::new();

        match fee_pools
            .storage_fee_distribution_pool
            .value(&drive, Some(&transaction))
        {
            Ok(_) => assert!(
                false,
                "should not be able to get genesis time on uninit fee pools"
            ),
            Err(e) => match e {
                error::Error::GroveDB(grovedb::Error::PathNotFound(_)) => assert!(true),
                _ => assert!(false, "invalid error type"),
            },
        }

        match fee_pools.storage_fee_distribution_pool.update(
            &drive,
            storage_fee,
            Some(&transaction),
        ) {
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
            .storage_fee_distribution_pool
            .update(&drive, storage_fee, Some(&transaction))
            .expect("to update storage fee pool");

        let stored_storage_fee = fee_pools
            .storage_fee_distribution_pool
            .value(&drive, Some(&transaction))
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

        match fee_pools
            .storage_fee_distribution_pool
            .value(&drive, Some(&transaction))
        {
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
