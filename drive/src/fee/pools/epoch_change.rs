use grovedb::{Element, TransactionArg};

use crate::error::fee::FeeError;
use crate::error::Error;
use crate::fee::pools::fee_pools::FeePools;

use super::constants;
use super::epoch::epoch_pool::EpochPool;

impl<'f> FeePools<'f> {
    pub fn get_genesis_time(&self, transaction: TransactionArg) -> Result<i64, Error> {
        if let Some(genesis_time) = self.genesis_time {
            return Ok(genesis_time);
        }

        let element = self
            .drive
            .grove
            .get(
                FeePools::get_path(),
                constants::KEY_GENESIS_TIME.as_bytes(),
                transaction,
            )
            .map_err(Error::GroveDB)?;

        if let Element::Item(item, _) = element {
            let genesis_time = i64::from_le_bytes(item.as_slice().try_into().map_err(|_| {
                Error::Fee(FeeError::CorruptedGenesisTimeInvalidItemLength(
                    "genesis time item have an invalid length",
                ))
            })?);

            Ok(genesis_time)
        } else {
            Err(Error::Fee(FeeError::CorruptedGenesisTimeNotItem(
                "fee pool genesis time must be an item",
            )))
        }
    }

    pub fn update_genesis_time(
        &mut self,
        genesis_time: i64,
        transaction: TransactionArg,
    ) -> Result<(), Error> {
        self.drive
            .grove
            .insert(
                FeePools::get_path(),
                constants::KEY_GENESIS_TIME.as_bytes(),
                Element::Item(genesis_time.to_le_bytes().to_vec(), None),
                transaction,
            )
            .map_err(Error::GroveDB)?;

        self.genesis_time = Some(genesis_time);

        Ok(())
    }

    pub fn get_current_epoch_index(
        &self,
        block_time: i64,
        previous_block_time: i64,
        transaction: TransactionArg,
    ) -> Result<(u16, bool), Error> {
        let genesis_time = self.get_genesis_time(transaction)?;

        let prev_epoch_index =
            (previous_block_time - genesis_time) as f64 / constants::EPOCH_CHANGE_TIME as f64;
        let prev_epoch_index_floored = prev_epoch_index.floor();

        let epoch_index = (block_time - genesis_time) as f64 / constants::EPOCH_CHANGE_TIME as f64;
        let epoch_index_floored = epoch_index.floor();

        let is_epoch_change = epoch_index_floored > prev_epoch_index_floored;

        Ok((epoch_index_floored as u16, is_epoch_change))
    }

    pub fn process_epoch_change(
        &self,
        epoch_index: u16,
        first_proposer_block_height: u64,
        transaction: TransactionArg,
    ) -> Result<(), Error> {
        // create and init next thousandth epoch
        let next_thousandth_epoch = EpochPool::new(epoch_index + 1000, self.drive);
        next_thousandth_epoch.init(transaction)?;

        // init first_proposer_block_height and processing_fee for an epoch
        let epoch = EpochPool::new(epoch_index, self.drive);
        epoch.update_first_proposer_block_height(first_proposer_block_height, transaction)?;
        epoch.update_processing_fee(0f64, transaction)?;
        epoch.init_proposers_tree(transaction)?;

        // distribute the storage fees
        self.distribute_storage_distribution_pool(epoch_index, transaction)
    }
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use crate::{
        drive::Drive,
        error,
        fee::pools::{epoch::epoch_pool::EpochPool, fee_pools::FeePools},
    };

    #[test]
    fn test_fee_pools_update_and_get_genesis_time() {
        let tmp_dir = TempDir::new().unwrap();
        let drive: Drive = Drive::open(tmp_dir).expect("expected to open Drive successfully");

        drive
            .create_root_tree(None)
            .expect("expected to create root tree successfully");

        let transaction = drive.grove.start_transaction();

        let mut fee_pools = FeePools::new(&drive);

        fee_pools
            .init(Some(&transaction))
            .expect("fee pools to init");

        let genesis_time: i64 = 1655396517902;

        fee_pools
            .update_genesis_time(genesis_time, Some(&transaction))
            .expect("to update genesis time");

        let stored_genesis_time = fee_pools
            .get_genesis_time(Some(&transaction))
            .expect("to get genesis time");

        assert_eq!(genesis_time, stored_genesis_time);

        // TODO: check db has not been called if genesis time was updated
    }

    #[test]
    fn test_fee_pools_get_current_epoch_index() {
        let tmp_dir = TempDir::new().unwrap();
        let drive: Drive = Drive::open(tmp_dir).expect("expected to open Drive successfully");

        drive
            .create_root_tree(None)
            .expect("expected to create root tree successfully");

        let transaction = drive.grove.start_transaction();

        let mut fee_pools = FeePools::new(&drive);

        fee_pools
            .init(Some(&transaction))
            .expect("fee pools to init");

        let genesis_time: i64 = 1655396517902;
        let block_time: i64 = 1655396517922;
        let prev_block_time: i64 = 1655396517912;

        fee_pools
            .update_genesis_time(genesis_time, Some(&transaction))
            .expect("to update genesis time");

        let (epoch_index, is_epoch_change) = fee_pools
            .get_current_epoch_index(block_time, prev_block_time, Some(&transaction))
            .expect("to get current epoch index");

        assert_eq!(epoch_index, 0);
        assert_eq!(is_epoch_change, false);

        let block_time: i64 = 1657125244561;

        let (epoch_index, is_epoch_change) = fee_pools
            .get_current_epoch_index(block_time, prev_block_time, Some(&transaction))
            .expect("to get current epoch index");

        assert_eq!(epoch_index, 1);
        assert_eq!(is_epoch_change, true);
    }

    #[test]
    fn test_fee_pools_process_epoch_change() {
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

        fee_pools
            .update_storage_fee_pool(42.0, Some(&transaction))
            .expect("to update storage fee pool");

        let first_proposer_block_height = 42;

        fee_pools
            .process_epoch_change(0, first_proposer_block_height, Some(&transaction))
            .expect("to process epoch change");

        let next_thousandth_epoch = EpochPool::new(1000, &drive);

        match next_thousandth_epoch.get_processing_fee(Some(&transaction)) {
            Ok(_) => assert!(false, "should not be able to get processing fee"),
            Err(e) => match e {
                error::Error::GroveDB(grovedb::Error::PathKeyNotFound(_)) => assert!(true),
                _ => assert!(false, "wrong error type"),
            },
        }

        let epoch = EpochPool::new(0, &drive);

        let processing_fee = epoch
            .get_processing_fee(Some(&transaction))
            .expect("to get processing fee");

        assert_eq!(processing_fee, 0.0);

        let first_proposer_block_count = epoch
            .get_first_proposed_block_height(Some(&transaction))
            .expect("to get first proposer block count");

        assert_eq!(first_proposer_block_count, 42);
    }
}
