use grovedb::{Element, TransactionArg};

use crate::drive::{Drive, RootTree};
use crate::error::fee::FeeError;
use crate::error::Error;

use super::constants;
use super::epoch::epoch_pool::EpochPool;

pub struct FeePools<'f> {
    pub drive: &'f Drive,
    pub genesis_time: Option<i64>,
}

impl<'f> FeePools<'f> {
    pub fn new(drive: &Drive) -> FeePools {
        FeePools {
            drive,
            genesis_time: None,
        }
    }

    pub fn get_path<'a>() -> [&'a [u8]; 1] {
        [Into::<&[u8; 1]>::into(RootTree::Pools)]
    }

    pub fn init(&self, multiplier: u64, transaction: TransactionArg) -> Result<(), Error> {
        // init fee pool subtree
        self.drive
            .grove
            .insert(
                [],
                FeePools::get_path()[0],
                Element::empty_tree(),
                transaction,
            )
            .map_err(Error::GroveDB)?;

        // Update storage credit pool
        self.drive
            .grove
            .insert(
                FeePools::get_path(),
                constants::KEY_STORAGE_FEE_POOL.as_bytes(),
                Element::Item(0f64.to_le_bytes().to_vec(), None),
                transaction,
            )
            .map_err(Error::GroveDB)?;

        // We need to insert 50 years worth of epochs,
        // with 20 epochs per year that's 1000 epochs
        for i in 0..1000 {
            let epoch = EpochPool::new(i, self.drive);
            epoch.init_empty(multiplier, transaction)?;
        }

        Ok(())
    }

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

        let is_epoch_change = if epoch_index_floored as u16 == 0 {
            true
        } else {
            epoch_index_floored > prev_epoch_index_floored
        };

        Ok((epoch_index_floored as u16, is_epoch_change))
    }

    pub fn process_epoch_change(
        &self,
        epoch_index: u16,
        first_proposer_block_height: u64,
        multiplier: u64,
        transaction: TransactionArg,
    ) -> Result<(), Error> {
        // create and init next thousandth epoch
        let next_thousandth_epoch = EpochPool::new(epoch_index + 1000, self.drive);
        next_thousandth_epoch.init_empty(multiplier, transaction)?;

        todo!("Store u64 multiplier");

        // init first_proposer_block_height and processing_fee for an epoch
        let epoch = EpochPool::new(epoch_index, self.drive);
        epoch.init_current(first_proposer_block_height, transaction)?;

        // distribute the storage fees
        self.distribute_storage_fee_pool(epoch_index, transaction)
    }
}

#[cfg(test)]
mod tests {
    use grovedb::Element;
    use tempfile::TempDir;

    use crate::drive::Drive;
    use crate::error;
    use crate::error::fee::FeeError;
    use crate::fee::pools::constants;
    use crate::fee::pools::epoch::epoch_pool::EpochPool;

    use super::FeePools;

    #[test]
    fn test_fee_pools_init() {
        let tmp_dir = TempDir::new().unwrap();
        let drive: Drive = Drive::open(tmp_dir).expect("expected to open Drive successfully");

        drive
            .create_root_tree(None)
            .expect("expected to create root tree successfully");

        let transaction = drive.grove.start_transaction();

        let fee_pools = FeePools::new(&drive);

        fee_pools
            .init(1, Some(&transaction))
            .expect("fee pools to init");

        let storage_fee_pool = fee_pools
            .get_storage_fee_pool(Some(&transaction))
            .expect("to get storage fee pool");

        assert_eq!(storage_fee_pool, 0f64);

        todo!("check that we have all 999 epoch pools")
    }

    #[test]
    fn test_fee_pools_update_and_get_genesis_time() {
        let tmp_dir = TempDir::new().unwrap();
        let drive: Drive = Drive::open(tmp_dir).expect("expected to open Drive successfully");

        drive
            .create_root_tree(None)
            .expect("expected to create root tree successfully");

        let transaction = drive.grove.start_transaction();

        let mut fee_pools = FeePools::new(&drive);

        let genesis_time: i64 = 1655396517902;

        match fee_pools.get_genesis_time(Some(&transaction)) {
            Ok(_) => assert!(
                false,
                "should not be able to get genesis time on uninit fee pools"
            ),
            Err(e) => match e {
                error::Error::GroveDB(grovedb::Error::PathNotFound(_)) => assert!(true),
                _ => assert!(false, "invalid error type"),
            },
        }

        match fee_pools.update_genesis_time(genesis_time, Some(&transaction)) {
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
            .init(1, Some(&transaction))
            .expect("fee pools to init");

        fee_pools
            .update_genesis_time(genesis_time, Some(&transaction))
            .expect("to update genesis time");

        let stored_genesis_time = fee_pools
            .get_genesis_time(Some(&transaction))
            .expect("to get genesis time");

        assert_eq!(genesis_time, stored_genesis_time);

        fee_pools.genesis_time = None;

        drive
            .grove
            .insert(
                FeePools::get_path(),
                constants::KEY_GENESIS_TIME.as_bytes(),
                Element::Item(u128::MAX.to_le_bytes().to_vec(), None),
                Some(&transaction),
            )
            .expect("to insert invalid data");

        match fee_pools.get_genesis_time(Some(&transaction)) {
            Ok(_) => assert!(false, "should not be able to decode stored value"),
            Err(e) => match e {
                error::Error::Fee(FeeError::CorruptedGenesisTimeInvalidItemLength(_)) => {
                    assert!(true)
                }
                _ => assert!(false, "ivalid error type"),
            },
        }
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
            .init(1, Some(&transaction))
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
        assert_eq!(is_epoch_change, true);

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
            .init(1, Some(&transaction))
            .expect("fee pools to init");

        let first_proposer_block_height = 1;

        fee_pools
            .process_epoch_change(0, first_proposer_block_height, 1, Some(&transaction))
            .expect("to process epoch change");

        // Verify next thousandth epoch
        let next_thousandth_epoch = EpochPool::new(1000, &drive);

        todo!("Check that storage fees are 0.0");

        // Make sure it's a new empty pool
        match next_thousandth_epoch.get_processing_fee(Some(&transaction)) {
            Ok(_) => assert!(false, "should not be able to get processing fee"),
            Err(e) => match e {
                error::Error::GroveDB(grovedb::Error::PathKeyNotFound(_)) => assert!(true),
                _ => assert!(false, "wrong error type"),
            },
        }

        // Make sure the current one was initialized
        let epoch = EpochPool::new(0, &drive);

        let processing_fee = epoch
            .get_processing_fee(Some(&transaction))
            .expect("to get processing fee");

        assert_eq!(processing_fee, 0.0);

        let first_proposer_block_count = epoch
            .get_first_proposer_block_height(Some(&transaction))
            .expect("to get first proposer block count");

        assert_eq!(first_proposer_block_count, first_proposer_block_height);

        todo!("check empty proposer tree exist");
    }
}
