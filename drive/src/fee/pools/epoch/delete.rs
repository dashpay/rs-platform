use grovedb::TransactionArg;

use crate::{error::Error, fee::pools::fee_pools::FeePools};

use super::epoch_pool::EpochPool;

impl<'e> EpochPool<'e> {
    pub fn delete(&self, transaction: TransactionArg) -> Result<(), Error> {
        self.drive
            .grove
            .delete(FeePools::get_path(), &self.key, transaction)
            .map_err(Error::GroveDB)
    }
}
