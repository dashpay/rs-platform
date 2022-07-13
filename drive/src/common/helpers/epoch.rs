use std::ops::Range;
use grovedb::TransactionArg;
use crate::drive::Drive;
use crate::fee_pools::epochs::Epoch;

pub fn get_storage_credits_for_distribution_for_epochs_in_range(
    drive: &Drive,
    epoch_range: Range<u16>,
    transaction: TransactionArg,
) -> Vec<u64> {
    epoch_range
        .map(|index| {
            let epoch = Epoch::new(index);
            drive
                .get_epoch_storage_credits_for_distribution(&epoch, transaction)
                .expect("should get storage fee")
        })
        .collect()
}