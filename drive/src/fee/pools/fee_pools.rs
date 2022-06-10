use grovedb::{Element, TransactionArg};

use crate::drive::{Drive, RootTree};
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

    pub fn init(&self, transaction: TransactionArg) -> Result<(), Error> {
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
                Element::Item(0f64.to_le_bytes().to_vec()),
                transaction,
            )
            .map_err(Error::GroveDB)?;

        // We need to insert 50 years worth of epochs,
        // with 20 epochs per year that's 1000 epochs
        for i in 0..1000 {
            let epoch = EpochPool::new(i, self.drive);
            epoch.init(transaction)?;
        }

        Ok(())
    }

    pub fn get_oldest_epoch_pool(
        &self,
        epoch_index: u16,
        transaction: TransactionArg,
    ) -> Result<EpochPool, Error> {
        if epoch_index == 1 {
            return Ok(EpochPool::new(epoch_index, self.drive));
        }

        let epoch = EpochPool::new(epoch_index, self.drive);

        if epoch.is_proposers_tree_empty(transaction)? {
            return Ok(epoch);
        }

        self.get_oldest_epoch_pool(epoch_index - 1, transaction)
    }
}
