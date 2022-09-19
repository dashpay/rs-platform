use ciborium::value::Value as CborValue;
use dashcore::Transaction;
use std::collections::BTreeMap;
use std::convert::TryInto;
use std::ops::RangeFull;

use dpp::util::cbor_value::{CborBTreeMapHelper, CborCanonicalMap};
use grovedb::{Element, PathQuery, Query, QueryItem, SizedQuery, TransactionArg};

use crate::drive::batch::GroveDbOpBatch;
use crate::drive::{Drive, RootTree};
use crate::error::drive::DriveError;
use crate::error::identity::WithdrawalError;
use crate::error::Error;
use crate::fee::calculate_fee;
use crate::fee::op::DriveOperation;

const QUERY_LIMIT: u16 = 16;

impl Drive {
    pub fn enqueue_withdrawals(
        &self,
        withdrawals: Vec<Transaction>,
        transaction: TransactionArg,
    ) -> Result<(i64, u64), Error> {
        let mut batch = GroveDbOpBatch::new();

        for withdrawal in withdrawals {
            batch.add_insert(
                vec![vec![RootTree::Withdrawals as u8]],
                withdrawal.id.to_be_bytes().to_vec(),
                Element::Item(withdrawal.to_cbor()?, None),
            );
        }

        let mut drive_operations: Vec<DriveOperation> = vec![];

        self.apply_batch_grovedb_operations(true, transaction, batch, &mut drive_operations)?;

        calculate_fee(None, Some(drive_operations))
    }

    pub fn dequeue_withdrawals(
        &self,
        transaction: TransactionArg,
    ) -> Result<Vec<Withdrawal>, Error> {
        let mut query = Query::new();

        query.insert_item(QueryItem::RangeFull(RangeFull));

        let path_query = PathQuery {
            path: vec![vec![RootTree::Withdrawals as u8]],
            query: SizedQuery {
                query,
                limit: Some(QUERY_LIMIT),
                offset: None,
            },
        };

        let (result_items, _) = self
            .grove
            .query(&path_query, transaction)
            .unwrap()
            .map_err(Error::GroveDB)?;

        let withdrawals = result_items
            .into_iter()
            .map(|cbor| {
                Withdrawal::from_cbor(cbor.as_slice()).map_err(|_| {
                    Error::Withdrawal(WithdrawalError::WithdrawalSerialization(
                        "failed to de-serialize withdrawal from CBOR",
                    ))
                })
            })
            .collect::<Result<Vec<Withdrawal>, Error>>()?;

        if withdrawals.len() > 0 {
            let mut batch_operations: Vec<DriveOperation> = vec![];
            let mut drive_operations: Vec<DriveOperation> = vec![];

            let withdrawals_path: [&[u8]; 1] = [Into::<&[u8; 1]>::into(RootTree::Withdrawals)];

            for withdrawal in withdrawals.iter() {
                self.batch_delete(
                    withdrawals_path,
                    &withdrawal.id.to_be_bytes(),
                    true,
                    transaction,
                    &mut batch_operations,
                )?;
            }

            self.apply_batch_drive_operations(
                true,
                transaction,
                batch_operations,
                &mut drive_operations,
            )?;
        }

        Ok(withdrawals)
    }
}

#[cfg(test)]
mod tests {
    use crate::common::helpers::setup::setup_drive_with_initial_state_structure;

    use super::Withdrawal;

    mod queue {
        use super::*;

        #[test]
        fn test_enqueue_and_dequeue() {
            let drive = setup_drive_with_initial_state_structure();

            let transaction = drive.grove.start_transaction();

            let withdrawals: Vec<Withdrawal> = (0..17)
                .map(|i| Withdrawal {
                    id: i,
                    index: 1,
                    fee: 1,
                    request_height: 1,
                    quorum_hash: vec![0; 32].try_into().unwrap(),
                    quorum_sig: vec![0; 96].try_into().unwrap(),
                    tx_out_hash: vec![0; 32].try_into().unwrap(),
                })
                .collect();

            drive
                .enqueue_withdrawals(withdrawals, Some(&transaction))
                .expect("to enqueue withdrawal");

            let withdrawals = drive
                .dequeue_withdrawals(Some(&transaction))
                .expect("to dequeue withdrawals");

            assert_eq!(withdrawals.len(), 16);

            let withdrawals = drive
                .dequeue_withdrawals(Some(&transaction))
                .expect("to dequeue withdrawals");

            assert_eq!(withdrawals.len(), 1);

            let withdrawals = drive
                .dequeue_withdrawals(Some(&transaction))
                .expect("to dequeue withdrawals");

            assert_eq!(withdrawals.len(), 0);
        }
    }
}
