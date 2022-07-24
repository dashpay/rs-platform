use crate::drive::Drive;
use crate::error::fee::FeeError;
use crate::error::Error;
use crate::fee_pools::epochs::Epoch;
use grovedb::TransactionArg;

impl Drive {
    pub fn get_epoch_block_count(
        &self,
        epoch: &Epoch,
        max_next_epoch_index: u16,
        cached_next_epoch_start_block_height: Option<u64>,
        transaction: TransactionArg,
    ) -> Result<u64, Error> {
        let next_start_block_height = if let Some(next_start_block_height) =
            cached_next_epoch_start_block_height
        {
            next_start_block_height
        } else {
            let (_, start_block_height) = self.find_next_epoch_start_block_height(
                    epoch.index,
                    max_next_epoch_index,
                    transaction,
                )?.ok_or(Error::Fee(FeeError::CorruptedCodeExecution("start_block_height must be present for current epoch or cached_next_epoch_start_block_height must be passed")))?;

            start_block_height
        };

        let current_start_block_height = self.get_epoch_start_block_height(epoch, transaction)?;

        let block_count = next_start_block_height
            .checked_sub(current_start_block_height)
            .ok_or(Error::Fee(FeeError::Overflow(
                "overflow for get_epoch_block_count",
            )))?;

        Ok(block_count)
    }
}

#[cfg(test)]
mod tests {
    mod get_epoch_block_count {
        #[test]
        fn test() {
            todo!()
        }
    }
}
