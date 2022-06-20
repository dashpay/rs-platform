use grovedb::{Element, TransactionArg};

use crate::drive::{Drive, RootTree};
use crate::error::Error;

use super::constants;
use super::epoch::epoch_pool::EpochPool;

pub struct FeePools<'f> {
    pub drive: &'f Drive,
    pub genesis_time: Option<i64>,
}

impl<'f> FeePools<'f> {
    pub fn new(drive: &Drive) -> FeePools {
        FeePools {
            drive,
            genesis_time: None,
        }
    }

    pub fn get_path<'a>() -> [&'a [u8]; 1] {
        [Into::<&[u8; 1]>::into(RootTree::Pools)]
    }

    pub fn init(&self, transaction: TransactionArg) -> Result<(), Error> {
        // init fee pool subtree
        self.drive
            .grove
            .insert(
                [],
                FeePools::get_path()[0],
                Element::empty_tree(),
                transaction,
            )
            .map_err(Error::GroveDB)?;

        // Update storage credit pool
        self.drive
            .grove
            .insert(
                FeePools::get_path(),
                constants::KEY_STORAGE_FEE_POOL.as_bytes(),
                Element::Item(0f64.to_le_bytes().to_vec(), None),
                transaction,
            )
            .map_err(Error::GroveDB)?;

        // We need to insert 50 years worth of epochs,
        // with 20 epochs per year that's 1000 epochs
        for i in 0..1000 {
            let epoch = EpochPool::new(i, self.drive);
            epoch.init(transaction)?;
        }

        Ok(())
    }

    pub fn get_oldest_epoch_pool(
        &self,
        epoch_index: u16,
        transaction: TransactionArg,
    ) -> Result<EpochPool, Error> {
        if epoch_index == 0 {
            return Ok(EpochPool::new(epoch_index, self.drive));
        }

        let epoch = EpochPool::new(epoch_index, self.drive);

        if epoch.is_proposers_tree_empty(transaction)? {
            return Ok(epoch);
        }

        self.get_oldest_epoch_pool(epoch_index - 1, transaction)
    }
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use crate::drive::Drive;

    use super::FeePools;

    #[test]
    fn test_fee_pools_init() {
        let tmp_dir = TempDir::new().unwrap();
        let drive: Drive = Drive::open(tmp_dir).expect("expected to open Drive successfully");

        drive
            .create_root_tree(None)
            .expect("expected to create root tree successfully");

        let transaction = drive.grove.start_transaction();

        let fee_pools = FeePools::new(&drive);

        fee_pools
            .init(Some(&transaction))
            .expect("fee pools to init");

        let storage_fee_pool = fee_pools
            .get_storage_fee_pool(Some(&transaction))
            .expect("to get storage fee pool");

        assert_eq!(storage_fee_pool, 0f64);
    }

    #[test]
    fn test_fee_pools_get_oldest_epoch() {
        let tmp_dir = TempDir::new().unwrap();
        let drive: Drive = Drive::open(tmp_dir).expect("expected to open Drive successfully");

        drive
            .create_root_tree(None)
            .expect("expected to create root tree successfully");

        let transaction = drive.grove.start_transaction();

        let fee_pools = FeePools::new(&drive);

        fee_pools
            .init(Some(&transaction))
            .expect("fee pools to init");

        let oldest_epoch = fee_pools
            .get_oldest_epoch_pool(999, Some(&transaction))
            .expect("to get oldest epoch pool");

        assert_eq!(oldest_epoch.index, 999);

        let proposer_pro_tx_hash: [u8; 32] =
            hex::decode("0101010101010101010101010101010101010101010101010101010101010101")
                .expect("to decode pro tx hash")
                .try_into()
                .expect("to convert vector to array of 32 bytes");

        oldest_epoch
            .init_proposers_tree(Some(&transaction))
            .expect("to init proposers tree");

        oldest_epoch
            .update_proposer_block_count(&proposer_pro_tx_hash, 1, Some(&transaction))
            .expect("to update proposer block count");

        let oldest_epoch = fee_pools
            .get_oldest_epoch_pool(999, Some(&transaction))
            .expect("to get oldest epoch pool");

        assert_eq!(oldest_epoch.index, 998);
    }
}
