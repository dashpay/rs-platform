use grovedb::{Element, TransactionArg};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;

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

    pub fn init(&self, drive: &Drive, transaction: TransactionArg) -> Result<(), Error> {
        // init fee pool subtree
        drive
            .grove
            .insert(
                [],
                FeePools::get_path()[0],
                Element::empty_tree(),
                transaction,
            )
            .unwrap()
            .map_err(Error::GroveDB)?;

        // Update storage credit pool
        drive
            .grove
            .insert(
                FeePools::get_path(),
                constants::KEY_STORAGE_FEE_POOL.as_bytes(),
                Element::Item(0i64.to_le_bytes().to_vec(), None),
                transaction,
            )
            .unwrap()
            .map_err(Error::GroveDB)?;

        // We need to insert 50 years worth of epochs,
        // with 20 epochs per year that's 1000 epochs
        for i in 0..1000 {
            let epoch = EpochPool::new(i, drive);
            epoch.init_empty(transaction)?;
        }

        Ok(())
    }

    pub fn get_genesis_time(
        &self,
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

            Ok(genesis_time)
        } else {
            Err(Error::Fee(FeeError::CorruptedGenesisTimeNotItem(
                "fee pool genesis time must be an item",
            )))
        }
    }

    pub fn update_genesis_time(
        &mut self,
        drive: &Drive,
        genesis_time: i64,
        transaction: TransactionArg,
    ) -> Result<(), Error> {
        drive
            .grove
            .insert(
                FeePools::get_path(),
                constants::KEY_GENESIS_TIME.as_bytes(),
                Element::Item(genesis_time.to_le_bytes().to_vec(), None),
                transaction,
            )
            .unwrap()
            .map_err(Error::GroveDB)?;

        self.genesis_time = Some(genesis_time);

        Ok(())
    }

    pub fn calculate_current_epoch_index(
        &self,
        drive: &Drive,
        block_time: i64,
        previous_block_time: i64,
        transaction: TransactionArg,
    ) -> Result<(u16, bool), Error> {
        let genesis_time = self.get_genesis_time(drive, transaction)?;

        let epoch_change_time = Decimal::from(constants::EPOCH_CHANGE_TIME);
        let block_time = Decimal::from(block_time);
        let genesis_time = Decimal::from(genesis_time);
        let previous_block_time = Decimal::from(previous_block_time);

        let prev_epoch_index = (previous_block_time - genesis_time) / epoch_change_time;
        let prev_epoch_index_floored = prev_epoch_index.floor();

        let epoch_index = (block_time - genesis_time) / epoch_change_time;
        let epoch_index_floored = epoch_index.floor();

        dbg!(epoch_index);

        let is_epoch_change = if epoch_index_floored == dec!(0) {
            true
        } else {
            epoch_index_floored > prev_epoch_index_floored
        };

        let epoch_index: u16 = epoch_index_floored.try_into().map_err(|_| {
            Error::Fee(FeeError::DecimalConversion(
                "can't convert reward to u16 from Decimal",
            ))
        })?;

        Ok((epoch_index, is_epoch_change))
    }

    pub fn shift_current_epoch_pool(
        &self,
        drive: &Drive,
        current_epoch_pool: &EpochPool,
        start_block_height: u64,
        start_block_time: i64,
        multiplier: u64,
        transaction: TransactionArg,
    ) -> Result<(), Error> {
        // create and init next thousandth epoch
        let next_thousandth_epoch = EpochPool::new(current_epoch_pool.index + 1000, drive);
        next_thousandth_epoch.init_empty(transaction)?;

        // init first_proposer_block_height and processing_fee for an epoch
        current_epoch_pool.init_current(
            multiplier,
            start_block_height,
            start_block_time,
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
                .expect("to get storage fee pool");

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
                    .expect("to get storage fee");

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
            let (transaction, fee_pools) = super::setup_fee_pools(
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
            let (transaction, fee_pools) = super::setup_fee_pools(&drive, None);

            drive
                .grove
                .insert(
                    super::FeePools::get_path(),
                    super::constants::KEY_GENESIS_TIME.as_bytes(),
                    super::Element::Item(u128::MAX.to_le_bytes().to_vec(), None),
                    Some(&transaction),
                )
                .unwrap()
                .expect("to insert invalid data");

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

            match fee_pools.update_genesis_time(&drive, genesis_time, Some(&transaction)) {
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

            fee_pools
                .update_genesis_time(&drive, genesis_time, Some(&transaction))
                .expect("to update genesis time");

            let stored_genesis_time = fee_pools
                .get_genesis_time(&drive, Some(&transaction))
                .expect("to get genesis time");

            assert_eq!(stored_genesis_time, genesis_time);
        }
    }

    mod calculate_current_epoch_index {
        #[test]
        fn test_epoch_0() {
            let drive = super::setup_drive();
            let (transaction, mut fee_pools) = super::setup_fee_pools(&drive, None);

            let genesis_time: i64 = 1655396517902;
            let block_time: i64 = 1655396517922;
            let prev_block_time: i64 = 1655396517912;

            fee_pools
                .update_genesis_time(&drive, genesis_time, Some(&transaction))
                .expect("to update genesis time");

            let (epoch_index, is_epoch_change) = fee_pools
                .calculate_current_epoch_index(
                    &drive,
                    block_time,
                    prev_block_time,
                    Some(&transaction),
                )
                .expect("to get current epoch index");

            assert_eq!(epoch_index, 0);
            assert_eq!(is_epoch_change, true);

            let block_time: i64 = 1657125244561;

            let (epoch_index, is_epoch_change) = fee_pools
                .calculate_current_epoch_index(
                    &drive,
                    block_time,
                    prev_block_time,
                    Some(&transaction),
                )
                .expect("to get current epoch index");

            assert_eq!(epoch_index, 1);
            assert_eq!(is_epoch_change, true);
        }

        #[test]
        fn test_epoch_epoch_1() {
            let drive = super::setup_drive();
            let (transaction, mut fee_pools) = super::setup_fee_pools(&drive, None);

            let genesis_time: i64 = 1655396517902;
            let prev_block_time: i64 = 1655396517912;
            let block_time: i64 = 1657125244561;

            fee_pools
                .update_genesis_time(&drive, genesis_time, Some(&transaction))
                .expect("to update genesis time");

            let (epoch_index, is_epoch_change) = fee_pools
                .calculate_current_epoch_index(
                    &drive,
                    block_time,
                    prev_block_time,
                    Some(&transaction),
                )
                .expect("to get current epoch index");

            assert_eq!(epoch_index, 1);
            assert_eq!(is_epoch_change, true);
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

            fee_pools
                .shift_current_epoch_pool(
                    &drive,
                    &current_epoch_pool,
                    start_block_height,
                    start_block_time,
                    multiplier,
                    Some(&transaction),
                )
                .expect("to shift epoch pool");

            let next_thousandth_epoch = EpochPool::new(1000, &drive);

            let storage_fee_pool = next_thousandth_epoch
                .get_storage_fee(Some(&transaction))
                .expect("to get storage fee");

            assert_eq!(storage_fee_pool, dec!(0));

            let stored_start_block_height = current_epoch_pool
                .get_start_block_height(Some(&transaction))
                .expect("to get start block height");

            assert_eq!(stored_start_block_height, start_block_height);

            let stored_start_block_time = current_epoch_pool
                .get_start_time(Some(&transaction))
                .expect("to get start time");

            assert_eq!(stored_start_block_time, start_block_time);

            let stored_multiplier = current_epoch_pool
                .get_fee_multiplier(Some(&transaction))
                .expect("to get fee multiplier");

            assert_eq!(stored_multiplier, multiplier);
        }
    }
}
