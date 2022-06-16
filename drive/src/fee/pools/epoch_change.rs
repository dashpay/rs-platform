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
        epoch.update_first_proposed_block_height(first_proposer_block_height, transaction)?;
        epoch.update_processing_fee(0f64, transaction)?;

        // distribute the storage fees
        self.distribute_storage_distribution_pool(epoch_index, transaction)
    }
}
