use crate::block::BlockInfo;
use crate::error::Error;
use crate::execution::fee_distribution::DistributionInfo;
use crate::platform::Platform;
use rs_drive::drive::batch::GroveDbOpBatch;
use rs_drive::error::fee::FeeError;
use crate::execution::epoch_change::epoch::EpochInfo;
use rs_drive::fee_pools::epochs::Epoch;
use rs_drive::grovedb::TransactionArg;
use crate::abci::messages::FeesAggregate;
use crate::error::execution::ExecutionError;

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

impl Platform {
    fn process_epoch_change(
        &self,
        current_epoch: &Epoch,
        block_info: &BlockInfo,
        fees: &FeesAggregate,
        transaction: TransactionArg,
    ) {
        let mut batch = GroveDbOpBatch::new();

        // make next epochs pool as a current
        // and create one more in future
        current_epoch.shift_to_new_epoch_operations(
            block_info.block_height,
            block_info.block_time,
            fees.fee_multiplier,
            &mut batch,
        );

        // distribute accumulated previous epochs storage fees
        if current_epoch.index > 0 {
            self.distribute_storage_fee_distribution_pool_to_epochs_operations(
                current_epoch.index - 1,
                transaction,
                &mut batch,
            )?;
        }

        // We need to apply new epochs tree structure and distributed storage fee
        self.drive
            .grove_apply_batch(batch, false, transaction)
            .map_err(Error::Drive)?;
    }

    pub fn process_block_fees(
        &self,
        block_info: &BlockInfo,
        epoch_info: &EpochInfo,
        fees: &FeesAggregate,
        transaction: TransactionArg,
    ) -> Result<DistributionInfo, Error> {
        let current_epoch = Epoch::new(epoch_info.current_epoch_index);

        if epoch_info.is_epoch_change {
            self.process_epoch_change(&current_epoch, block_info, fees, transaction);
        }

        let mut batch = GroveDbOpBatch::new();

        batch.push(current_epoch.increment_proposer_block_count_operation(
            &self.drive,
            &block_info.proposer_pro_tx_hash,
            transaction,
        )?);

        let distribution_info = self
            .drive
            .add_distribute_fees_from_unpaid_pools_to_proposers_operations(
                epoch_info.current_epoch_index,
                transaction,
                &mut batch,
            )?;

        // Move integer part of the leftovers to processing
        // and fractional part to storage fees for the upcoming epochs
        let storage_fees_leftovers: u64 = (distribution_info.fee_leftovers.fract())
            .try_into()
            .map_err(|_| {
                Error::Fee(FeeError::DecimalConversion(
                    "can't convert storage fees leftovers from Decimal to i64",
                ))
            })?;
        let processing_fees_leftovers: u64 = (distribution_info.fee_leftovers.floor())
            .try_into()
            .map_err(|_| {
            Error::Fee(FeeError::DecimalConversion(
                "can't convert processing fees leftover from Decimal to u64",
            ))
        })?;

        let processing_fees_with_leftovers = fees
            .processing_fees
            .checked_add(processing_fees_leftovers)
            .ok_or(Error::Execution(ExecutionError::Overflow(
                "overflow combining processing with leftovers",
            )))?;

        let storage_fees_with_leftovers = fees
            .storage_fees
            .checked_add(storage_fees_leftovers)
            .ok_or(Error::Execution(ExecutionError::Overflow(
                "overflow combining storage with leftovers",
            )))?;

        self.add_distribute_fees_into_pools_operations(
            &current_epoch,
            processing_fees_with_leftovers,
            storage_fees_with_leftovers,
            transaction,
            &mut batch,
        )?;

        self.drive.grove_apply_batch(batch, false, transaction)?;

        Ok(distribution_info)
    }
}
