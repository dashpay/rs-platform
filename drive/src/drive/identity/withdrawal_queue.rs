// MIT LICENSE
//
// Copyright (c) 2021 Dash Core Group
//
// Permission is hereby granted, free of charge, to any
// person obtaining a copy of this software and associated
// documentation files (the "Software"), to deal in the
// Software without restriction, including without
// limitation the rights to use, copy, modify, merge,
// publish, distribute, sublicense, and/or sell copies of
// the Software, and to permit persons to whom the Software
// is furnished to do so, subject to the following
// conditions:
//
// The above copyright notice and this permission notice
// shall be included in all copies or substantial portions
// of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF
// ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED
// TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A
// PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT
// SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY
// CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION
// OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR
// IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
// DEALINGS IN THE SOFTWARE.
//

//! This module defines functions within the Drive struct related to withdrawal transaction (AssetUnlock)
//!

use std::ops::RangeFull;

use grovedb::query_result_type::QueryResultType::QueryKeyElementPairResultType;
use grovedb::{Element, PathQuery, Query, QueryItem, SizedQuery, TransactionArg};

use crate::drive::batch::GroveDbOpBatch;
use crate::drive::{Drive, RootTree};
use crate::error::drive::DriveError;
use crate::error::Error;
use crate::fee::op::DriveOperation;

impl Drive {
    /// Add insert operations for withdrawal transactions to the batch
    pub fn add_enqueue_withdrawal_transaction_operations(
        &self,
        batch: &mut GroveDbOpBatch,
        withdrawals: Vec<(Vec<u8>, Vec<u8>)>,
    ) -> () {
        for (id, bytes) in withdrawals {
            batch.add_insert(
                vec![vec![RootTree::WithdrawalTransactions as u8]],
                id,
                Element::Item(bytes, None),
            );
        }
    }

    /// Get specified amount of withdrawal transactions from the DB
    pub fn dequeue_withdrawal_transactions(
        &self,
        num_of_transactions: u16,
        transaction: TransactionArg,
    ) -> Result<Vec<(Vec<u8>, Vec<u8>)>, Error> {
        let mut query = Query::new();

        query.insert_item(QueryItem::RangeFull(RangeFull));

        let path_query = PathQuery {
            path: vec![vec![RootTree::WithdrawalTransactions as u8]],
            query: SizedQuery {
                query,
                limit: Some(num_of_transactions),
                offset: None,
            },
        };

        let result_items = self
            .grove
            .query_raw(&path_query, QueryKeyElementPairResultType, transaction)
            .unwrap()
            .map_err(Error::GroveDB)?
            .0
            .to_key_elements();

        let withdrawals = result_items
            .into_iter()
            .map(|(id, element)| match element {
                Element::Item(bytes, _) => Ok((id, bytes)),
                _ => Err(Error::Drive(DriveError::CorruptedWithdrawalNotItem(
                    "withdrawal is not an item",
                ))),
            })
            .collect::<Result<Vec<(Vec<u8>, Vec<u8>)>, Error>>()?;

        if withdrawals.len() > 0 {
            let mut batch_operations: Vec<DriveOperation> = vec![];
            let mut drive_operations: Vec<DriveOperation> = vec![];

            let withdrawals_path: [&[u8]; 1] =
                [Into::<&[u8; 1]>::into(RootTree::WithdrawalTransactions)];

            for (id, _) in withdrawals.iter() {
                self.batch_delete(
                    withdrawals_path,
                    id,
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

// #[cfg(test)]
// mod tests {
//     use crate::common::helpers::setup::setup_drive_with_initial_state_structure;

//     mod queue {
//         use super::*;

//         #[test]
//         fn test_enqueue_and_dequeue() {
//             let drive = setup_drive_with_initial_state_structure();

//             let transaction = drive.grove.start_transaction();

//             let withdrawals: Vec<Withdrawal> = (0..17)
//                 .map(|i| Withdrawal {
//                     id: i,
//                     index: 1,
//                     fee: 1,
//                     request_height: 1,
//                     quorum_hash: vec![0; 32].try_into().unwrap(),
//                     quorum_sig: vec![0; 96].try_into().unwrap(),
//                     tx_out_hash: vec![0; 32].try_into().unwrap(),
//                 })
//                 .collect();

//             drive
//                 .enqueue_withdrawal_transactions(withdrawals, Some(&transaction))
//                 .expect("to enqueue withdrawal");

//             let withdrawals = drive
//                 .dequeue_withdrawal_transactions(Some(&transaction))
//                 .expect("to dequeue withdrawals");

//             assert_eq!(withdrawals.len(), 16);

//             let withdrawals = drive
//                 .dequeue_withdrawal_transactions(Some(&transaction))
//                 .expect("to dequeue withdrawals");

//             assert_eq!(withdrawals.len(), 1);

//             let withdrawals = drive
//                 .dequeue_withdrawal_transactions(Some(&transaction))
//                 .expect("to dequeue withdrawals");

//             assert_eq!(withdrawals.len(), 0);
//         }
//     }
// }
