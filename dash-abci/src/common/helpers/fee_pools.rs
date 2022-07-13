use std::collections::BTreeMap;
use std::ops::Range;
use ciborium::value::Value;

use dpp::{
    identifier::Identifier,
    identity::{Identity, IdentityPublicKey, KeyType},
};
use rs_drive::contract::Contract;
use rs_drive::contract::document::Document;
use rs_drive::grovedb::TransactionArg;
use rs_drive::drive::batch::GroveDbOpBatch;
use rs_drive::drive::Drive;
use rs_drive::drive::flags::StorageFlags;
use rs_drive::drive::object_size_info::DocumentAndContractInfo;
use rs_drive::drive::object_size_info::DocumentInfo::DocumentAndSerialization;
use rs_drive::fee_pools::epochs::Epoch;
use crate::contracts::reward_shares::MN_REWARD_SHARES_DOCUMENT_TYPE;

pub fn get_storage_credits_for_distribution_for_epochs_in_range(
    drive: &Drive,
    epoch_range: Range<u16>,
    transaction: TransactionArg,
) -> Vec<u64> {
    epoch_range
        .map(|index| {
            let epoch = Epoch::new(index);
            drive
                .get_epoch_storage_credits_for_distribution(&epoch, transaction)
                .expect("should get storage fee")
        })
        .collect()
}

fn create_mn_share_document(
    drive: &Drive,
    contract: &Contract,
    identity: &Identity,
    pay_to_identity: &Identity,
    percentage: u16,
    transaction: TransactionArg,
) -> Document {
    let id = rand::random::<[u8; 32]>();

    let mut properties: BTreeMap<String, Value> = BTreeMap::new();

    properties.insert(
        String::from("payToId"),
        Value::Bytes(pay_to_identity.id.buffer.to_vec()),
    );
    properties.insert(String::from("percentage"), percentage.into());

    let document = Document {
        id,
        properties,
        owner_id: identity.id.buffer,
    };

    let document_type = contract
        .document_type_for_name(MN_REWARD_SHARES_DOCUMENT_TYPE)
        .expect("expected to get a document type");

    let storage_flags = StorageFlags { epoch: 0 };

    let document_cbor = document.to_cbor();

    drive
        .add_document_for_contract(
            DocumentAndContractInfo {
                document_info: DocumentAndSerialization((
                    &document,
                    &document_cbor,
                    &storage_flags,
                )),
                contract: &contract,
                document_type,
                owner_id: None,
            },
            false,
            0f64,
            true,
            transaction,
        )
        .expect("expected to insert a document successfully");

    document
}

pub fn create_masternode_share_identities_and_documents(
    drive: &Drive,
    contract: &Contract,
    pro_tx_hashes: &Vec<[u8; 32]>,
    transaction: TransactionArg,
) -> Vec<(Identity, Document)> {
    drive.fetch_identities(pro_tx_hashes, transaction)
        .into_iter()
        .map(|(mn_identity, flags)| {
            let id: [u8; 32] = rand::random();
            let identity = create_test_identity(drive, id, transaction);
            let document = create_mn_share_document(
                drive,
                contract,
                mn_identity,
                &identity,
                5000,
                transaction,
            );

            (identity, document)
        })
        .collect()
}

pub fn refetch_identities(
    drive: &Drive,
    identities: Vec<&Identity>,
    transaction: TransactionArg,
) -> Vec<Identity> {
    identities
        .iter()
        .map(|identity| {
            drive
                .fetch_identity(&identity.id.buffer, transaction)
                .unwrap()
        })
        .collect()
}
