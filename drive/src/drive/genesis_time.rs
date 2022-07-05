use crate::drive::object_size_info::PathKeyElementInfo;
use crate::drive::{Drive, RootTree};
use crate::error::drive::DriveError;
use crate::error::Error;
use grovedb::{Element, TransactionArg};
use std::array::TryFromSliceError;

pub const KEY_GENESIS_TIME: &str = "g";

impl Drive {
    pub fn get_genesis_time(&self, transaction: TransactionArg) -> Result<i64, Error> {
        let element = self
            .grove
            .get(
                [Into::<&[u8; 1]>::into(RootTree::Misc).as_slice()],
                KEY_GENESIS_TIME.as_bytes(),
                transaction,
            )
            .unwrap()
            .map_err(Error::GroveDB)?;

        if let Element::Item(item, _) = element {
            let genesis_time = i64::from_le_bytes(item.as_slice().try_into().map_err(
                |e: TryFromSliceError| {
                    Error::Drive(DriveError::CorruptedGenesisTimeInvalidItemLength(
                        e.to_string(),
                    ))
                },
            )?);

            Ok(genesis_time)
        } else {
            Err(Error::Drive(DriveError::CorruptedGenesisTimeNotItem()))
        }
    }

    pub fn update_genesis_time(&self, genesis_time: i64) -> Result<(), Error> {
        self.current_batch_insert(PathKeyElementInfo::PathFixedSizeKeyElement((
            [Into::<&[u8; 1]>::into(RootTree::Misc)],
            KEY_GENESIS_TIME.as_bytes(),
            Element::Item(genesis_time.to_le_bytes().to_vec(), None),
        )))?;

        Ok(())
    }
}

mod tests {
    use crate::drive::genesis_time::KEY_GENESIS_TIME;
    use crate::drive::Drive;
    use crate::drive::RootTree;
    use crate::error;
    use grovedb::{Element, TransactionArg};
    use tempfile::TempDir;

    mod get_genesis_time {

        #[test]
        fn test_error_if_fee_pools_is_not_initiated() {
            let tmp_dir = super::TempDir::new().unwrap();
            let drive: super::Drive =
                super::Drive::open(tmp_dir).expect("expected to open Drive successfully");

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
            let tmp_dir = super::TempDir::new().unwrap();
            let drive: super::Drive =
                super::Drive::open(tmp_dir).expect("expected to open Drive successfully");

            drive
                .create_root_tree(None)
                .expect("expected to create root tree successfully");

            drive
                .grove
                .insert(
                    [Into::<&[u8; 1]>::into(super::RootTree::Misc).as_slice()],
                    super::KEY_GENESIS_TIME.as_bytes(),
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
        #[test]
        fn test_error_if_fee_pools_is_not_initiated() {
            let tmp_dir = super::TempDir::new().unwrap();
            let drive: super::Drive =
                super::Drive::open(tmp_dir).expect("expected to open Drive successfully");

            let genesis_time: i64 = 1655396517902;

            drive
                .update_genesis_time(genesis_time)
                .expect("should update genesis time");

            match drive.apply_current_batch(true, None) {
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
            let tmp_dir = super::TempDir::new().unwrap();
            let drive: super::Drive =
                super::Drive::open(tmp_dir).expect("expected to open Drive successfully");

            drive
                .create_root_tree(None)
                .expect("expected to create root tree successfully");

            let genesis_time: i64 = 1655396517902;

            drive
                .update_genesis_time(genesis_time)
                .expect("should update genesis time");

            drive
                .apply_current_batch(true, None)
                .expect("should apply batch");

            let stored_genesis_time = drive
                .get_genesis_time(None)
                .expect("should get genesis time");

            assert_eq!(stored_genesis_time, genesis_time);
        }
    }
}
