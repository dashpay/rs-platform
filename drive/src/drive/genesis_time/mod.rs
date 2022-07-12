pub mod operations;

use crate::drive::genesis_time::operations::update_genesis_time_operation;
use crate::drive::{Drive, RootTree};
use crate::error::drive::DriveError;
use crate::error::Error;
use grovedb::{Element, TransactionArg};
use std::array::TryFromSliceError;

const KEY_GENESIS_TIME: &[u8; 1] = b"g";

impl Drive {
    pub fn get_genesis_time(&self, transaction: TransactionArg) -> Result<Option<u64>, Error> {
        let element = self
            .grove
            .get(
                [Into::<&[u8; 1]>::into(RootTree::Pools).as_slice()],
                KEY_GENESIS_TIME.as_slice(),
                transaction,
            )
            .unwrap()
            .map(Some)
            .or_else(|e| match e {
                grovedb::Error::PathKeyNotFound(_) => Ok(None),
                _ => Err(e),
            })?;

        if let Some(Element::Item(item, _)) = element {
            let genesis_time = u64::from_be_bytes(item.as_slice().try_into().map_err(
                |e: TryFromSliceError| {
                    Error::Drive(DriveError::CorruptedGenesisTimeInvalidItemLength(
                        e.to_string(),
                    ))
                },
            )?);

            Ok(Some(genesis_time))
        } else {
            Err(Error::Drive(DriveError::CorruptedGenesisTimeNotItem()))
        }
    }

    pub fn init_genesis(
        &self,
        genesis_time: u64,
        transaction: TransactionArg,
    ) -> Result<(), Error> {
        let op = update_genesis_time_operation(genesis_time);

        self.grove_apply_operation(op, false, transaction)
    }
}

#[cfg(test)]
mod tests {
    use crate::common::tests::helpers::setup::setup_drive;
    use crate::drive::genesis_time::KEY_GENESIS_TIME;
    use crate::drive::RootTree;
    use crate::error;
    use grovedb::Element;

    mod get_genesis_time {
        #[test]
        fn test_error_if_fee_pools_is_not_initiated() {
            let drive = super::setup_drive();

            match drive.get_genesis_time(None) {
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

            drive
                .create_initial_state_structure(None)
                .expect("expected to create root tree successfully");

            drive
                .grove
                .insert(
                    [
                        Into::<&[u8; 1]>::into(super::RootTree::SpentAssetLockTransactions)
                            .as_slice(),
                    ],
                    super::KEY_GENESIS_TIME.as_slice(),
                    super::Element::Item(u128::MAX.to_be_bytes().to_vec(), None),
                    None,
                )
                .unwrap()
                .expect("should insert invalid data");

            match drive.get_genesis_time(None) {
                Ok(_) => assert!(false, "should not be able to decode stored value"),
                Err(e) => match e {
                    super::error::Error::Drive(
                        super::error::drive::DriveError::CorruptedGenesisTimeInvalidItemLength(_),
                    ) => {
                        assert!(true)
                    }
                    _ => assert!(false, "ivalid error type"),
                },
            }
        }
    }

    mod update_genesis_time {
        use crate::common::tests::helpers::setup::setup_drive;
        use crate::drive::batch::GroveDbOpBatch;
        use crate::drive::genesis_time::operations::update_genesis_time_operation;

        #[test]
        fn test_error_if_fee_pools_is_not_initiated() {
            let drive = setup_drive();

            let genesis_time: u64 = 1655396517902;

            let mut batch = GroveDbOpBatch::new();

            batch.push(update_genesis_time_operation(genesis_time));

            match drive.grove_apply_batch(batch, false, None) {
                Ok(_) => assert!(
                    false,
                    "should not be able to update genesis time on uninit fee pools"
                ),
                Err(e) => match e {
                    super::error::Error::GroveDB(grovedb::Error::PathKeyNotFound(_)) => {
                        assert!(true)
                    }
                    _ => assert!(false, "invalid error type"),
                },
            }
        }

        #[test]
        fn test_value_is_set() {
            let drive = setup_drive();

            drive
                .create_initial_state_structure(None)
                .expect("expected to create root tree successfully");

            let genesis_time: u64 = 1655396517902;

            let op = update_genesis_time_operation(genesis_time);

            drive
                .grove_apply_operation(op, false, None)
                .expect("should apply batch");

            let stored_genesis_time = drive
                .get_genesis_time(None)
                .expect("should not have an error getting genesis time")
                .expect("should have a genesis time");

            assert_eq!(stored_genesis_time, genesis_time);
        }
    }
}
