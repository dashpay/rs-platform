use crate::drive::Drive;
use crate::error::fee::FeeError;
use crate::error::Error;
use crate::fee_pools::epochs::Epoch;
use grovedb::TransactionArg;

impl Drive {
    pub fn get_epoch_block_count(
        &self,
        epoch: &Epoch,
        cached_next_epoch_start_block_height: Option<u64>,
        transaction: TransactionArg,
    ) -> Result<u64, Error> {
        let next_epoch_pool = Epoch::new(epoch.index + 1);

        let next_start_block_height =
            if let Some(next_start_block_height) = cached_next_epoch_start_block_height {
                next_start_block_height
            } else {
                self.get_epoch_start_block_height(&next_epoch_pool, transaction)?
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
