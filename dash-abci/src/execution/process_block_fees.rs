use crate::abci::messages::FeesAggregate;
use crate::block::BlockInfo;
use crate::error::execution::ExecutionError;
use crate::error::Error;
use crate::execution::constants::{
    DEFAULT_ORIGINAL_FEE_MULTIPLIER, FOREVER_STORAGE_EPOCHS, FOREVER_STORAGE_YEARS,
    GENESIS_EPOCH_INDEX,
};
use crate::execution::epoch_change::distribute_storage_pool::DistributeStoragePoolResult;
use crate::execution::epoch_change::epoch::EpochInfo;
use crate::execution::fee_distribution::DistributionFeesFromUnpaidPoolsToProposersInfo;
use crate::platform::Platform;
use rs_drive::drive::batch::GroveDbOpBatch;
use rs_drive::fee_pools::epochs::Epoch;
use rs_drive::grovedb::TransactionArg;
use std::option::Option::None;

/// From the Dash Improvement Proposal:

/// For the purpose of this explanation we can trivialize that the execution of a block comprises
/// the sum of the execution of all state transitions contained within the block. In order to
/// avoid altering participating masternode identity balances every block and distribute fees
/// evenly, the concept of pools is introduced. We will also introduce the concepts of an Epoch
/// and the Epoch Year that are both covered later in this document. As the block executes state
/// transitions, processing and storage fees are accumulated, as well as a list of refunded fees
/// from various Epochs and fee multipliers. When there are no more state transitions to execute
/// we can say the block has ended its state transition execution phase. The system will then add
/// the accumulated fees to their corresponding pools, and in the case of deletion of data, remove
/// storage fees from future Epoch storage pools.
///

impl Platform {
    /// When processing an epoch change a DistributeStorageResult will be returned, expect if
    /// we are at Epoch 0.
    fn add_process_epoch_change_operations(
        &self,
        epoch_info: &EpochInfo,
        block_info: &BlockInfo,
        transaction: TransactionArg,
        batch: &mut GroveDbOpBatch,
    ) -> Result<Option<DistributeStoragePoolResult>, Error> {
        // init next thousandth empty epochs since last initiated
        let last_initiated_epoch_index = epoch_info
            .previous_epoch_index
            .map_or(GENESIS_EPOCH_INDEX, |i| i + 1);

        for epoch_index in last_initiated_epoch_index..=epoch_info.current_epoch_index {
            dbg!(epoch_index + FOREVER_STORAGE_EPOCHS);
            let next_thousandth_epoch = Epoch::new(epoch_index + FOREVER_STORAGE_EPOCHS);
            next_thousandth_epoch.add_init_empty_operations(batch);
        }

        // init current epoch pool for processing
        let current_epoch = Epoch::new(epoch_info.current_epoch_index);

        current_epoch.add_init_current_operations(
            DEFAULT_ORIGINAL_FEE_MULTIPLIER, // TODO use a data contract to choose the fee multiplier
            block_info.block_height,
            block_info.block_time_ms,
            batch,
        );

        // Nothing to distribute on epoch 0 start
        if current_epoch.index == GENESIS_EPOCH_INDEX {
            return Ok(None);
        }

        // Distribute accumulated storage fees from previous epoch
        let distribute_storage_result = self
            .add_distribute_storage_fee_distribution_pool_to_epochs_operations(
                &epoch_info,
                transaction,
                batch,
            )?;

        Ok(Some(distribute_storage_result))
    }

    pub fn process_block_fees(
        &self,
        block_info: &BlockInfo,
        epoch_info: &EpochInfo,
        block_fees: FeesAggregate,
        transaction: TransactionArg,
    ) -> Result<DistributionFeesFromUnpaidPoolsToProposersInfo, Error> {
        let current_epoch = Epoch::new(epoch_info.current_epoch_index);

        let mut batch = GroveDbOpBatch::new();

        let distribute_storage_pool_result = if epoch_info.is_epoch_change {
            self.add_process_epoch_change_operations(
                epoch_info,
                block_info,
                transaction,
                &mut batch,
            )?
        } else {
            None
        };

        batch.push(current_epoch.increment_proposer_block_count_operation(
            &self.drive,
            &block_info.proposer_pro_tx_hash,
            transaction,
        )?);

        let distribution_info = self
            .add_distribute_fees_from_unpaid_pools_to_proposers_operations(
                epoch_info,
                transaction,
                &mut batch,
            )?;

        // Add leftovers after storage fee pool distribution to the current block storage fees
        let block_fees_with_leftovers = if let Some(distribute_storage_pool_result) =
            distribute_storage_pool_result
        {
            let storage_fees_with_leftovers = block_fees
                .storage_fees
                .checked_add(distribute_storage_pool_result.leftover_storage_distribution_credits)
                .ok_or(Error::Execution(ExecutionError::Overflow(
                    "overflow combining storage with leftovers",
                )))?;

            FeesAggregate {
                storage_fees: storage_fees_with_leftovers,
                ..block_fees
            }
        } else {
            block_fees
        };

        self.add_distribute_block_fees_into_pools_operations(
            &current_epoch,
            block_fees_with_leftovers,
            transaction,
            &mut batch,
        )?;

        self.drive.grove_apply_batch(batch, false, transaction)?;

        Ok(distribution_info)
    }
}

#[cfg(test)]
mod tests {
    mod add_process_epoch_change_operations {
        use crate::abci::messages::FeesAggregate;
        use crate::block::BlockInfo;
        use crate::common::helpers::setup::setup_platform_with_initial_state_structure;
        use crate::execution::constants::{FOREVER_STORAGE_EPOCHS, GENESIS_EPOCH_INDEX};
        use crate::execution::epoch_change::epoch::{EpochInfo, EPOCH_CHANGE_TIME_MS};
        use chrono::Utc;
        use rs_drive::drive::batch::GroveDbOpBatch;
        use rs_drive::fee_pools::epochs::Epoch;
        use rust_decimal::prelude::ToPrimitive;

        mod helpers {
            use crate::abci::messages::FeesAggregate;
            use crate::block::BlockInfo;
            use crate::execution::constants::FOREVER_STORAGE_EPOCHS;
            use crate::execution::epoch_change::distribute_storage_pool::DistributeStoragePoolResult;
            use crate::execution::epoch_change::epoch::{EpochInfo, EPOCH_CHANGE_TIME_MS};
            use crate::platform::Platform;
            use rs_drive::drive::batch::GroveDbOpBatch;
            use rs_drive::fee_pools::epochs::Epoch;
            use rs_drive::grovedb::TransactionArg;

            pub fn process_and_validate_epoch_change(
                platform: &Platform,
                genesis_time_ms: u64,
                epoch_index: u16,
                block_height: u64,
                previous_block_time_ms: Option<u64>,
                should_distribute: bool,
                transaction: TransactionArg,
            ) -> BlockInfo {
                let current_epoch = Epoch::new(epoch_index);

                // Add some storage fees to distribute next time
                if should_distribute {
                    let block_fees = FeesAggregate {
                        processing_fees: 1000,
                        storage_fees: 1000000000,
                        refunds_by_epoch: vec![],
                    };

                    let mut batch = GroveDbOpBatch::new();

                    platform
                        .add_distribute_block_fees_into_pools_operations(
                            &current_epoch,
                            block_fees,
                            transaction,
                            &mut batch,
                        )
                        .expect("should add distribute block fees into pools operations");

                    platform
                        .drive
                        .grove_apply_batch(batch, false, transaction)
                        .expect("should apply batch");
                }

                let proposer_pro_tx_hash: [u8; 32] = [
                    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
                    1, 1, 1, 1, 1, 1,
                ];

                let block_time_ms = genesis_time_ms + epoch_index as u64 * EPOCH_CHANGE_TIME_MS;

                let block_info = BlockInfo {
                    block_height,
                    block_time_ms,
                    previous_block_time_ms,
                    proposer_pro_tx_hash,
                };

                let epoch_info =
                    EpochInfo::from_genesis_time_and_block_info(genesis_time_ms, &block_info)
                        .expect("should calculate epoch info");

                let mut batch = GroveDbOpBatch::new();

                let distribute_storage_pool_result = platform
                    .add_process_epoch_change_operations(
                        &epoch_info,
                        &block_info,
                        transaction,
                        &mut batch,
                    )
                    .expect("should process epoch");

                platform
                    .drive
                    .grove_apply_batch(batch, false, transaction)
                    .expect("should apply batch");

                // Next thousandth epoch should be created
                let next_thousandth_epoch = Epoch::new(epoch_index + FOREVER_STORAGE_EPOCHS);

                let is_epoch_tree_exists = platform
                    .drive
                    .is_epoch_tree_exists(&next_thousandth_epoch, transaction)
                    .expect("should check epoch tree existence");

                assert!(is_epoch_tree_exists);

                // epoch should be initialized as current
                let epoch_start_block_height = platform
                    .drive
                    .get_epoch_start_block_height(&current_epoch, transaction)
                    .expect("should get start block time from start epoch");

                assert_eq!(epoch_start_block_height, block_height);

                // storage fee should be distributed
                assert_eq!(distribute_storage_pool_result.is_some(), should_distribute);

                let thousandth_epoch = Epoch::new(next_thousandth_epoch.index - 1);

                let aggregate_storage_fees = platform
                    .drive
                    .get_epoch_storage_credits_for_distribution(&thousandth_epoch, transaction)
                    .expect("should get epoch storage fees");

                if should_distribute {
                    assert_ne!(aggregate_storage_fees, 0);
                } else {
                    assert_eq!(aggregate_storage_fees, 0);
                }

                block_info
            }
        }

        #[test]
        fn test_processing_epoch_change_for_epoch_0_1_and_4() {
            let platform = setup_platform_with_initial_state_structure();
            let transaction = platform.drive.grove.start_transaction();

            let genesis_time_ms = Utc::now()
                .timestamp_millis()
                .to_u64()
                .expect("block time can not be before 1970");

            /*
            Process genesis

            Storage fees shouldn't be distributed
             */

            let epoch_index = GENESIS_EPOCH_INDEX;
            let block_height = 1;

            let block_info = helpers::process_and_validate_epoch_change(
                &platform,
                genesis_time_ms,
                epoch_index,
                block_height,
                None,
                false,
                Some(&transaction),
            );

            /*
            Process epoch 1

            Storage fees should be distributed
             */

            let block_height = 2;
            let epoch_index = 1;

            let block_info = helpers::process_and_validate_epoch_change(
                &platform,
                genesis_time_ms,
                epoch_index,
                block_height,
                Some(block_info.block_time_ms),
                true,
                Some(&transaction),
            );

            /*
            Process epoch 4

            Multiple next empty epochs must be initialized and fees must be distributed
             */

            let block_height = 3;
            let epoch_index = 4;

            helpers::process_and_validate_epoch_change(
                &platform,
                genesis_time_ms,
                epoch_index,
                block_height,
                Some(block_info.block_time_ms),
                true,
                Some(&transaction),
            );
        }
    }

    mod process_block_fees {}
}
