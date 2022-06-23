use grovedb::TransactionArg;

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
        previous_block_time: i64,
        proposer_pro_tx_hash: [u8; 32],
        processing_fees: u64,
        storage_fees: i64,
        fee_multiplier: u64,
        transaction: TransactionArg,
    ) -> Result<(), Error> {
        if block_height == 1 {
            let genesis_time = Utc::now().timestamp();

            self.fee_pools
                .borrow_mut()
                .update_genesis_time(&self, genesis_time, transaction)?;
        }

        let fee_pools = self.fee_pools.borrow();

        let (current_epoch_index, is_epoch_change) = fee_pools.calculate_current_epoch_index(
            &self,
            block_time,
            previous_block_time,
            transaction,
        )?;

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
    use crate::{
        contract::{Contract, Document},
        drive::{
            flags::StorageFlags,
            object_size_info::{DocumentAndContractInfo, DocumentInfo::DocumentAndSerialization},
            Drive,
        },
        fee::pools::constants,
    };

    mod process_block {
        use crate::drive::Drive;
        use chrono::Utc;
        use tempfile::TempDir;

        #[test]
        fn test_process_first_block() {
            let tmp_dir = TempDir::new().unwrap();
            let mut drive: Drive =
                Drive::open(tmp_dir).expect("expected to open Drive successfully");

            drive
                .create_root_tree(None)
                .expect("expected to create root tree successfully");

            super::setup_mn_share_contract_and_docs(&drive);

            let transaction = drive.grove.start_transaction();

            drive
                .init_fee_pools(Some(&transaction))
                .expect("to init fee pools");

            let block_time = Utc::now().timestamp();
            let previous_block_time = Utc::now().timestamp();

            let proposer_pro_tx_hash = [
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0,
            ];

            let processing_fees = 100;
            let storage_fees = 2000;

            drive
                .process_block(
                    1,
                    block_time,
                    previous_block_time,
                    proposer_pro_tx_hash,
                    processing_fees,
                    storage_fees,
                    1,
                    Some(&transaction),
                )
                .expect("to process block 1");
        }

        #[test]
        fn test_process_second_block() {
            todo!()
        }

        #[test]
        fn test_process_epoch_change() {
            todo!()
        }
    }
}
