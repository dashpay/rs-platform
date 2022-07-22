use crate::drive::fee_pools::pools_path;
use crate::drive::Drive;
use crate::error::Error;
use crate::fee_pools::epochs::Epoch;
use grovedb::TransactionArg;

pub mod block_count;
pub mod credit_distribution_pools;
pub mod proposers;
pub mod start_block;
pub mod start_time;

impl Drive {
    // TODO Should be part of Epoch
    pub fn is_epoch_tree_exists(
        &self,
        epoch_pool: &Epoch,
        transaction: TransactionArg,
    ) -> Result<bool, Error> {
        self.grove
            .has_raw(pools_path(), &epoch_pool.key, transaction)
            .unwrap()
            .map_err(Error::GroveDB)
    }
}

#[cfg(test)]
mod tests {
    use crate::common::helpers::setup::{setup_drive, setup_drive_with_initial_state_structure};

    use crate::drive::batch::GroveDbOpBatch;
    use crate::error;
    use crate::fee_pools::epochs::epoch_key_constants;
    use crate::fee_pools::epochs::Epoch;

    mod is_epoch_tree_exists {
        #[test]
        fn test_tree_exists() {
            todo!()
        }

        #[test]
        fn test_tree_doesnt_exist() {
            todo!()
        }
    }
}
