use crate::drive::fee_pools::{
    aggregate_storage_fees_distribution_pool_path,
    aggregate_storage_fees_distribution_pool_vec_path,
};
use crate::drive::Drive;
use grovedb::{Element, TransactionArg};

use crate::error::fee::FeeError;
use crate::error::Error;

use super::constants;

impl Drive {
    pub fn get_aggregate_storage_fees_in_current_distribution_pool(
        &self,
        transaction: TransactionArg,
    ) -> Result<u64, Error> {
        let element = self
            .grove
            .get(
                aggregate_storage_fees_distribution_pool_path(),
                constants::KEY_STORAGE_FEE_POOL.as_slice(),
                transaction,
            )
            .unwrap()
            .map_err(Error::GroveDB)?;

        if let Element::Item(item, _) = element {
            let fee = u64::from_be_bytes(item.as_slice().try_into().map_err(|_| {
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
