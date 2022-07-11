use crate::drive::{Drive, RootTree};
use crate::error::drive::DriveError;
use crate::error::Error;
use grovedb::{Element, TransactionArg};
use std::array::TryFromSliceError;
use grovedb::batch::{GroveDbOp, Op};

const KEY_GENESIS_TIME: &[u8; 1] = b"g";

impl Drive {
    pub fn get_genesis_time(&self, transaction: TransactionArg) -> Result<Option<u64>, Error> {
        let element = self
            .grove
            .get(
                [Into::<&[u8; 1]>::into(RootTree::SpentAssetLockTransactions).as_slice()],
                KEY_GENESIS_TIME.as_slice(),
                transaction,
            )
            .unwrap()
            .map(Some)
            .or_else(|e| match e {
                Error::GroveDB(grovedb::Error::PathKeyNotFound(_)) => Ok(None),
                _ => Err(e),
            })?;

        if let Element::Item(item, _) = element {
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

    pub fn init_genesis(&self,
                        genesis_time: u64,
                        transaction: TransactionArg,
    ) {
        let op = self.update_genesis_time_operation(genesis_time)?;

        self.grove_apply_batch_with_add_costs(vec![op], false, transaction)?;

        request.block_time
    }

    pub fn update_genesis_time_operation(
        genesis_time: u64,
    ) -> GroveDbOp {
        GroveDbOp {
            path: vec![vec![RootTree::SpentAssetLockTransactions as u8]],
            key: KEY_GENESIS_TIME.to_vec(),
            //todo make this into a Op::Replace
            op: Op::Insert {
                element: Element::Item(genesis_time.to_be_bytes().to_vec(), None)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::drive::genesis_time::KEY_GENESIS_TIME;
    use crate::drive::RootTree;
    use crate::error;
    use crate::fee::pools::tests::helpers::setup::setup_drive;
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
                    super::Element::Item(u128::MAX.to_le_bytes().to_vec(), None),
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
        use crate::drive::storage::batch::GroveDbOpBatch;

        #[test]
        fn test_error_if_fee_pools_is_not_initiated() {
            let drive = super::setup_drive();

            let genesis_time: u64 = 1655396517902;

            let mut batch = GroveDbOpBatch::new(&drive);

            drive
                .add_update_genesis_time_operations(&mut batch, genesis_time)
                .expect("should update genesis time");

            match drive.apply_batch(batch, false, None) {
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
            let drive = super::setup_drive();

            drive
                .create_initial_state_structure(None)
                .expect("expected to create root tree successfully");

            let genesis_time: u64 = 1655396517902;

            let mut batch = GroveDbOpBatch::new(&drive);

            drive
                .add_update_genesis_time_operations(&mut batch, genesis_time)
                .expect("should update genesis time");

            drive
                .apply_batch(batch, false, None)
                .expect("should apply batch");

            let stored_genesis_time = drive
                .get_genesis_time(None)
                .expect("should get genesis time");

            assert_eq!(stored_genesis_time, genesis_time);
        }
    }
}
