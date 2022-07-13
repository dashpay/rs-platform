use crate::drive::batch::GroveDbOpBatch;
use grovedb::{Element, PathQuery, Query, SizedQuery, TransactionArg};

use crate::drive::Drive;
use crate::error::drive::DriveError;
use crate::error::fee::FeeError;
use crate::error::Error;
use crate::fee_pools::epochs::Epoch;

impl Drive {
    pub(crate) fn get_epochs_proposer_block_count(
        &self,
        epoch_pool: &Epoch,
        proposer_tx_hash: &[u8; 32],
        transaction: TransactionArg,
    ) -> Result<u64, Error> {
        let element = self
            .grove
            .get(
                epoch_pool.get_proposers_path(),
                proposer_tx_hash,
                transaction,
            )
            // TODO: Shouldn't we wrap all errors to Fee Pool errors?
            //  in this case we know the source of error
            .unwrap()
            .map_err(Error::GroveDB)?;

        if let Element::Item(item, _) = element {
            Ok(u64::from_be_bytes(item.as_slice().try_into().map_err(
                |_| {
                    Error::Fee(FeeError::CorruptedProposerBlockCountItemLength(
                        "epochs proposer block count item have an invalid length",
                    ))
                },
            )?))
        } else {
            Err(Error::Fee(FeeError::CorruptedProposerBlockCountNotItem(
                "epochs proposer block count must be an item",
            )))
        }
    }

    pub fn is_epochs_proposers_tree_empty(
        &self,
        epoch_pool: &Epoch,
        transaction: TransactionArg,
    ) -> Result<bool, Error> {
        match self
            .grove
            .is_empty_tree(epoch_pool.get_proposers_path(), transaction)
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

    pub fn get_epochs_proposers(
        &self,
        epoch_pool: &Epoch,
        limit: u16,
        transaction: TransactionArg,
    ) -> Result<Vec<(Vec<u8>, u64)>, Error> {
        let path_as_vec: Vec<Vec<u8>> = epoch_pool
            .get_proposers_path()
            .iter()
            .map(|slice| slice.to_vec())
            .collect();

        let mut query = Query::new();
        query.insert_all();

        let path_query = PathQuery::new(path_as_vec, SizedQuery::new(query, Some(limit), None));

        let (elements, _) = self
            .grove
            .query_raw(&path_query, transaction)
            .unwrap()
            .map_err(Error::GroveDB)?;

        let result = elements
            .into_iter()
            .map(|(pro_tx_hash, element)| {
                if let Element::Item(item, _) = element {
                    let block_count =
                        u64::from_be_bytes(item.as_slice().try_into().map_err(|_| {
                            Error::Fee(FeeError::CorruptedProposerBlockCountItemLength(
                                "epochs proposer block count item have an invalid length",
                            ))
                        })?);

                    Ok((pro_tx_hash, block_count))
                } else {
                    Err(Error::Fee(FeeError::CorruptedProposerBlockCountNotItem(
                        "epochs proposer block count must be an item",
                    )))
                }
            })
            .collect::<Result<Vec<(Vec<u8>, u64)>, Error>>()?;

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use grovedb::Element;

    use crate::error::{self, fee::FeeError};

    use crate::common::tests::helpers::setup::setup_drive;
    use crate::common::tests::helpers::setup::setup_drive_with_initial_state_structure;
    use crate::drive::batch::GroveDbOpBatch;
    use crate::fee_pools::epochs::Epoch;

    mod get_proposer_block_count {

        #[test]
        fn test_error_if_value_has_invalid_length() {
            let drive = super::setup_drive_with_initial_state_structure();
            let transaction = drive.grove.start_transaction();

            let pro_tx_hash: [u8; 32] = rand::random();

            let epoch = super::Epoch::new(0);

            let mut batch = super::GroveDbOpBatch::new();

            batch.push(epoch.init_proposers_tree_operation());

            batch.add_insert(
                epoch.get_proposers_vec_path(),
                pro_tx_hash.to_vec(),
                super::Element::Item(u128::MAX.to_be_bytes().to_vec(), None),
            );

            drive
                .grove_apply_batch(batch, false, Some(&transaction))
                .expect("should apply batch");

            match drive.get_epochs_proposer_block_count(&epoch, &pro_tx_hash, Some(&transaction)) {
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
            let drive = super::setup_drive_with_initial_state_structure();
            let transaction = drive.grove.start_transaction();

            let pro_tx_hash: [u8; 32] = rand::random();

            let epoch = super::Epoch::new(7000);

            match drive.get_epochs_proposer_block_count(&epoch, &pro_tx_hash, Some(&transaction)) {
                Ok(_) => assert!(
                    false,
                    "should not be able to get proposer block count on uninit epochs pool"
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
            let drive = super::setup_drive_with_initial_state_structure();
            let transaction = drive.grove.start_transaction();

            let pro_tx_hash: [u8; 32] = rand::random();
            let block_count = 42;

            let epoch = super::Epoch::new(0);

            let mut batch = super::GroveDbOpBatch::new();

            batch.push(epoch.init_proposers_tree_operation());

            batch.push(epoch.update_proposer_block_count_operation(&pro_tx_hash, block_count));

            drive
                .grove_apply_batch(batch, false, Some(&transaction))
                .expect("should apply batch");

            let stored_block_count = drive
                .get_epochs_proposer_block_count(&epoch, &pro_tx_hash, Some(&transaction))
                .expect("should get proposer block count");

            assert_eq!(stored_block_count, block_count);
        }
    }

    mod increment_proposer_block_count {
        #[test]
        fn test_value_is_set_if_epoch_is_not_initialized() {
            let drive = super::setup_drive_with_initial_state_structure();
            let transaction = drive.grove.start_transaction();

            let pro_tx_hash: [u8; 32] = rand::random();

            let epoch = super::Epoch::new(0);

            let mut batch = super::GroveDbOpBatch::new();

            batch.push(epoch.init_proposers_tree_operation());

            // Apply proposers tree
            drive
                .grove_apply_batch(batch, false, Some(&transaction))
                .expect("should apply batch");

            let mut batch = super::GroveDbOpBatch::new();

            batch.push(
                epoch
                    .increment_proposer_block_count_operation(
                        &drive,
                        &pro_tx_hash,
                        Some(&transaction),
                    )
                    .expect("should increment proposer block count operations"),
            );

            drive
                .grove_apply_batch(batch, false, Some(&transaction))
                .expect("should apply batch");

            let stored_block_count = drive
                .get_epochs_proposer_block_count(&epoch, &pro_tx_hash, Some(&transaction))
                .expect("should get proposer block count");

            assert_eq!(stored_block_count, 1);
        }

        #[test]
        fn test_value_is_incremented() {
            let drive = super::setup_drive_with_initial_state_structure();
            let transaction = drive.grove.start_transaction();

            let pro_tx_hash: [u8; 32] = rand::random();

            let epoch = super::Epoch::new(0);

            let mut batch = super::GroveDbOpBatch::new();

            batch.push(epoch.init_proposers_tree_operation());

            // Apply proposers tree
            drive
                .grove_apply_batch(batch, false, Some(&transaction))
                .expect("should apply batch");

            let mut batch = super::GroveDbOpBatch::new();

            batch.push(epoch.update_proposer_block_count_operation(&pro_tx_hash, 1));

            // Apply proposer block count
            drive
                .grove_apply_batch(batch, false, Some(&transaction))
                .expect("should apply batch");

            let mut batch = super::GroveDbOpBatch::new();

            batch.push(
                epoch
                    .increment_proposer_block_count_operation(
                        &drive,
                        &pro_tx_hash,
                        Some(&transaction),
                    )
                    .expect("should update proposer block count"),
            );

            drive
                .grove_apply_batch(batch, false, Some(&transaction))
                .expect("should apply batch");

            let stored_block_count = drive
                .get_epochs_proposer_block_count(&epoch, &pro_tx_hash, Some(&transaction))
                .expect("should get proposer block count");

            assert_eq!(stored_block_count, 2);
        }
    }

    mod is_empty_tree {
        #[test]
        fn test_check_if_empty() {
            let drive = super::setup_drive_with_initial_state_structure();
            let transaction = drive.grove.start_transaction();

            let epoch = super::Epoch::new(0);

            let result = drive
                .is_epochs_proposers_tree_empty(&epoch, Some(&transaction))
                .expect("should check if tree is empty");

            assert_eq!(result, true);
        }
    }

    mod get_proposers {
        #[test]
        fn test_value() {
            let drive = super::setup_drive_with_initial_state_structure();
            let transaction = drive.grove.start_transaction();

            let pro_tx_hash: [u8; 32] = rand::random();
            let block_count = 42;

            let epoch = super::Epoch::new(0);

            let mut batch = super::GroveDbOpBatch::new();

            batch.push(epoch.init_proposers_tree_operation());

            batch.push(epoch.update_proposer_block_count_operation(&pro_tx_hash, block_count));

            drive
                .grove_apply_batch(batch, false, Some(&transaction))
                .expect("should apply batch");

            let result = drive
                .get_epochs_proposers(&epoch, 100, Some(&transaction))
                .expect("should get proposers");

            assert_eq!(result, vec!((pro_tx_hash.to_vec(), block_count)));
        }
    }

    mod delete_proposers_tree {
        use crate::fee_pools::epochs::epoch_key_constants::KEY_PROPOSERS;

        #[test]
        fn test_values_has_been_deleted() {
            let drive = super::setup_drive_with_initial_state_structure();
            let transaction = drive.grove.start_transaction();

            let epoch = super::Epoch::new(0);

            let mut batch = super::GroveDbOpBatch::new();

            batch.push(epoch.init_proposers_tree_operation());

            // Apply proposers tree
            drive
                .grove_apply_batch(batch, false, Some(&transaction))
                .expect("should apply batch");

            let mut batch = super::GroveDbOpBatch::new();

            batch.push(epoch.delete_proposers_tree_operation());

            drive
                .grove_apply_batch(batch, false, Some(&transaction))
                .expect("should apply batch");

            match drive
                .grove
                .get(
                    epoch.get_path(),
                    KEY_PROPOSERS.as_slice(),
                    Some(&transaction),
                )
                .unwrap()
            {
                Ok(_) => assert!(false, "expect tree not exists"),
                Err(e) => match e {
                    grovedb::Error::PathKeyNotFound(_) => assert!(true),
                    _ => assert!(false, "invalid error type"),
                },
            }
        }
    }

    mod delete_proposers {
        #[test]
        fn test_values_are_being_deleted() {
            let drive = super::setup_drive_with_initial_state_structure();
            let transaction = drive.grove.start_transaction();

            let epoch = super::Epoch::new(0);

            let mut batch = super::GroveDbOpBatch::new();

            batch.push(epoch.init_proposers_tree_operation());

            // Apply proposers tree
            drive
                .grove_apply_batch(batch, false, Some(&transaction))
                .expect("should apply batch");

            let pro_tx_hashes: Vec<[u8; 32]> = (0..10).map(|_| rand::random()).collect();

            let mut batch = super::GroveDbOpBatch::new();

            for pro_tx_hash in pro_tx_hashes.iter() {
                batch.push(epoch.update_proposer_block_count_operation(pro_tx_hash, 1));
            }

            // Apply proposers block count updates
            drive
                .grove_apply_batch(batch, false, Some(&transaction))
                .expect("should apply batch");

            let mut stored_proposers = drive
                .get_epochs_proposers(&epoch, 20, Some(&transaction))
                .expect("should get proposers");

            let mut awaited_result = pro_tx_hashes
                .iter()
                .map(|hash| (hash.to_vec(), 1))
                .collect::<Vec<(Vec<u8>, u64)>>();

            // sort both result to be able to compare them
            stored_proposers.sort();
            awaited_result.sort();

            assert_eq!(stored_proposers, awaited_result);

            let deleted_pro_tx_hashes = vec![
                awaited_result.get(0).unwrap().0.clone(),
                awaited_result.get(1).unwrap().0.clone(),
            ];

            // remove items we deleted
            awaited_result.remove(0);
            awaited_result.remove(1);

            let mut batch = super::GroveDbOpBatch::new();

            epoch.add_delete_proposers_operations(deleted_pro_tx_hashes, &mut batch);

            // Apply proposers deletion
            drive
                .grove_apply_batch(batch, false, Some(&transaction))
                .expect("should apply batch");

            let stored_proposers = drive
                .get_epochs_proposers(&epoch, 20, Some(&transaction))
                .expect("should get proposers");

            let mut stored_hexes: Vec<String> = stored_proposers
                .iter()
                .map(|(hash, _)| hex::encode(hash))
                .collect();

            let mut awaited_hexes: Vec<String> = stored_proposers
                .iter()
                .map(|(hash, _)| hex::encode(hash))
                .collect();

            stored_hexes.sort();
            awaited_hexes.sort();

            assert_eq!(stored_hexes, awaited_hexes);
        }
    }
}
