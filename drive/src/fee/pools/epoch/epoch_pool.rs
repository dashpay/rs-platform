use grovedb::{Element, TransactionArg};

use crate::drive::Drive;
use crate::error::Error;
use crate::fee::pools::fee_pools::FeePools;

use super::constants;

pub struct EpochPool<'e> {
    pub index: u16,
    pub key: [u8; 2],
    pub drive: &'e Drive,
}

impl<'e> EpochPool<'e> {
    pub fn new(index: u16, drive: &Drive) -> EpochPool {
        EpochPool {
            index,
            key: index.to_le_bytes(),
            drive,
        }
    }

    pub fn init_empty(&self, transaction: TransactionArg) -> Result<(), Error> {
        // init epoch tree
        self.drive
            .grove
            .insert(
                FeePools::get_path(),
                &self.key,
                Element::empty_tree(),
                transaction,
            )
            .map_err(Error::GroveDB)?;

        // init storage fee item to 0
        self.drive
            .grove
            .insert(
                self.get_path(),
                constants::KEY_STORAGE_FEE.as_bytes(),
                Element::Item(0f64.to_le_bytes().to_vec(), None),
                transaction,
            )
            .map_err(Error::GroveDB)?;

        Ok(())
    }

    pub fn init_current(
        &self,
        multiplier: u64,
        first_proposer_block_height: u64,
        transaction: TransactionArg,
    ) -> Result<(), Error> {
        self.update_first_proposer_block_height(first_proposer_block_height, transaction)?;

        self.update_processing_fee(0u64, transaction)?;

        self.init_proposers(transaction)?;

        self.update_fee_multiplier(multiplier, transaction)?;

        // TODO: Store start time as well

        Ok(())
    }

    pub fn mark_as_paid(&self, transaction: TransactionArg) -> Result<(), Error> {
        self.delete_proposers(transaction)?;

        self.delete_storage_fee(transaction)?;

        self.delete_processing_fee(transaction)?;

        Ok(())
    }

    pub fn get_path(&self) -> [&[u8]; 2] {
        [FeePools::get_path()[0], &self.key]
    }

    pub fn get_proposers_path(&self) -> [&[u8]; 3] {
        [
            FeePools::get_path()[0],
            &self.key,
            constants::KEY_PROPOSERS.as_bytes(),
        ]
    }
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use crate::{drive::Drive, error, fee::pools::fee_pools::FeePools};

    use super::EpochPool;

    #[test]
    fn test_init_empty() {
        let tmp_dir = TempDir::new().unwrap();
        let drive: Drive = Drive::open(tmp_dir).expect("expected to open Drive successfully");

        drive
            .create_root_tree(None)
            .expect("expected to create root tree successfully");

        let transaction = drive.grove.start_transaction();

        let epoch = EpochPool::new(1042, &drive);

        match epoch.init_empty(Some(&transaction)) {
            Ok(_) => assert!(false, "should not be able to init epoch without FeePools"),
            Err(e) => match e {
                error::Error::GroveDB(grovedb::Error::InvalidPath(_)) => assert!(true),
                _ => assert!(false, "ivalid error type"),
            },
        }

        let fee_pools = FeePools::new();

        fee_pools
            .init(&drive, Some(&transaction))
            .expect("fee pools to init");

        let epoch = EpochPool::new(1042, &drive);

        let multiplier = 42;

        epoch
            .init_empty(Some(&transaction))
            .expect("to init an epoch pool");

        let storage_fee = epoch
            .get_storage_fee(Some(&transaction))
            .expect("to get storage fee");

        assert_eq!(storage_fee, 0);

        let stored_multiplier = epoch
            .get_fee_multiplier(Some(&transaction))
            .expect("to get multiplier");

        assert_eq!(stored_multiplier, multiplier);
    }

    #[test]
    fn test_init_current() {
        todo!()
    }

    #[test]
    fn test_mark_as_paid() {
        let tmp_dir = TempDir::new().unwrap();
        let drive: Drive = Drive::open(tmp_dir).expect("expected to open Drive successfully");

        drive
            .create_root_tree(None)
            .expect("expected to create root tree successfully");

        let transaction = drive.grove.start_transaction();

        let fee_pools = FeePools::new();

        fee_pools
            .init(&drive, Some(&transaction))
            .expect("fee pools to init");

        let uninit_epoch_pool = EpochPool::new(7000, &drive);

        match uninit_epoch_pool.mark_as_paid(Some(&transaction)) {
            Ok(_) => assert!(false, "should not be able to delete uninit pool"),
            Err(e) => match e {
                error::Error::GroveDB(grovedb::Error::PathKeyNotFound(_)) => assert!(true),
                _ => assert!(false, "error type is wrong"),
            },
        }

        let epoch = EpochPool::new(42, &drive);

        epoch
            .mark_as_paid(Some(&transaction))
            .expect("to delete 42th epoch");

        match drive
            .grove
            .get(FeePools::get_path(), &epoch.key, Some(&transaction))
        {
            Ok(_) => assert!(false, "should not be able to get deleted epoch pool"),
            Err(e) => match e {
                grovedb::Error::PathKeyNotFound(_) => assert!(true),
                _ => assert!(false, "error should be of type PathKeyNotFound"),
            },
        }
    }
}
