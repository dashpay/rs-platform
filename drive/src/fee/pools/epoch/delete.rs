use grovedb::TransactionArg;

use crate::error::Error;

use super::epoch_pool::EpochPool;

impl<'e> EpochPool<'e> {
    pub fn delete(&self, transaction: TransactionArg) -> Result<(), Error> {
        Ok(())
    }
}
