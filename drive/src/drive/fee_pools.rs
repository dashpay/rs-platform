use grovedb::TransactionArg;

use crate::drive::Drive;
use crate::error::Error;
use crate::fee::pools::fee_pools::FeePools;

use chrono::Utc;

impl Drive {
    pub fn init_fee_pools(
        &self,
        genesis_time: i64,
        transaction: TransactionArg,
    ) -> Result<(), Error> {
        let fee_pool = FeePools::new(self);

        // initialize the pools with epochs
        fee_pool.init(transaction)?;

        Ok(())
    }

    pub fn process_block(
        &self,
        block_height: u64,
        block_time: i64,
        previous_block_time: i64,
        proposer_pro_tx_hash: [u8; 32],
        processing_fees: f64,
        storage_fees: f64,
        transaction: TransactionArg,
    ) -> Result<(), Error> {
        let mut fee_pools = FeePools::new(self);

        if block_height == 1 {
            let genesis_time = Utc::now().timestamp();
            fee_pools.update_genesis_time(genesis_time, transaction)?;
        }

        let (epoch_index, is_epoch_change) =
            fee_pools.get_current_epoch_index(block_time, previous_block_time, transaction)?;

        if is_epoch_change {
            fee_pools.process_epoch_change(epoch_index, block_height, transaction)?;
        }

        fee_pools.distribute_st_fees(
            epoch_index,
            processing_fees,
            storage_fees,
            proposer_pro_tx_hash,
            transaction,
        )?;

        fee_pools.distribute_fees_to_proposers(epoch_index, transaction)?;

        Ok(())
    }
}
