use grovedb::{Element, PathQuery, Query, SizedQuery, TransactionArg};

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
        transaction: TransactionArg,
    ) -> Result<(), Error> {
        self.drive
            .grove
            .insert(
                self.get_proposers_path(),
                proposer_pro_tx_hash,
                Element::Item(block_count.to_le_bytes().to_vec(), None),
                transaction,
            )
            .map_err(Error::GroveDB)
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

        self.update_proposer_block_count(
            &proposer_pro_tx_hash,
            proposed_block_count + 1,
            transaction,
        )?;

        Ok(())
    }

    pub fn is_proposers_tree_empty(&self, transaction: TransactionArg) -> Result<bool, Error> {
        match self
            .drive
            .grove
            .is_empty_tree(self.get_proposers_path(), transaction)
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

    pub fn init_proposers(&self, transaction: TransactionArg) -> Result<(), Error> {
        self.drive
            .grove
            .insert(
                self.get_path(),
                constants::KEY_PROPOSERS.as_bytes(),
                Element::empty_tree(),
                transaction,
            )
            .map_err(Error::GroveDB)?;

        Ok(())
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

        let path_queries = [&path_query];

        let elements = self
            .drive
            .grove
            .get_path_queries_raw(&path_queries, transaction)
            .map_err(Error::GroveDB)?;

        let result = elements
            .into_iter()
            .map(|(pro_tx_hash, e)| {
                if let Element::Item(item, _) = e {
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
        self.drive
            .grove
            .delete(
                self.get_path(),
                constants::KEY_PROPOSERS.as_bytes(),
                transaction,
            )
            .map_err(Error::GroveDB)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use grovedb::Element;
    use tempfile::TempDir;

    use crate::{
        drive::Drive,
        error::{self, fee::FeeError},
        fee::pools::{
            epoch::{constants, epoch_pool::EpochPool},
            fee_pools::FeePools,
        },
    };

    #[test]
    fn test_update_and_get_proposer_block_count() {
        let tmp_dir = TempDir::new().unwrap();
        let drive: Drive = Drive::open(tmp_dir).expect("expected to open Drive successfully");

        drive
            .create_root_tree(None)
            .expect("expected to create root tree successfully");

        let transaction = drive.grove.start_transaction();

        let fee_pools = FeePools::new();

        fee_pools
            .init(&drive, Some(&transaction))
            .expect("fee pools to init");

        let pro_tx_hash: [u8; 32] =
            hex::decode("0101010101010101010101010101010101010101010101010101010101010101")
                .expect("to decode pro tx hash")
                .try_into()
                .expect("to convert vector to array of 32 bytes");

        let epoch = EpochPool::new(7000, &drive);

        match epoch.get_proposer_block_count(&pro_tx_hash, Some(&transaction)) {
            Ok(_) => assert!(
                false,
                "should not be able to get proposer block count on uninit epoch pool"
            ),
            Err(e) => match e {
                error::Error::GroveDB(grovedb::Error::PathNotFound(_)) => assert!(true),
                _ => assert!(false, "invalid error type"),
            },
        }

        match epoch.update_proposer_block_count(&pro_tx_hash, 1, Some(&transaction)) {
            Ok(_) => assert!(
                false,
                "should not be able to update proposer block count on uninit epoch pool"
            ),
            Err(e) => match e {
                error::Error::GroveDB(grovedb::Error::InvalidPath(_)) => assert!(true),
                _ => assert!(false, "invalid error type"),
            },
        }

        let epoch = EpochPool::new(42, &drive);

        let is_proposers_tree_empty = epoch
            .is_proposers_tree_empty(Some(&transaction))
            .expect("to check if proposer tree epmty");

        assert_eq!(is_proposers_tree_empty, true);

        epoch
            .init_proposers(Some(&transaction))
            .expect("to init proposers tree");

        let block_count = 42;

        epoch
            .update_proposer_block_count(&pro_tx_hash, block_count, Some(&transaction))
            .expect("to update proposer block count");

        let stored_block_count = epoch
            .get_proposer_block_count(&pro_tx_hash, Some(&transaction))
            .expect("to get proposer block count");

        assert_eq!(stored_block_count, block_count);

        drive
            .grove
            .insert(
                epoch.get_proposers_path(),
                &pro_tx_hash,
                Element::Item(u128::MAX.to_le_bytes().to_vec(), None),
                Some(&transaction),
            )
            .expect("to insert invalid data");

        match epoch.get_proposer_block_count(&pro_tx_hash, Some(&transaction)) {
            Ok(_) => assert!(false, "should not be able to decode stored value"),
            Err(e) => match e {
                error::Error::Fee(FeeError::CorruptedProposerBlockCountItemLength(_)) => {
                    assert!(true)
                }
                _ => assert!(false, "invalid error type"),
            },
        }
    }

    #[test]
    fn test_get_proposers() {
        let tmp_dir = TempDir::new().unwrap();
        let drive: Drive = Drive::open(tmp_dir).expect("expected to open Drive successfully");

        drive
            .create_root_tree(None)
            .expect("expected to create root tree successfully");

        let transaction = drive.grove.start_transaction();

        let fee_pools = FeePools::new();

        fee_pools
            .init(&drive, Some(&transaction))
            .expect("fee pools to init");

        let epoch = EpochPool::new(42, &drive);

        epoch
            .init_proposers(Some(&transaction))
            .expect("to init proposers tree");

        let pro_tx_hash: [u8; 32] =
            hex::decode("0101010101010101010101010101010101010101010101010101010101010101")
                .expect("to decode pro tx hash")
                .try_into()
                .expect("to convert vector to array of 32 bytes");

        let block_count = 42;

        epoch
            .update_proposer_block_count(&pro_tx_hash, block_count, Some(&transaction))
            .expect("to update proposer block count");

        let result = epoch
            .get_proposers(100, Some(&transaction))
            .expect("to get proposers");

        assert_eq!(result, vec!((pro_tx_hash.to_vec(), block_count)));
    }
}
