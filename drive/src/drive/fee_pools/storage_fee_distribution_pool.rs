use crate::drive::Drive;
use grovedb::{Element, TransactionArg};

use crate::error::fee::FeeError;
use crate::error::Error;

use super::constants;

impl Drive {
    pub fn get_aggregate_storage_fees_in_current_distribution_pool (
        &self,
        transaction: TransactionArg,
    ) -> Result<i64, Error> {
        let element = self
            .grove
            .get(
                get_poo,
                constants::KEY_STORAGE_FEE_POOL.as_slice(),
                transaction,
            )
            .unwrap()
            .map_err(Error::GroveDB)?;

        if let Element::Item(item, _) = element {
            let fee = i64::from_le_bytes(item.as_slice().try_into().map_err(|_| {
                Error::Fee(FeeError::CorruptedStorageFeePoolInvalidItemLength(
                    "fee pools storage fee pool is not i64",
                ))
            })?);

            Ok(fee)
        } else {
            Err(Error::Fee(FeeError::CorruptedStorageFeePoolNotItem(
                "fee pools storage fee pool must be an item",
            )))
        }
    }
}

#[cfg(test)]
mod tests {

    mod helpers {
        use grovedb::TransactionArg;
        use rust_decimal::Decimal;
        use crate::drive::Drive;
        use crate::fee_pools::epoch_pool::EpochPool;

        pub fn get_storage_fees_from_epoch_pools(
            drive: &Drive,
            epoch_index: u16,
            transaction: TransactionArg,
        ) -> Vec<Decimal> {
            (epoch_index..epoch_index + 1000)
                .map(|index| {
                    let epoch_pool = EpochPool::new(index);
                    epoch_pool
                        .get_storage_fee(transaction)
                        .expect("should get storage fee")
                })
                .collect()
        }
    }
}
