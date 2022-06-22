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

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use crate::{
        drive::Drive,
        error,
        fee::pools::{epoch::epoch_pool::EpochPool, fee_pools::FeePools},
    };

    #[test]
    fn test_epoch_pool_delete() {
        let tmp_dir = TempDir::new().unwrap();
        let drive: Drive = Drive::open(tmp_dir).expect("expected to open Drive successfully");

        drive
            .create_root_tree(None)
            .expect("expected to create root tree successfully");

        let transaction = drive.grove.start_transaction();

        let fee_pools = FeePools::new();

        fee_pools
            .init(&drive, 1, Some(&transaction))
            .expect("fee pools to init");

        let uninit_epoch_pool = EpochPool::new(7000, &drive);

        match uninit_epoch_pool.delete(Some(&transaction)) {
            Ok(_) => assert!(false, "should not be able to delete uninit pool"),
            Err(e) => match e {
                error::Error::GroveDB(grovedb::Error::PathKeyNotFound(_)) => assert!(true),
                _ => assert!(false, "error type is wrong"),
            },
        }

        let epoch = EpochPool::new(42, &drive);

        epoch
            .delete(Some(&transaction))
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
