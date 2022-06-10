use grovedb::{Element, PathQuery, Query, SizedQuery, TransactionArg};

use crate::{
    error::{drive::DriveError, Error},
    fee::pools::epoch::epoch_pool::EpochPool,
};

use super::constants;

impl<'e> EpochPool<'e> {
    pub fn get_first_proposed_block_height(
        &self,
        transaction: TransactionArg,
    ) -> Result<u64, Error> {
        let element = self
            .drive
            .grove
            .get(
                self.get_path(),
                constants::KEY_FIRST_PROPOSER_BLOCK_HEIGHT.as_bytes(),
                transaction,
            )
            .map_err(Error::GroveDB)?;

        if let Element::Item(item) = element {
            Ok(u64::from_le_bytes(
                item.as_slice().try_into().expect("invalid item length"),
            ))
        } else {
            Err(Error::Drive(DriveError::CorruptedEpochElement(
                "epoch first proposed block height must be an item",
            )))
        }
    }

    pub fn update_first_proposed_block_height(
        &self,
        first_proposer_block_height: u64,
        transaction: TransactionArg,
    ) -> Result<(), Error> {
        self.drive
            .grove
            .insert(
                self.get_path(),
                constants::KEY_FIRST_PROPOSER_BLOCK_HEIGHT.as_bytes(),
                Element::Item(first_proposer_block_height.to_le_bytes().to_vec()),
                transaction,
            )
            .map_err(Error::GroveDB)
    }

    pub fn get_proposer_block_count(
        &self,
        proposer_tx_hash: &[u8; 32],
        transaction: TransactionArg,
    ) -> Result<u64, Error> {
        let element = self
            .drive
            .grove
            .get(self.get_proposers_path(), proposer_tx_hash, transaction)
            .map_err(Error::GroveDB)?;

        if let Element::Item(item) = element {
            Ok(u64::from_le_bytes(
                item.as_slice().try_into().expect("invalid item length"),
            ))
        } else {
            Err(Error::Drive(DriveError::CorruptedEpochElement(
                "epoch proposer block count must be an item",
            )))
        }
    }

    pub fn update_proposer_block_count(
        &self,
        proposer_tx_hash: &[u8; 32],
        block_count: u64,
        transaction: TransactionArg,
    ) -> Result<(), Error> {
        self.drive
            .grove
            .insert(
                self.get_path(),
                proposer_tx_hash,
                Element::Item(block_count.to_le_bytes().to_vec()),
                transaction,
            )
            .map_err(Error::GroveDB)
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

        let path_query = PathQuery::new(
            path_as_vec,
            SizedQuery::new(Query::new(), Some(limit), None),
        );

        let path_queries = [&path_query];

        let elements = self
            .drive
            .grove
            .get_path_queries(&path_queries, transaction)
            .map_err(Error::GroveDB)?;

        let result: Vec<(Vec<u8>, u64)> = elements
            .into_iter()
            .map(|e| {
                (
                    vec![], // TODO: get an actuall tx_hash
                    u64::from_le_bytes(e.try_into().expect("invalid item length")),
                )
            })
            .collect();

        Ok(result)
    }
}
