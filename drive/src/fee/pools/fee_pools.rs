use grovedb::{Element, TransactionArg};

use crate::drive::block::BlockInfo;
use crate::drive::object_size_info::{KeyInfo, PathKeyElementInfo};
use crate::drive::{Drive, RootTree};
use crate::error::fee::FeeError;
use crate::error::Error;
use crate::fee::pools::storage_fee_distribution_pool::StorageFeeDistributionPool;

use super::constants;
use super::epoch::epoch_pool::EpochPool;

pub struct FeePools {
    pub genesis_time: Option<i64>,
    pub storage_fee_distribution_pool: StorageFeeDistributionPool,
}

impl FeePools {
    pub fn new() -> FeePools {
        FeePools {
            genesis_time: None,
            storage_fee_distribution_pool: StorageFeeDistributionPool {},
        }
    }

    pub fn get_path<'a>() -> [&'a [u8]; 1] {
        [Into::<&[u8; 1]>::into(RootTree::Pools)]
    }

    pub fn init(&self, drive: &Drive) -> Result<(), Error> {
        // init fee pool subtree
        drive.current_batch_insert_empty_tree(
            [],
            KeyInfo::KeyRef(FeePools::get_path()[0]),
            None,
        )?;

        // Update storage credit pool
        drive.current_batch_insert(PathKeyElementInfo::PathFixedSizeKeyElement((
            FeePools::get_path(),
            constants::KEY_STORAGE_FEE_POOL.as_bytes(),
            Element::Item(0i64.to_le_bytes().to_vec(), None),
        )))?;

        // We need to insert 50 years worth of epochs,
        // with 20 epochs per year that's 1000 epochs
        for i in 0..1000 {
            let epoch = EpochPool::new(i, drive);
            epoch.init_empty()?;
        }

        Ok(())
    }

    pub fn get_genesis_time(
        &mut self,
        drive: &Drive,
        transaction: TransactionArg,
    ) -> Result<i64, Error> {
        if let Some(genesis_time) = self.genesis_time {
            return Ok(genesis_time);
        }

        let element = drive
            .grove
            .get(
                FeePools::get_path(),
                constants::KEY_GENESIS_TIME.as_bytes(),
                transaction,
            )
            .unwrap()
            .map_err(Error::GroveDB)?;

        if let Element::Item(item, _) = element {
            let genesis_time = i64::from_le_bytes(item.as_slice().try_into().map_err(|_| {
                Error::Fee(FeeError::CorruptedGenesisTimeInvalidItemLength(
                    "genesis time item have an invalid length",
                ))
            })?);

            self.genesis_time = Some(genesis_time);

            Ok(genesis_time)
        } else {
            Err(Error::Fee(FeeError::CorruptedGenesisTimeNotItem(
                "fee pool genesis time must be an item",
            )))
        }
    }

    pub fn update_genesis_time(&mut self, drive: &Drive, genesis_time: i64) -> Result<(), Error> {
        drive.current_batch_insert(PathKeyElementInfo::PathFixedSizeKeyElement((
            FeePools::get_path(),
            constants::KEY_GENESIS_TIME.as_bytes(),
            Element::Item(genesis_time.to_le_bytes().to_vec(), None),
        )))?;

        self.genesis_time = Some(genesis_time);

        Ok(())
    }

    pub fn shift_current_epoch_pool(
        &self,
        drive: &Drive,
        current_epoch_pool: &EpochPool,
        start_block_height: u64,
        start_block_time: i64,
        fee_multiplier: u64,
    ) -> Result<(), Error> {
        // create and init next thousandth epoch
        let next_thousandth_epoch = EpochPool::new(current_epoch_pool.index + 1000, drive);
        next_thousandth_epoch.init_empty()?;

        // init first_proposer_block_height and processing_fee for an epoch
        current_epoch_pool.init_current(fee_multiplier, start_block_height, start_block_time)?;

        Ok(())
    }

    pub fn process_block_fees(
        &self,
        drive: &Drive,
        block_info: &BlockInfo,
        processing_fees: u64,
        storage_fees: i64,
        fee_multiplier: u64,
        transaction: TransactionArg,
    ) -> Result<(), Error> {
        let epoch_info = drive.epoch_info.borrow();

        let current_epoch_pool = EpochPool::new(epoch_info.current_epoch_index, drive);

        if epoch_info.is_epoch_change {
            // make next epoch pool as a current
            // and create one more in future
            self.shift_current_epoch_pool(
                drive,
                &current_epoch_pool,
                block_info.block_height,
                block_info.block_time,
                fee_multiplier,
            )?;

            // distribute accumulated previous epoch storage fees
            if current_epoch_pool.index > 0 {
                self.storage_fee_distribution_pool.distribute(
                    drive,
                    current_epoch_pool.index - 1,
                    transaction,
                )?;
            }

            // We need to apply new epoch tree structure and distributed storage fee
            drive.apply_current_batch(false, transaction)?;

            drive.start_current_batch()?;
        }

        self.distribute_fees_into_pools(
            drive,
            &current_epoch_pool,
            processing_fees,
            storage_fees,
            transaction,
        )?;

        current_epoch_pool
            .increment_proposer_block_count(&block_info.proposer_pro_tx_hash, transaction)?;

        self.distribute_fees_from_unpaid_pools_to_proposers(
            drive,
            epoch_info.current_epoch_index,
            transaction,
        )?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        error::{self, fee::FeeError},
        fee::pools::constants,
        fee::pools::tests::helpers::setup::{setup_drive, setup_fee_pools, SetupFeePoolsOptions},
    };
    use grovedb::Element;

    use super::FeePools;

    mod init {
        use rust_decimal_macros::dec;

        use crate::fee::pools::epoch::epoch_pool::EpochPool;

        #[test]
        fn test_values_are_set() {
            let drive = super::setup_drive();
            let (transaction, fee_pools) = super::setup_fee_pools(&drive, None);

            let storage_fee_pool = fee_pools
                .storage_fee_distribution_pool
                .value(&drive, Some(&transaction))
                .expect("should get storage fee pool");

            assert_eq!(storage_fee_pool, 0i64);
        }

        #[test]
        fn test_epoch_pools_are_init() {
            let drive = super::setup_drive();
            let (transaction, _) = super::setup_fee_pools(&drive, None);

            for epoch_index in 0..1000 {
                let epoch_pool = EpochPool::new(epoch_index, &drive);

                let storage_fee = epoch_pool
                    .get_storage_fee(Some(&transaction))
                    .expect("should get storage fee");

                assert_eq!(storage_fee, dec!(0));
            }

            let epoch_pool = EpochPool::new(1000, &drive); // 1001th epoch pool

            match epoch_pool.get_storage_fee(Some(&transaction)) {
                Ok(_) => assert!(false, "must be an error"),
                Err(e) => match e {
                    super::error::Error::GroveDB(_) => assert!(true),
                    _ => assert!(false, "invalid error type"),
                },
            }
        }
    }

    mod get_genesis_time {
        #[test]
        fn test_error_if_fee_pools_is_not_initiated() {
            let drive = super::setup_drive();
            let (transaction, mut fee_pools) = super::setup_fee_pools(
                &drive,
                Some(super::SetupFeePoolsOptions {
                    init_fee_pools: false,
                }),
            );

            match fee_pools.get_genesis_time(&drive, Some(&transaction)) {
                Ok(_) => assert!(
                    false,
                    "should not be able to get genesis time on uninit fee pools"
                ),
                Err(e) => match e {
                    super::error::Error::GroveDB(grovedb::Error::PathNotFound(_)) => assert!(true),
                    _ => assert!(false, "invalid error type"),
                },
            }
        }

        #[test]
        fn test_error_if_value_has_invalid_length() {
            let drive = super::setup_drive();
            let (transaction, mut fee_pools) = super::setup_fee_pools(&drive, None);

            drive
                .grove
                .insert(
                    super::FeePools::get_path(),
                    super::constants::KEY_GENESIS_TIME.as_bytes(),
                    super::Element::Item(u128::MAX.to_le_bytes().to_vec(), None),
                    Some(&transaction),
                )
                .unwrap()
                .expect("should insert invalid data");

            match fee_pools.get_genesis_time(&drive, Some(&transaction)) {
                Ok(_) => assert!(false, "should not be able to decode stored value"),
                Err(e) => match e {
                    super::error::Error::Fee(
                        super::FeeError::CorruptedGenesisTimeInvalidItemLength(_),
                    ) => {
                        assert!(true)
                    }
                    _ => assert!(false, "ivalid error type"),
                },
            }
        }
    }

    mod update_genesis_time {
        #[test]
        fn test_error_if_fee_pools_is_not_initiated() {
            let drive = super::setup_drive();
            let (transaction, mut fee_pools) = super::setup_fee_pools(
                &drive,
                Some(super::SetupFeePoolsOptions {
                    init_fee_pools: false,
                }),
            );

            let genesis_time: i64 = 1655396517902;

            drive
                .start_current_batch()
                .expect("should start current batch");

            fee_pools
                .update_genesis_time(&drive, genesis_time)
                .expect("should update genesis time");

            match drive.apply_current_batch(true, Some(&transaction)) {
                Ok(_) => assert!(
                    false,
                    "should not be able to update genesis time on uninit fee pools"
                ),
                Err(e) => match e {
                    super::error::Error::GroveDB(grovedb::Error::InvalidPath(_)) => assert!(true),
                    _ => assert!(false, "invalid error type"),
                },
            }
        }

        #[test]
        fn test_value_is_set() {
            let drive = super::setup_drive();
            let (transaction, mut fee_pools) = super::setup_fee_pools(&drive, None);

            let genesis_time: i64 = 1655396517902;

            drive
                .start_current_batch()
                .expect("should start current batch");

            fee_pools
                .update_genesis_time(&drive, genesis_time)
                .expect("should update genesis time");

            drive
                .apply_current_batch(true, Some(&transaction))
                .expect("should apply batch");

            let stored_genesis_time = fee_pools
                .get_genesis_time(&drive, Some(&transaction))
                .expect("should get genesis time");

            match fee_pools.genesis_time {
                None => assert!(false, "genesis_time must be set to FeePools"),
                Some(t) => assert_eq!(t, genesis_time),
            }

            assert_eq!(stored_genesis_time, genesis_time);
        }
    }

    mod shift_current_epoch_pool {
        use rust_decimal_macros::dec;

        use crate::fee::pools::epoch::epoch_pool::EpochPool;

        #[test]
        fn test_values_are_set() {
            let drive = super::setup_drive();
            let (transaction, fee_pools) = super::setup_fee_pools(&drive, None);

            let current_epoch_pool = EpochPool::new(0, &drive);

            let start_block_height = 10;
            let start_block_time = 1655396517912;
            let multiplier = 42;

            drive
                .start_current_batch()
                .expect("should start current batch");

            fee_pools
                .shift_current_epoch_pool(
                    &drive,
                    &current_epoch_pool,
                    start_block_height,
                    start_block_time,
                    multiplier,
                )
                .expect("should shift epoch pool");

            drive
                .apply_current_batch(true, Some(&transaction))
                .expect("should apply batch");

            let next_thousandth_epoch = EpochPool::new(1000, &drive);

            let storage_fee_pool = next_thousandth_epoch
                .get_storage_fee(Some(&transaction))
                .expect("should get storage fee");

            assert_eq!(storage_fee_pool, dec!(0));

            let stored_start_block_height = current_epoch_pool
                .get_start_block_height(Some(&transaction))
                .expect("should get start block height");

            assert_eq!(stored_start_block_height, start_block_height);

            let stored_start_block_time = current_epoch_pool
                .get_start_time(Some(&transaction))
                .expect("should get start time");

            assert_eq!(stored_start_block_time, start_block_time);

            let stored_multiplier = current_epoch_pool
                .get_fee_multiplier(Some(&transaction))
                .expect("should get fee multiplier");

            assert_eq!(stored_multiplier, multiplier);
        }
    }

    use crate::drive::Drive;
    use crate::fee::pools::tests::helpers::fee_pools::create_mn_shares_contract;
    use chrono::Utc;

    mod process_block_fees {
        use crate::drive::block::BlockInfo;
        use crate::fee::epoch;
        use crate::fee::pools::epoch::epoch_pool::EpochPool;
        use chrono::Duration;

        #[test]
        fn test_processing_of_the_first_block_then_new_epoch_and_one_more_block_after() {
            let drive = super::setup_drive();
            let (transaction, mut fee_pools) = super::setup_fee_pools(&drive, None);

            fee_pools.init(&drive).expect("should init fee pools");

            super::create_mn_shares_contract(&drive);

            /*

            Block 1. Epoch 0.

             */

            let block_time = super::Utc::now();

            let processing_fees = 100;
            let storage_fees = 2000;
            let fee_multiplier = 2;

            let block_1_info = BlockInfo {
                block_height: 1,
                block_time: block_time.timestamp_millis(),
                previous_block_time: None,
                proposer_pro_tx_hash: [
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0,
                ],
            };

            fee_pools
                .process_block_fees(
                    &drive,
                    &block_1_info,
                    processing_fees,
                    storage_fees,
                    fee_multiplier,
                    Some(&transaction),
                )
                .expect("should process block 1");

            // Genesis time must be set
            let stored_genesis_time = fee_pools
                .get_genesis_time(&drive, Some(&transaction))
                .expect("should get genesis time");

            assert_eq!(stored_genesis_time, block_1_info.block_time);

            // Fees must be distributed

            let stored_storage_fees = fee_pools
                .storage_fee_distribution_pool
                .value(&drive, Some(&transaction))
                .expect("should get storage fees");

            assert_eq!(stored_storage_fees, storage_fees);

            let epoch_pool_0 = EpochPool::new(0, &drive);

            let stored_processing_fees = epoch_pool_0
                .get_processing_fee(Some(&transaction))
                .expect("should get processing fees");

            assert_eq!(stored_processing_fees, processing_fees);

            // Proposer must be added

            let stored_proposer_block_count = epoch_pool_0
                .get_proposer_block_count(&block_1_info.proposer_pro_tx_hash, Some(&transaction))
                .expect("should get proposer block count");

            assert_eq!(stored_proposer_block_count, 1);

            /*

            Block 2. Epoch 1

             */

            let block_time = block_time + Duration::milliseconds(epoch::EPOCH_CHANGE_TIME + 1);

            let processing_fees = 100;
            let storage_fees = 2000;

            let block_2_info = BlockInfo {
                block_height: 2,
                block_time: block_time.timestamp_millis(),
                previous_block_time: Some(block_1_info.block_time),
                proposer_pro_tx_hash: block_1_info.proposer_pro_tx_hash,
            };

            fee_pools
                .process_block_fees(
                    &drive,
                    &block_2_info,
                    processing_fees,
                    storage_fees,
                    fee_multiplier,
                    Some(&transaction),
                )
                .expect("should process block 2");

            /*

            Block 3. Epoch 1

            */
        }
    }
}
