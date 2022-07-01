use grovedb::{Element, PathQuery, Query, SizedQuery, TransactionArg};

use crate::drive::object_size_info::{KeyInfo, PathKeyElementInfo};
use crate::{
    error,
    error::{drive::DriveError, fee::FeeError, Error},
    fee::pools::epoch::epoch_pool::EpochPool,
};

use super::constants;

impl<'e> EpochPool<'e> {
    pub fn get_proposer_block_count(
        &self,
        proposer_tx_hash: &[u8; 32],
        transaction: TransactionArg,
    ) -> Result<u64, Error> {
        let element = self
            .drive
            .grove
            .get(self.get_proposers_path(), proposer_tx_hash, transaction)
            // TODO: Shouldn't we wrap all errors to Fee Pool errors?
            //  in this case we know the source of error
            .unwrap()
            .map_err(Error::GroveDB)?;

        if let Element::Item(item, _) = element {
            Ok(u64::from_le_bytes(item.as_slice().try_into().map_err(
                |_| {
                    Error::Fee(FeeError::CorruptedProposerBlockCountItemLength(
                        "epoch proposer block count item have an invalid length",
                    ))
                },
            )?))
        } else {
            Err(Error::Fee(FeeError::CorruptedProposerBlockCountNotItem(
                "epoch proposer block count must be an item",
            )))
        }
    }

    pub fn update_proposer_block_count(
        &self,
        proposer_pro_tx_hash: &[u8; 32],
        block_count: u64,
    ) -> Result<(), Error> {
        self.drive
            .current_batch_insert(PathKeyElementInfo::PathFixedSizeKeyElement((
                self.get_proposers_path(),
                proposer_pro_tx_hash,
                Element::Item(block_count.to_le_bytes().to_vec(), None),
            )))
    }

    pub fn increment_proposer_block_count(
        &self,
        proposer_pro_tx_hash: &[u8; 32],
        transaction: TransactionArg,
    ) -> Result<(), Error> {
        // update proposer's block count
        let proposed_block_count = self
            .get_proposer_block_count(proposer_pro_tx_hash, transaction)
            .or_else(|e| match e {
                error::Error::GroveDB(grovedb::Error::PathKeyNotFound(_)) => Ok(0u64),
                _ => Err(e),
            })?;

        self.update_proposer_block_count(&proposer_pro_tx_hash, proposed_block_count + 1)?;

        Ok(())
    }

    pub fn is_proposers_tree_empty(&self, transaction: TransactionArg) -> Result<bool, Error> {
        match self
            .drive
            .grove
            .is_empty_tree(self.get_proposers_path(), transaction)
            .unwrap()
        {
            Ok(result) => Ok(result),
            Err(err) => match err {
                grovedb::Error::PathNotFound(_) => Ok(true),
                _ => Err(Error::Drive(DriveError::CorruptedCodeExecution(
                    "internal grovedb error",
                ))),
            },
        }
    }

    pub fn init_proposers(&self) -> Result<(), Error> {
        self.drive.current_batch_insert_empty_tree(
            self.get_path(),
            KeyInfo::KeyRef(constants::KEY_PROPOSERS.as_bytes()),
            None,
        )
    }

    pub fn get_proposers(
        &self,
        limit: u16,
        transaction: TransactionArg,
    ) -> Result<Vec<(Vec<u8>, u64)>, Error> {
        let path_as_vec: Vec<Vec<u8>> = self
            .get_proposers_path()
            .iter()
            .map(|slice| slice.to_vec())
            .collect();

        let mut query = Query::new();
        query.insert_all();

        let path_query = PathQuery::new(path_as_vec, SizedQuery::new(query, Some(limit), None));

        let (elements, _) = self
            .drive
            .grove
            .query_raw(&path_query, transaction)
            .unwrap()
            .map_err(Error::GroveDB)?;

        let result = elements
            .into_iter()
            .map(|(pro_tx_hash, element)| {
                if let Element::Item(item, _) = element {
                    let block_count =
                        u64::from_le_bytes(item.as_slice().try_into().map_err(|_| {
                            Error::Fee(FeeError::CorruptedProposerBlockCountItemLength(
                                "epoch proposer block count item have an invalid length",
                            ))
                        })?);

                    Ok((pro_tx_hash, block_count))
                } else {
                    Err(Error::Fee(FeeError::CorruptedProposerBlockCountNotItem(
                        "epoch proposer block count must be an item",
                    )))
                }
            })
            .collect::<Result<Vec<(Vec<u8>, u64)>, Error>>()?;

        Ok(result)
    }

    pub fn delete_proposers(&self, transaction: TransactionArg) -> Result<(), Error> {
        self.drive.current_batch_delete(
            self.get_path(),
            constants::KEY_PROPOSERS.as_bytes(),
            true,
            transaction,
        )
    }
}

#[cfg(test)]
mod tests {
    use grovedb::Element;

    use crate::{
        error::{self, fee::FeeError},
        fee::pools::{
            epoch::epoch_pool::EpochPool,
            tests::helpers::setup::{setup_drive, setup_fee_pools},
        },
    };

    mod get_proposer_block_count {
        use crate::drive::object_size_info::PathKeyElementInfo;

        #[test]
        fn test_error_if_value_has_invalid_length() {
            let drive = super::setup_drive();
            let (transaction, _) = super::setup_fee_pools(&drive, None);

            let pro_tx_hash: [u8; 32] = rand::random();

            let epoch = super::EpochPool::new(0, &drive);

            drive
                .start_current_batch()
                .expect("should start current batch");

            epoch.init_proposers().expect("should init proposers");

            drive
                .current_batch_insert(PathKeyElementInfo::PathFixedSizeKeyElement((
                    epoch.get_proposers_path(),
                    &pro_tx_hash,
                    super::Element::Item(u128::MAX.to_le_bytes().to_vec(), None),
                )))
                .expect("should insert invalid value");

            drive
                .apply_current_batch(true, Some(&transaction))
                .expect("should apply batch");

            match epoch.get_proposer_block_count(&pro_tx_hash, Some(&transaction)) {
                Ok(_) => assert!(false, "should not be able to decode stored value"),
                Err(e) => match e {
                    super::error::Error::Fee(
                        super::FeeError::CorruptedProposerBlockCountItemLength(_),
                    ) => {
                        assert!(true)
                    }
                    _ => assert!(false, "invalid error type"),
                },
            }
        }

        #[test]
        fn test_error_if_epoch_pool_is_not_initiated() {
            let drive = super::setup_drive();
            let (transaction, _) = super::setup_fee_pools(&drive, None);

            let pro_tx_hash: [u8; 32] = rand::random();

            let epoch = super::EpochPool::new(7000, &drive);

            match epoch.get_proposer_block_count(&pro_tx_hash, Some(&transaction)) {
                Ok(_) => assert!(
                    false,
                    "should not be able to get proposer block count on uninit epoch pool"
                ),
                Err(e) => match e {
                    super::error::Error::GroveDB(grovedb::Error::PathNotFound(_)) => {
                        assert!(true)
                    }
                    _ => assert!(false, "invalid error type"),
                },
            }
        }
    }

    mod update_proposer_block_count {
        #[test]
        fn test_value_is_set() {
            let drive = super::setup_drive();
            let (transaction, _) = super::setup_fee_pools(&drive, None);

            let pro_tx_hash: [u8; 32] = rand::random();
            let block_count = 42;

            let epoch = super::EpochPool::new(0, &drive);

            drive
                .start_current_batch()
                .expect("should start current batch");

            epoch.init_proposers().expect("should init proposers");

            epoch
                .update_proposer_block_count(&pro_tx_hash, block_count)
                .expect("should update proposer block count");

            drive
                .apply_current_batch(true, Some(&transaction))
                .expect("should apply batch");

            let stored_block_count = epoch
                .get_proposer_block_count(&pro_tx_hash, Some(&transaction))
                .expect("should get proposer block count");

            assert_eq!(stored_block_count, block_count);
        }
    }

    mod increment_proposer_block_count {
        #[test]
        fn test_value_is_set_if_epoch_is_not_initialized() {
            let drive = super::setup_drive();
            let (transaction, _) = super::setup_fee_pools(&drive, None);

            let pro_tx_hash: [u8; 32] = rand::random();

            let epoch = super::EpochPool::new(0, &drive);

            drive
                .start_current_batch()
                .expect("should start current batch");

            epoch.init_proposers().expect("should init proposers");

            // Apply proposers tree
            drive
                .apply_current_batch(true, Some(&transaction))
                .expect("should apply batch");

            drive
                .start_current_batch()
                .expect("should start current batch");

            epoch
                .increment_proposer_block_count(&pro_tx_hash, Some(&transaction))
                .expect("should update proposer block count");

            drive
                .apply_current_batch(true, Some(&transaction))
                .expect("should apply batch");

            let stored_block_count = epoch
                .get_proposer_block_count(&pro_tx_hash, Some(&transaction))
                .expect("should get proposer block count");

            assert_eq!(stored_block_count, 1);
        }

        #[test]
        fn test_value_is_incremented() {
            let drive = super::setup_drive();
            let (transaction, _) = super::setup_fee_pools(&drive, None);

            let pro_tx_hash: [u8; 32] = rand::random();

            let epoch = super::EpochPool::new(0, &drive);

            drive
                .start_current_batch()
                .expect("should start current batch");

            epoch.init_proposers().expect("should init proposers");

            // Apply proposers tree
            drive
                .apply_current_batch(true, Some(&transaction))
                .expect("should apply batch");

            drive
                .start_current_batch()
                .expect("should start current batch");

            epoch
                .update_proposer_block_count(&pro_tx_hash, 1)
                .expect("should update proposer block count");

            // Apply proposer block count
            drive
                .apply_current_batch(true, Some(&transaction))
                .expect("should apply batch");

            drive
                .start_current_batch()
                .expect("should start current batch");

            epoch
                .increment_proposer_block_count(&pro_tx_hash, Some(&transaction))
                .expect("should update proposer block count");

            drive
                .apply_current_batch(true, Some(&transaction))
                .expect("should apply batch");

            let stored_block_count = epoch
                .get_proposer_block_count(&pro_tx_hash, Some(&transaction))
                .expect("should get proposer block count");

            assert_eq!(stored_block_count, 2);
        }
    }

    mod is_empty_tree {
        #[test]
        fn test_check_if_empty() {
            let drive = super::setup_drive();
            let (transaction, _) = super::setup_fee_pools(&drive, None);

            let epoch = super::EpochPool::new(0, &drive);

            let result = epoch
                .is_proposers_tree_empty(Some(&transaction))
                .expect("should check if tree is empty");

            assert_eq!(result, true);
        }
    }

    mod get_proposers {
        #[test]
        fn test_value() {
            let drive = super::setup_drive();
            let (transaction, _) = super::setup_fee_pools(&drive, None);

            let pro_tx_hash: [u8; 32] = rand::random();
            let block_count = 42;

            let epoch = super::EpochPool::new(0, &drive);

            drive
                .start_current_batch()
                .expect("should start current batch");

            epoch.init_proposers().expect("should init proposers");

            epoch
                .update_proposer_block_count(&pro_tx_hash, block_count)
                .expect("should update proposer block count");

            drive
                .apply_current_batch(true, Some(&transaction))
                .expect("should apply batch");

            let result = epoch
                .get_proposers(100, Some(&transaction))
                .expect("should get proposers");

            assert_eq!(result, vec!((pro_tx_hash.to_vec(), block_count)));
        }
    }

    mod delete_proposers {
        use crate::fee::pools::epoch::constants;
        use costs::CostResult;
        use grovedb::{Element, Error};

        #[test]
        fn test_values_has_been_deleted() {
            let drive = super::setup_drive();
            let (transaction, _) = super::setup_fee_pools(&drive, None);

            let pro_tx_hash: [u8; 32] = rand::random();
            let block_count = 42;

            let epoch = super::EpochPool::new(0, &drive);

            drive
                .start_current_batch()
                .expect("should start current batch");

            epoch.init_proposers().expect("should init proposers");

            // Apply proposers tree
            drive
                .apply_current_batch(true, Some(&transaction))
                .expect("should apply batch");

            drive
                .start_current_batch()
                .expect("should start current batch");

            epoch
                .delete_proposers(Some(&transaction))
                .expect("should delete proposers");

            drive
                .apply_current_batch(true, Some(&transaction))
                .expect("should apply batch");

            match drive
                .grove
                .get(
                    epoch.get_path(),
                    constants::KEY_PROPOSERS.as_bytes(),
                    Some(&transaction),
                )
                .unwrap()
            {
                Ok(_) => assert!(false, "expect tree not exists"),
                Err(e) => match e {
                    Error::PathKeyNotFound(_) => assert!(true),
                    _ => assert!(false, "invalid error type"),
                },
            }
        }
    }
}
