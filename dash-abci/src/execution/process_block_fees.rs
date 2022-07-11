use rs_drive::drive::batch::GroveDbOpBatch;
use rs_drive::drive::fee_pools::fee_distribution::DistributionInfo;
use rs_drive::error::fee::FeeError;
use rs_drive::fee::epoch::EpochInfo;
use rs_drive::fee::fees_aggregate::FeesAggregate;
use rs_drive::fee_pools::epochs::EpochPool;
use rs_drive::query::GroveError::StorageError;
use rs_drive::query::TransactionArg;
use crate::block::BlockInfo;
use crate::error::Error;
use crate::platform::Platform;

impl Platform {
    pub fn process_block_fees(
        &self,
        block_info: &BlockInfo,
        epoch_info: &EpochInfo,
        fees: &FeesAggregate,
        transaction: TransactionArg,
    ) -> Result<DistributionInfo, Error> {
        let current_epoch_pool = EpochPool::new(epoch_info.current_epoch_index);

        if epoch_info.is_epoch_change {
            let mut batch = GroveDbOpBatch::new();

            // make next epochs pool as a current
            // and create one more in future
            current_epoch_pool.add_shift_current_epoch_pool_operations(
                &current_epoch_pool,
                block_info.block_height,
                block_info.block_time,
                fees.fee_multiplier,
                &mut batch,
            );

            // distribute accumulated previous epochs storage fees
            if current_epoch_pool.index > 0 {
                self.distribute_storage_fee_distribution_pool(

                    current_epoch_pool.index - 1,
                    transaction,
                    &mut batch,
                )?;
            }

            // We need to apply new epochs tree structure and distributed storage fee
            self.drive.grove_apply_batch(batch, false, transaction).map_err(StorageError)?;
        }

        let mut batch = GroveDbOpBatch::new();

        current_epoch_pool.add_increment_proposer_block_count_operations(
            &block_info.proposer_pro_tx_hash,
            transaction,
            &mut batch,
        )?;

        let distribution_info = self.drive.add_distribute_fees_from_unpaid_pools_to_proposers_operations(
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

        self.drive.add_distribute_fees_into_pools_operations(
            &current_epoch_pool,
            fees.processing_fees + processing_fees_leftovers,
            fees.storage_fees + storage_fees_leftovers,
            transaction,
            &mut batch,
        )?;

        self.drive.apply_if_not_empty(batch, false, transaction)?;

        Ok(distribution_info)
    }
}
