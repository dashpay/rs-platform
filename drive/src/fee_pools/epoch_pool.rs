use std::borrow::Cow;
use grovedb::batch::{GroveDbOp, Op};
use grovedb::{Element, TransactionArg};
use grovedb::batch::Op::Insert;
use crate::drive::batch::GroveDbOpBatch;
use crate::drive::fee_pools::epoch::tree_key_constants;
use crate::drive::fee_pools::epoch::tree_key_constants::{KEY_FEE_MULTIPLIER, KEY_POOL_PROCESSING_FEES, KEY_POOL_STORAGE_FEES, KEY_START_BLOCK_HEIGHT, KEY_START_TIME};
use crate::error::Error;

pub struct EpochPool {
    pub index: u16,
    pub key: [u8; 2],
}

impl EpochPool {
    pub fn new(index: u16) -> EpochPool {
        EpochPool {
            index,
            key: index.to_be_bytes(),
        }
    }
}

impl EpochPool {
    pub fn add_shift_current_epoch_pool_operations(
        &self,
        current_epoch_pool: &EpochPool,
        start_block_height: u64,
        start_block_time_ms: u64,
        fee_multiplier: u64,
        batch: &mut GroveDbOpBatch,
    ) {
        // create and init next thousandth epoch
        let next_thousandth_epoch = EpochPool::new(current_epoch_pool.index + 1000);
        next_thousandth_epoch.add_init_empty_operations(batch);

        // init first_proposer_block_height and processing_fee for an epoch
        current_epoch_pool.add_init_current_operations(
            fee_multiplier,
            start_block_height,
            start_block_time_ms,
            batch,
        );
    }


    pub fn add_init_empty_operations(&self, batch: &mut GroveDbOpBatch) {
        batch.insert_empty_tree(FeePools::get_path(), self.key.to_vec());

        // init storage fee item to 0
        batch.push(self.update_storage_fee_operation( 0));
    }

    pub fn add_init_current_operations(
        &self,
        multiplier: u64,
        start_block_height: u64,
        start_time_ms: u64,
        batch: &mut GroveDbOpBatch,
    ) {
        batch.push(self.update_start_block_height_operation(start_block_height));

        batch.push(self.update_processing_fee_operation(0u64));

        batch.push(self.init_proposers_operation());

        batch.push(self.update_fee_multiplier_operation(multiplier));

        batch.push(self.update_start_time_operation(start_time_ms));
    }

    pub fn add_mark_as_paid_operations(
        &self,
        transaction: TransactionArg,
        batch: &mut GroveDbOpBatch,
    ) -> Result<(), Error> {
        self.add_delete_proposers_tree_operations(batch, transaction)?;

        self.add_delete_storage_fee_operations(batch, transaction)?;

        self.add_delete_processing_fee_operations(batch, transaction)?;

        Ok(())
    }

    pub fn get_path(&self) -> [&[u8]; 2] {
        [FeePools::get_path()[0], &self.key]
    }

    pub fn get_vec_path(&self) -> Vec<Vec<u8>> {
        vec![ FEE, self.key.to_vec()]
    }

    pub fn update_start_time_operation(
        &self,
        time_ms: u64,
    ) -> GroveDbOp {
        GroveDbOp {
            path: self.get_vec_path(),
            key: KEY_START_TIME.to_vec(),
            op: Insert {
                element : Element::Item(time_ms.to_be_bytes().to_vec(), None)
            }
        }
    }

    pub fn update_start_block_height_operation(
        &self,
        start_block_height: u64,
    ) -> GroveDbOp {
        GroveDbOp {
            path: self.get_vec_path(),
            key: KEY_START_BLOCK_HEIGHT.to_vec(),
            op: Insert {
                element : Element::Item(start_block_height.to_be_bytes().to_vec(), None)
            }
        }
    }

    pub fn update_fee_multiplier_operation(
        &self,
        multiplier: u64,
    ) -> GroveDbOp {
        GroveDbOp {
            path: self.get_vec_path,
            key: KEY_FEE_MULTIPLIER.to_vec(),
            op: Insert { element: Element::Item(multiplier.to_be_bytes().to_vec(), None)}
        }
    }

    pub fn update_processing_fee_operation(
        &self,
        processing_fee: u64,
    ) -> GroveDbOp {
        GroveDbOp {
            path: self.get_vec_path(),
            key: KEY_POOL_PROCESSING_FEES.to_vec(),
            op: Insert { element: Element::new_item(processing_fee.to_be_bytes().to_vec())}
        }
    }

    pub fn delete_processing_fee_operation(
        &self,
    ) -> GroveDbOp {
        GroveDbOp {
            path: self.get_vec_path(),
            key: KEY_POOL_PROCESSING_FEES.to_vec(),
            op: Op::Delete
        }
    }

    pub fn update_storage_fee_operation(
        &self,
        storage_fee: u64,
    ) -> GroveDbOp {
        GroveDbOp {
            path: self.get_vec_path(),
            key: KEY_POOL_STORAGE_FEES.to_vec(),
            op: Insert { element: Element::new_item(storage_fee.to_be_bytes().to_vec())}
        }
    }

    pub fn delete_storage_fee_operation(
        &self
    ) -> GroveDbOp {
        GroveDbOp {
            path: self.get_vec_path(),
            key: KEY_POOL_STORAGE_FEES.to_vec(),
            op: Op::Delete
        }
    }

    fn update_proposer_block_count_operation(
        &self,
        proposer_pro_tx_hash: &[u8; 32],
        block_count: u64,
    ) -> GroveDbOp {
        GroveDbOp {
            path: self.get_proposers_path(),
            key: proposer_pro_tx_hash.to_vec(),
            op: Insert { element : Element::Item(block_count.to_be_bytes().to_vec(), None)}
        }
    }

    pub fn add_increment_proposer_block_count_operations(
        &self,
        proposer_pro_tx_hash: &[u8; 32],
        transaction: TransactionArg,
        batch: &mut GroveDbOpBatch,
    ) -> Result<(), Error> {
        // update proposer's block count
        let proposed_block_count = self
            .get_proposer_block_count(proposer_pro_tx_hash, transaction)
            .or_else(|e| match e {
                Error::GroveDB(grovedb::Error::PathKeyNotFound(_)) => Ok(0u64),
                _ => Err(e),
            })?;

        let op = self.update_proposer_block_count_operation(
            proposer_pro_tx_hash,
            proposed_block_count + 1,
        );

        batch.push(op);

        Ok(())
    }

    pub fn init_proposers_operation(&self) -> GroveDbOp {
        GroveDbOp {
            path: self.get_vec_path(),
            key: tree_key_constants::KEY_PROPOSERS.to_vec(),
            op: Op::Insert { element: Element::empty_tree()}
        }
    }

    pub fn delete_proposers_tree_operation(
        &self,
    ) -> GroveDbOp {
        GroveDbOp {
            path: self.get_vec_path(),
            key: tree_key_constants::KEY_PROPOSERS.to_vec(),
            op: Op::Delete
        }
    }

    pub fn add_delete_proposers_operations(
        &self,
        pro_tx_hashes: Vec<Vec<u8>>,
        batch: &mut GroveDbOpBatch,
    ) -> Result<(), Error> {
        for pro_tx_hash in pro_tx_hashes.into_iter() {
            batch.delete(self.get_proposers_path(), Cow::from(pro_tx_hash))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::drive::storage::batch::GroveDbOpBatch;
    use chrono::Utc;
    use grovedb::Element;
    use rust_decimal_macros::dec;

    use crate::error;
    use crate::error::fee::FeeError;
    use crate::fee::pools::epoch::constants;
    use crate::fee::pools::tests::helpers::setup::SetupFeePoolsOptions;
    use crate::fee::pools::tests::helpers::setup::{setup_drive, setup_fee_pools};

    use super::EpochPool;

    #[test]
    fn test_update_start_time() {
        let drive = setup_drive();

        let (transaction, _) = setup_fee_pools(&drive, None);

        let epoch_pool = super::EpochPool::new(0, &drive);

        let start_time: i64 = Utc::now().timestamp_millis();

        let mut batch = GroveDbOpBatch::new(&drive);

        epoch_pool
            .add_update_start_time_operations(&mut batch, start_time)
            .expect("should update start time");

        drive
            .apply_batch(batch, false, Some(&transaction))
            .expect("should apply batch");

        let actual_start_time = epoch_pool
            .get_start_time(Some(&transaction))
            .expect("should get start time");

        assert_eq!(start_time, actual_start_time);
    }

    mod get_start_time {
        #[test]
        fn test_error_if_epoch_pool_is_not_initiated() {
            let drive = super::setup_drive();

            let (transaction, _) = super::setup_fee_pools(&drive, None);

            let non_initiated_epoch_pool = super::EpochPool::new(7000, &drive);

            match non_initiated_epoch_pool.get_start_time(Some(&transaction)) {
                Ok(_) => assert!(
                    false,
                    "should not be able to get start time on uninit epoch pool"
                ),
                Err(e) => match e {
                    super::error::Error::GroveDB(grovedb::Error::PathNotFound(_)) => assert!(true),
                    _ => assert!(false, "invalid error type"),
                },
            }
        }

        #[test]
        fn test_error_if_value_is_not_set() {
            let drive = super::setup_drive();

            let (transaction, _) = super::setup_fee_pools(&drive, None);

            let epoch_pool = super::EpochPool::new(0, &drive);

            match epoch_pool.get_start_time(Some(&transaction)) {
                Ok(_) => assert!(false, "must be an error"),
                Err(e) => match e {
                    super::error::Error::GroveDB(_) => assert!(true),
                    _ => assert!(false, "invalid error type"),
                },
            }
        }

        #[test]
        fn test_error_if_element_has_invalid_type() {
            let drive = super::setup_drive();

            let (transaction, _) = super::setup_fee_pools(&drive, None);

            let epoch_pool = super::EpochPool::new(0, &drive);

            drive
                .grove
                .insert(
                    epoch_pool.get_path(),
                    super::constants::KEY_START_TIME.as_slice(),
                    super::Element::empty_tree(),
                    Some(&transaction),
                )
                .unwrap()
                .expect("should insert invalid data");

            match epoch_pool.get_start_time(Some(&transaction)) {
                Ok(_) => assert!(false, "must be an error"),
                Err(e) => match e {
                    super::error::Error::Fee(super::FeeError::CorruptedStartTimeNotItem()) => {
                        assert!(true)
                    }
                    _ => assert!(false, "invalid error type"),
                },
            }
        }

        #[test]
        fn test_error_if_value_has_invalid_length() {
            let drive = super::setup_drive();

            let (transaction, _) = super::setup_fee_pools(&drive, None);

            let epoch_pool = super::EpochPool::new(0, &drive);

            drive
                .grove
                .insert(
                    epoch_pool.get_path(),
                    super::constants::KEY_START_TIME.as_slice(),
                    super::Element::Item(u128::MAX.to_be_bytes().to_vec(), None),
                    Some(&transaction),
                )
                .unwrap()
                .expect("should insert invalid data");

            match epoch_pool.get_start_time(Some(&transaction)) {
                Ok(_) => assert!(false, "must be an error"),
                Err(e) => match e {
                    super::error::Error::Fee(super::FeeError::CorruptedStartTimeLength()) => {
                        assert!(true)
                    }
                    _ => assert!(false, "invalid error type"),
                },
            }
        }
    }

    #[test]
    fn test_update_start_block_height() {
        let drive = setup_drive();

        let (transaction, _) = setup_fee_pools(&drive, None);

        let epoch_pool = EpochPool::new(0, &drive);

        let start_block_height = 1;

        let mut batch = GroveDbOpBatch::new(&drive);

        epoch_pool
            .add_update_start_block_height_operations(&mut batch, start_block_height)
            .expect("should update start block height");

        drive
            .apply_batch(batch, false, Some(&transaction))
            .expect("should apply batch");

        let actual_start_block_height = epoch_pool
            .get_start_block_height(Some(&transaction))
            .expect("should get start block height");

        assert_eq!(start_block_height, actual_start_block_height);
    }

    mod get_start_block_height {
        #[test]
        fn test_error_if_epoch_pool_is_not_initiated() {
            let drive = super::setup_drive();

            let (transaction, _) = super::setup_fee_pools(&drive, None);

            let non_initiated_epoch_pool = super::EpochPool::new(7000, &drive);

            match non_initiated_epoch_pool.get_start_block_height(Some(&transaction)) {
                Ok(_) => assert!(
                    false,
                    "should not be able to get start block height on uninit epoch pool"
                ),
                Err(e) => match e {
                    super::error::Error::GroveDB(grovedb::Error::PathNotFound(_)) => assert!(true),
                    _ => assert!(false, "invalid error type"),
                },
            }
        }

        #[test]
        fn test_error_if_value_is_not_set() {
            let drive = super::setup_drive();

            let (transaction, _) = super::setup_fee_pools(&drive, None);

            let epoch_pool = super::EpochPool::new(0, &drive);

            match epoch_pool.get_start_block_height(Some(&transaction)) {
                Ok(_) => assert!(false, "must be an error"),
                Err(e) => match e {
                    super::error::Error::GroveDB(_) => assert!(true),
                    _ => assert!(false, "invalid error type"),
                },
            }
        }

        #[test]
        fn test_error_if_value_has_invalid_length() {
            let drive = super::setup_drive();

            let (transaction, _) = super::setup_fee_pools(&drive, None);

            let epoch_pool = super::EpochPool::new(0, &drive);

            drive
                .grove
                .insert(
                    epoch_pool.get_path(),
                    super::constants::KEY_START_BLOCK_HEIGHT.as_slice(),
                    super::Element::Item(u128::MAX.to_be_bytes().to_vec(), None),
                    Some(&transaction),
                )
                .unwrap()
                .expect("should insert invalid data");

            match epoch_pool.get_start_block_height(Some(&transaction)) {
                Ok(_) => assert!(false, "should not be able to decode stored value"),
                Err(e) => match e {
                    super::error::Error::Fee(
                        super::FeeError::CorruptedStartBlockHeightItemLength(),
                    ) => {
                        assert!(true)
                    }
                    _ => assert!(false, "invalid error type"),
                },
            }
        }

        #[test]
        fn test_error_if_element_has_invalid_type() {
            let drive = super::setup_drive();

            let (transaction, _) = super::setup_fee_pools(&drive, None);

            let epoch_pool = super::EpochPool::new(0, &drive);

            drive
                .grove
                .insert(
                    epoch_pool.get_path(),
                    super::constants::KEY_START_BLOCK_HEIGHT.as_slice(),
                    super::Element::empty_tree(),
                    Some(&transaction),
                )
                .unwrap()
                .expect("should insert invalid data");

            match epoch_pool.get_start_block_height(Some(&transaction)) {
                Ok(_) => assert!(false, "should not be able to decode stored value"),
                Err(e) => match e {
                    super::error::Error::Fee(
                        super::FeeError::CorruptedStartBlockHeightNotItem(),
                    ) => {
                        assert!(true)
                    }
                    _ => assert!(false, "invalid error type"),
                },
            }
        }
    }

    mod init_empty {
        use crate::drive::storage::batch::GroveDbOpBatch;

        #[test]
        fn test_error_if_fee_pools_not_initialized() {
            let drive = super::setup_drive();
            let (transaction, _) = super::setup_fee_pools(
                &drive,
                Some(super::SetupFeePoolsOptions {
                    apply_fee_pool_structure: false,
                }),
            );

            let epoch = super::EpochPool::new(1042, &drive);

            let mut batch = GroveDbOpBatch::new(&drive);

            epoch
                .add_init_empty_operations(&mut batch)
                .expect("should init empty pool");

            match drive.apply_batch(batch, false, Some(&transaction)) {
                Ok(_) => assert!(false, "should not be able to init epoch without FeePools"),
                Err(e) => match e {
                    super::error::Error::GroveDB(grovedb::Error::PathKeyNotFound(_)) => {
                        assert!(true)
                    }
                    _ => assert!(false, "invalid error type"),
                },
            }
        }

        #[test]
        fn test_values_are_set() {
            let drive = super::setup_drive();
            let (transaction, _) = super::setup_fee_pools(&drive, None);

            let epoch = super::EpochPool::new(1042, &drive);

            let mut batch = GroveDbOpBatch::new(&drive);

            epoch
                .add_init_empty_operations(&mut batch)
                .expect("should init an epoch pool");

            drive
                .apply_batch(batch, false, Some(&transaction))
                .expect("should apply batch");

            let storage_fee = epoch
                .get_storage_fee(Some(&transaction))
                .expect("should get storage fee");

            assert_eq!(storage_fee, super::dec!(0.0));
        }
    }

    mod init_current {
        use crate::drive::storage::batch::GroveDbOpBatch;

        #[test]
        fn test_values_are_set() {
            let drive = super::setup_drive();
            let (transaction, _) = super::setup_fee_pools(&drive, None);

            let epoch = super::EpochPool::new(1042, &drive);

            let multiplier = 42;
            let start_time = 1;
            let start_block_height = 2;

            let mut batch = GroveDbOpBatch::new(&drive);

            epoch
                .add_init_empty_operations(&mut batch)
                .expect("should init empty epoch pool");

            epoch
                .add_init_current_operations(multiplier, start_block_height, start_time, &mut batch)
                .expect("should init an epoch pool");

            drive
                .apply_batch(batch, false, Some(&transaction))
                .expect("should apply batch");

            let stored_multiplier = epoch
                .get_fee_multiplier(Some(&transaction))
                .expect("should get multiplier");

            assert_eq!(stored_multiplier, multiplier);

            let stored_start_time = epoch
                .get_start_time(Some(&transaction))
                .expect("should get start time");

            assert_eq!(stored_start_time, start_time);

            let stored_block_height = epoch
                .get_start_block_height(Some(&transaction))
                .expect("should get start block height");

            assert_eq!(stored_block_height, start_block_height);

            let stored_processing_fee = epoch
                .get_processing_fee(Some(&transaction))
                .expect("should get processing fee");

            assert_eq!(stored_processing_fee, 0);

            let proposers = epoch
                .get_proposers(1, Some(&transaction))
                .expect("should get proposers");

            assert_eq!(proposers, vec!());
        }
    }

    mod mark_as_paid {
        use crate::drive::storage::batch::GroveDbOpBatch;

        #[test]
        fn test_values_are_deleted() {
            let drive = super::setup_drive();
            let (transaction, _) = super::setup_fee_pools(&drive, None);

            let epoch = super::EpochPool::new(0, &drive);

            let mut batch = GroveDbOpBatch::new(&drive);

            epoch
                .add_init_current_operations(1, 2, 3, &mut batch)
                .expect("should init an epoch pool");

            // Apply init current
            drive
                .apply_batch(batch, false, Some(&transaction))
                .expect("should apply batch");

            let mut batch = GroveDbOpBatch::new(&drive);

            epoch
                .add_mark_as_paid_operations(&mut batch, Some(&transaction))
                .expect("should mark epoch as paid");

            drive
                .apply_batch(batch, false, Some(&transaction))
                .expect("should apply batch");

            match drive
                .grove
                .get(
                    epoch.get_path(),
                    super::constants::KEY_PROPOSERS.as_slice(),
                    Some(&transaction),
                )
                .unwrap()
            {
                Ok(_) => assert!(false, "should not be able to get proposers"),
                Err(e) => match e {
                    grovedb::Error::PathKeyNotFound(_) => assert!(true),
                    _ => assert!(false, "invalid error type"),
                },
            }

            match epoch.get_processing_fee(Some(&transaction)) {
                Ok(_) => assert!(false, "should not be able to get processing fee"),
                Err(e) => match e {
                    super::error::Error::GroveDB(grovedb::Error::PathKeyNotFound(_)) => {
                        assert!(true)
                    }
                    _ => assert!(false, "invalid error type"),
                },
            }

            match epoch.get_storage_fee(Some(&transaction)) {
                Ok(_) => assert!(false, "should not be able to get storage fee"),
                Err(e) => match e {
                    super::error::Error::GroveDB(grovedb::Error::PathKeyNotFound(_)) => {
                        assert!(true)
                    }
                    _ => assert!(false, "invalid error type"),
                },
            }
        }
    }
}
