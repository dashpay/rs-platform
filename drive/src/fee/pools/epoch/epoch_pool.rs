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

    pub fn init(&self, transaction: TransactionArg) -> Result<(), Error> {
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
            .map_err(Error::GroveDB)
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
    fn test_epoch_pool_init() {
        let tmp_dir = TempDir::new().unwrap();
        let drive: Drive = Drive::open(tmp_dir).expect("expected to open Drive successfully");

        drive
            .create_root_tree(None)
            .expect("expected to create root tree successfully");

        let transaction = drive.grove.start_transaction();

        let epoch = EpochPool::new(1042, &drive);

        match epoch.init(Some(&transaction)) {
            Ok(_) => assert!(false, "should not be able to init epoch without FeePools"),
            Err(e) => match e {
                error::Error::GroveDB(grovedb::Error::InvalidPath(_)) => assert!(true),
                _ => assert!(false, "ivalid error type"),
            },
        }

        let fee_pools = FeePools::new(&drive);

        fee_pools
            .init(Some(&transaction))
            .expect("fee pools to init");

        let epoch = EpochPool::new(1042, &drive);

        epoch
            .init(Some(&transaction))
            .expect("to init an epoch pool");

        let storage_fee = epoch
            .get_storage_fee(Some(&transaction))
            .expect("to get storage fee");

        assert_eq!(storage_fee, 0f64);
    }
}
