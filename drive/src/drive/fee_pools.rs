use grovedb::TransactionArg;
use std::borrow::BorrowMut;

use crate::drive::Drive;
use crate::error::Error;

use crate::fee::pools::epoch::epoch_pool::EpochPool;
use chrono::Utc;

impl Drive {
    pub fn init_fee_pools(&self, transaction: TransactionArg) -> Result<(), Error> {
        // initialize the pools with epochs
        self.fee_pools.borrow().init(self, transaction)
    }

    pub fn process_block(
        &self,
        block_height: u64,
        block_time: i64,
        previous_block_time: Option<i64>,
        proposer_pro_tx_hash: [u8; 32],
        processing_fees: u64,
        storage_fees: i64,
        fee_multiplier: u64,
        transaction: TransactionArg,
    ) -> Result<(), Error> {
        if block_height == 1 {
            self.fee_pools
                .borrow_mut()
                .update_genesis_time(&self, block_time, transaction)?;
        }

        let (current_epoch_index, is_epoch_change) = match previous_block_time {
            Some(previous_block_time) => {
                self.fee_pools.borrow_mut().calculate_current_epoch_index(
                    &self,
                    block_time,
                    previous_block_time,
                    transaction,
                )?
            }
            None => (0, true),
        };

        let fee_pools = self.fee_pools.borrow();

        let current_epoch_pool = EpochPool::new(current_epoch_index, self);

        if is_epoch_change {
            // make next epoch pool as a current
            // and create one more in future
            fee_pools.shift_current_epoch_pool(
                &self,
                &current_epoch_pool,
                block_height,
                block_time,
                fee_multiplier,
                transaction,
            )?;

            // distribute accumulated previous epoch storage fees
            fee_pools.storage_fee_distribution_pool.distribute(
                &self,
                current_epoch_pool.index,
                transaction,
            )?;
        }

        fee_pools.distribute_fees_into_pools(
            &self,
            &current_epoch_pool,
            processing_fees,
            storage_fees,
            transaction,
        )?;

        current_epoch_pool.increment_proposer_block_count(&proposer_pro_tx_hash, transaction)?;

        fee_pools.distribute_fees_from_unpaid_pools_to_proposers(
            &self,
            current_epoch_index,
            transaction,
        )?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::fee::pools::tests::helpers::fee_pools::create_mn_shares_contract;
    use crate::fee::pools::tests::helpers::setup::setup_drive;
    use crate::fee::pools::tests::helpers::setup::setup_fee_pools;
    use crate::{
        contract::document::Document,
        drive::{
            flags::StorageFlags,
            object_size_info::{DocumentAndContractInfo, DocumentInfo::DocumentAndSerialization},
            Drive,
        },
        fee::pools::constants,
    };
    use chrono::Utc;

    mod process_block {
        use crate::fee::pools::constants;
        use crate::fee::pools::epoch::epoch_pool::EpochPool;
        use chrono::{Duration, NaiveDateTime, TimeZone};
        use std::borrow::BorrowMut;

        #[test]
        fn test_processing_of_the_first_block_then_new_epoch_and_one_more_block_after() {
            let drive = super::setup_drive();
            let (transaction, mut fee_pools) = super::setup_fee_pools(&drive, None);

            fee_pools
                .init(&drive, Some(&transaction))
                .expect("should init fee pools");

            super::create_mn_shares_contract(&drive);

            /*

            Block 1. Epoch 0.

             */

            let block_height = 1;
            let block_time = super::Utc::now();
            let block_timestamp = block_time.timestamp_millis();

            let proposer_pro_tx_hash = [
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0,
            ];

            let processing_fees = 100;
            let storage_fees = 2000;
            let fee_multiplier = 2;

            drive
                .process_block(
                    block_height,
                    block_timestamp,
                    None,
                    proposer_pro_tx_hash,
                    processing_fees,
                    storage_fees,
                    fee_multiplier,
                    Some(&transaction),
                )
                .expect("should process block 1");

            // Genesis time must be set
            let stored_genesis_time = fee_pools
                .get_genesis_time(&drive, Some(&transaction))
                .expect("should get genesis time");

            assert_eq!(stored_genesis_time, block_timestamp);

            // Fees must be distributed

            let stored_storage_fees = fee_pools
                .storage_fee_distribution_pool
                .value(&drive, Some(&transaction))
                .expect("should get storage fees");

            assert_eq!(stored_storage_fees, storage_fees);

            let epoch_pool_0 = EpochPool::new(0, &drive);

            let stored_processing_fees = epoch_pool_0
                .get_processing_fee(Some(&transaction))
                .expect("should get processing fees");

            assert_eq!(stored_processing_fees, processing_fees);

            // Proposer must be added

            let stored_proposer_block_count = epoch_pool_0
                .get_proposer_block_count(&proposer_pro_tx_hash, Some(&transaction))
                .expect("should get proposer block count");

            assert_eq!(stored_proposer_block_count, 1);

            /*

            Block 2. Epoch 1

             */

            let block_height = 2;
            let previous_block_timestamp = block_timestamp;
            let block_time = block_time + Duration::milliseconds(constants::EPOCH_CHANGE_TIME + 1);

            let block_timestamp = block_time.timestamp_millis();

            drive
                .process_block(
                    block_height,
                    block_timestamp,
                    Some(previous_block_timestamp),
                    proposer_pro_tx_hash,
                    processing_fees,
                    storage_fees,
                    fee_multiplier,
                    Some(&transaction),
                )
                .expect("should process block 2");

            /*

            Block 3. Epoch 1

            */
        }
    }
}
