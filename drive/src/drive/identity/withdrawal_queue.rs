use ciborium::value::Value as CborValue;
use std::collections::BTreeMap;
use std::ops::RangeFull;

use dpp::util::cbor_value::{CborBTreeMapHelper, CborCanonicalMap};
use grovedb::query_result_type::QueryResultType::QueryElementResultType;
use grovedb::{Element, PathQuery, Query, QueryItem, SizedQuery, TransactionArg};

use crate::drive::batch::GroveDbOpBatch;
use crate::drive::{Drive, RootTree};
use crate::error::drive::DriveError;
use crate::error::identity::WithdrawalError;
use crate::error::Error;
use crate::fee::calculate_fee;
use crate::fee::op::DriveOperation;

const QUERY_LIMIT: u16 = 16;

#[derive(Debug)]
pub struct Withdrawal {
    pub id: u128,
    pub index: u64,
    pub fee: u32,
    pub request_height: u32,
    pub quorum_hash: Vec<u8>,
    pub quorum_sig: Vec<u8>,
    pub tx_out_hash: Vec<u8>,
}

impl Withdrawal {
    pub fn to_cbor(&self) -> Result<Vec<u8>, Error> {
        let mut map = CborCanonicalMap::new();

        map.insert("id", self.id);
        map.insert("index", self.index);
        map.insert("fee", self.fee);
        map.insert("request_height", self.request_height);
        map.insert("quorum_hash", self.quorum_hash.clone());
        map.insert("quorum_sig", self.quorum_sig.clone());
        map.insert("tx_out_hash", self.tx_out_hash.clone());

        map.to_bytes().map_err(|_| {
            Error::Drive(DriveError::CorruptedCodeExecution(
                "Could not seriazlie withdrawal to CBOR",
            ))
        })
    }

    pub fn from_cbor(cbor: &[u8]) -> Result<Self, Error> {
        let map: BTreeMap<String, CborValue> = ciborium::de::from_reader(cbor).map_err(|_| {
            Error::Drive(DriveError::CorruptedCodeExecution(
                "Could not read withdrawal CBOR map",
            ))
        })?;

        let id: u128 = map.get_u128("id").map_err(|_| {
            Error::Drive(DriveError::CorruptedCodeExecution(
                "Could not read id from CBOR map",
            ))
        })?;

        let index: u64 = map.get_u64("index").map_err(|_| {
            Error::Drive(DriveError::CorruptedCodeExecution(
                "Could not read index from CBOR map",
            ))
        })?;

        let fee: u32 = map.get_u32("fee").map_err(|_| {
            Error::Drive(DriveError::CorruptedCodeExecution(
                "Could not read fee from CBOR map",
            ))
        })?;

        let request_height: u32 = map.get_u32("request_height").map_err(|_| {
            Error::Drive(DriveError::CorruptedCodeExecution(
                "Could not read request_height from CBOR map",
            ))
        })?;

        let quorum_hash: Vec<u8> = map
            .get("quorum_hash")
            .ok_or_else(|| {
                Error::Drive(DriveError::CorruptedCodeExecution(
                    "could not get quorum_hash from cbor",
                ))
            })?
            .as_bytes()
            .ok_or_else(|| {
                Error::Drive(DriveError::CorruptedCodeExecution(
                    "could not convert quorum_hash to Vec<u8>",
                ))
            })?
            .clone();

        let quorum_sig: Vec<u8> = map
            .get("quorum_sig")
            .ok_or_else(|| {
                Error::Drive(DriveError::CorruptedCodeExecution(
                    "could not get quorum_sig from cbor",
                ))
            })?
            .as_bytes()
            .ok_or_else(|| {
                Error::Drive(DriveError::CorruptedCodeExecution(
                    "could not convert quorum_sig to Vec<u8>",
                ))
            })?
            .clone();

        let tx_out_hash: Vec<u8> = map
            .get("tx_out_hash")
            .ok_or_else(|| {
                Error::Drive(DriveError::CorruptedCodeExecution(
                    "could not get tx_out_hash from cbor",
                ))
            })?
            .as_bytes()
            .ok_or_else(|| {
                Error::Drive(DriveError::CorruptedCodeExecution(
                    "could not convert tx_out_hash to Vec<u8>",
                ))
            })?
            .clone();

        Ok(Self {
            id,
            index,
            fee,
            request_height,
            quorum_hash,
            quorum_sig,
            tx_out_hash,
        })
    }
}

impl Drive {
    pub fn enqueue_withdrawal(
        &self,
        withdrawal: Withdrawal,
        transaction: TransactionArg,
    ) -> Result<(i64, u64), Error> {
        let mut batch = GroveDbOpBatch::new();

        batch.add_insert(
            vec![vec![RootTree::Withdrawals as u8]],
            withdrawal.id.to_be_bytes().to_vec(),
            Element::Item(withdrawal.to_cbor()?, None),
        );

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
            .query_raw(&path_query, QueryElementResultType, transaction)
            .unwrap()
            .map_err(Error::GroveDB)?;

        let result: Result<Vec<Withdrawal>, Error> = result_items
            .to_elements()
            .into_iter()
            .map(|element| {
                if let Element::Item(cbor, _) = element {
                    let withdrawal = Withdrawal::from_cbor(cbor.as_slice()).map_err(|_| {
                        Error::Withdrawal(WithdrawalError::WithdrawalSerialization(
                            "failed to de-serialize withdrawal from CBOR",
                        ))
                    })?;

                    Ok(withdrawal)
                } else {
                    Err(Error::Drive(DriveError::CorruptedWithdrawalNotItem(
                        "withdrawal must be an item",
                    )))
                }
            })
            .collect();

        let withdrawals = result?;

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

    #[test]
    fn test_enqueue_and_dequeue() {
        let drive = setup_drive_with_initial_state_structure();

        let transaction = drive.grove.start_transaction();

        for i in 0..17 {
            let withdrawal = Withdrawal {
                id: i,
                index: 1,
                fee: 1,
                request_height: 1,
                quorum_hash: vec![],
                quorum_sig: vec![],
                tx_out_hash: vec![],
            };

            drive
                .enqueue_withdrawal(withdrawal, Some(&transaction))
                .expect("to enqueue withdrawal");
        }

        let withdrawals = drive
            .dequeue_withdrawals(Some(&transaction))
            .expect("to dequeue withdrawals");

        assert_eq!(withdrawals.len(), 16);

        let withdrawals = drive
            .dequeue_withdrawals(Some(&transaction))
            .expect("to dequeue withdrawals");

        assert_eq!(withdrawals.len(), 1);
    }
}
