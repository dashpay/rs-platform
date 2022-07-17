use crate::abci::messages::FeesAggregate;
use crate::block::BlockInfo;
use crate::error::execution::ExecutionError;
use crate::error::Error;
use crate::execution::constants::DEFAULT_ORIGINAL_FEE_MULTIPLIER;
use crate::execution::epoch_change::distribute_storage_pool::DistributeStoragePoolResult;
use crate::execution::epoch_change::epoch::EpochInfo;
use crate::execution::fee_distribution::DistributionInfo;
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
    fn process_epoch_change_operations(
        &self,
        current_epoch: &Epoch,
        block_info: &BlockInfo,
        transaction: TransactionArg,
        batch: &mut GroveDbOpBatch,
    ) -> Result<Option<DistributeStoragePoolResult>, Error> {
        // make next epochs pool as a current
        // and create one more in future
        current_epoch.shift_to_new_epoch_operations(
            block_info.block_height,
            block_info.block_time,
            DEFAULT_ORIGINAL_FEE_MULTIPLIER, //todo use a data contract to choose the fee multiplier
            batch,
        );

        // distribute accumulated previous epochs storage fees
        let distribute_storage_result = if current_epoch.index > 0 {
            // On epoch change we
            Some(
                self.distribute_storage_fee_distribution_pool_to_epochs_operations(
                    EpochInfo {
                        current_epoch_index: current_epoch.index,
                        is_epoch_change: true,
                        block_height: block_info.block_height,
                    },
                    transaction,
                    batch,
                )?,
            )
        } else {
            None
        };
        Ok(distribute_storage_result)
    }

    pub fn process_block_fees(
        &self,
        block_info: &BlockInfo,
        epoch_info: &EpochInfo,
        mut block_fees: FeesAggregate,
        transaction: TransactionArg,
    ) -> Result<DistributionInfo, Error> {
        let current_epoch = Epoch::new(epoch_info.current_epoch_index);

        let mut batch = GroveDbOpBatch::new();

        let distribute_storage_pool_info_on_epoch_change = if epoch_info.is_epoch_change {
            self.process_epoch_change_operations(
                &current_epoch,
                block_info,
                transaction,
                &mut batch,
            )?
        } else {
            None
        };

        batch.push(current_epoch.increment_proposer_block_count_operation(
            &self.drive,
            epoch_info.is_epoch_change,
            &block_info.proposer_pro_tx_hash,
            transaction,
        )?);

        let mut distribution_info = self
            .add_distribute_fees_from_unpaid_pools_to_proposers_operations(
                epoch_info,
                transaction,
                &mut batch,
            )?;

        if let Some(distribute_storage_pool_info_on_epoch_change) =
            distribute_storage_pool_info_on_epoch_change
        {
            distribution_info.storage_distribution_pool_current_credits =
                distribute_storage_pool_info_on_epoch_change.leftover_storage_distribution_credits;
        }

        block_fees.storage_fees = block_fees
            .storage_fees
            .checked_add(distribution_info.storage_distribution_pool_current_credits)
            .ok_or(Error::Execution(ExecutionError::Overflow(
                "overflow combining storage with leftovers",
            )))?;

        self.add_distribute_fees_into_pools_operations(
            &current_epoch,
            epoch_info.is_epoch_change,
            block_fees,
            transaction,
            &mut batch,
        )?;

        self.drive.grove_apply_batch(batch, false, transaction)?;

        Ok(distribution_info)
    }
}
