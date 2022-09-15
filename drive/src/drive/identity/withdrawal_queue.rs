use ciborium::value::Value as CborValue;
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

#[derive(Debug, PartialEq, Eq)]
pub struct Withdrawal {
    pub id: u128,
    pub index: u64,
    pub fee: u32,
    pub request_height: u32,
    pub quorum_hash: [u8; 32],
    pub quorum_sig: [u8; 96],
    pub tx_out_hash: [u8; 32],
}

impl Withdrawal {
    pub fn to_cbor(&self) -> Result<Vec<u8>, Error> {
        let mut map = CborCanonicalMap::new();

        map.insert("id", self.id);
        map.insert("index", self.index);
        map.insert("fee", self.fee);
        map.insert("request_height", self.request_height);
        map.insert("quorum_hash", self.quorum_hash.to_vec());
        map.insert("quorum_sig", self.quorum_sig.to_vec());
        map.insert("tx_out_hash", self.tx_out_hash.to_vec());

        map.to_bytes().map_err(|_| {
            Error::Drive(DriveError::CorruptedCodeExecution(
                "Could not seriazlie withdrawal to CBOR",
            ))
        })
    }

    pub fn from_cbor(cbor: &[u8]) -> Result<Self, Error> {
        let map: BTreeMap<String, CborValue> = ciborium::de::from_reader(cbor).map_err(|e| {
            dbg!(e);
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

        let quorum_hash: [u8; 32] = map
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
            .clone()
            .try_into()
            .or_else(|_| {
                Err(Error::Drive(DriveError::CorruptedCodeExecution(
                    "Expected quorum_hash vector to be of length 32",
                )))
            })?;

        let quorum_sig: [u8; 96] = map
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
            .clone()
            .try_into()
            .or_else(|_| {
                Err(Error::Drive(DriveError::CorruptedCodeExecution(
                    "Expected quorum_sig vector to be of length 96",
                )))
            })?;

        let tx_out_hash: [u8; 32] = map
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
            .clone()
            .try_into()
            .or_else(|_| {
                Err(Error::Drive(DriveError::CorruptedCodeExecution(
                    "Expected tx_out_hash vector to be of length 32",
                )))
            })?;

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
    pub fn enqueue_withdrawals(
        &self,
        withdrawals: Vec<Withdrawal>,
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

    mod serialization {
        use std::convert::TryInto;

        use super::*;
        use hex::ToHex;

        #[test]
        fn test_successfull_serialization() {
            let withdrawal = Withdrawal {
                id: 1,
                index: 1,
                fee: 1,
                request_height: 1,
                quorum_hash: vec![0; 32].try_into().unwrap(),
                quorum_sig: vec![0; 96].try_into().unwrap(),
                tx_out_hash: vec![0; 32].try_into().unwrap(),
            };

            let cbor = withdrawal.to_cbor().expect("to serialize the withdrawal");

            let hex: String = cbor.encode_hex();

            assert_eq!(hex, "a762696401636665650165696e646578016a71756f72756d5f73696758600000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000006b71756f72756d5f68617368582000000000000000000000000000000000000000000000000000000000000000006b74785f6f75745f68617368582000000000000000000000000000000000000000000000000000000000000000006e726571756573745f68656967687401");
        }

        #[test]
        fn test_successfull_deserialization() {
            let original_withdrawal = Withdrawal {
                id: 1,
                index: 1,
                fee: 1,
                request_height: 1,
                quorum_hash: vec![0; 32].try_into().unwrap(),
                quorum_sig: vec![0; 96].try_into().unwrap(),
                tx_out_hash: vec![0; 32].try_into().unwrap(),
            };

            let withdrawal = Withdrawal::from_cbor(
                original_withdrawal
                    .to_cbor()
                    .expect("to serialize withdrawal")
                    .as_slice(),
            )
            .expect("to deserialize withdrawal");

            assert_eq!(withdrawal, original_withdrawal,);
        }
    }

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
