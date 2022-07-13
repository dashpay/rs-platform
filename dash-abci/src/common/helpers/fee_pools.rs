use std::collections::BTreeMap;
use std::ops::Range;

use dpp::{
    identifier::Identifier,
    identity::{Identity, IdentityPublicKey, KeyType},
};
use rs_drive::grovedb::TransactionArg;
use rs_drive::drive::batch::GroveDbOpBatch;
use rs_drive::drive::Drive;
use rs_drive::fee_pools::epochs::Epoch;

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

fn create_test_identity(drive: &Drive, id: [u8; 32], transaction: TransactionArg) -> Identity {
    let identity_key = IdentityPublicKey {
        id: 1,
        key_type: KeyType::ECDSA_SECP256K1,
        data: vec![0, 1, 2, 3],
        purpose: dpp::identity::Purpose::AUTHENTICATION,
        security_level: dpp::identity::SecurityLevel::MASTER,
        read_only: false,
    };

    let identity = Identity {
        id: Identifier::new(id),
        revision: 1,
        balance: 0,
        protocol_version: 0,
        public_keys: vec![identity_key],
        asset_lock_proof: None,
        metadata: None,
    };

    drive
        .insert_identity(identity.clone(), true, StorageFlags::default(), transaction)
        .expect("should insert identity");

    identity
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

pub fn create_masternode_identities_and_increment_proposers(
    drive: &Drive,
    epoch_pool: &Epoch,
    count: u8,
    transaction: TransactionArg,
) -> Vec<[u8; 32]> {
    let proposers = create_masternode_identities(drive, count, transaction);

    increment_proposers_block_count(drive, &proposers, epoch_pool, transaction);

    proposers
}

pub fn create_masternode_identities(
    drive: &Drive,
    count: u8,
    transaction: TransactionArg,
) -> Vec<[u8; 32]> {
    let mut proposers: Vec<[u8; 32]> = Vec::with_capacity(count as usize);

    for _ in 0..count {
        let proposer_pro_tx_hash: [u8; 32] = rand::random();

        create_test_identity(drive, proposer_pro_tx_hash, transaction);

        proposers.push(proposer_pro_tx_hash);
    }

    proposers
}

pub fn increment_proposers_block_count(
    drive: &Drive,
    proposers: &Vec<[u8; 32]>,
    epoch_pool: &Epoch,
    transaction: TransactionArg,
) {
    let mut batch = GroveDbOpBatch::new();

    for proposer_pro_tx_hash in proposers {
        let op = epoch_pool
            .increment_proposer_block_count_operation(&drive, &proposer_pro_tx_hash, transaction)
            .expect("should increment proposer block count");
        batch.push(op);
    }

    drive
        .grove_apply_batch(batch, true, transaction)
        .expect("should apply batch");
}

pub fn create_masternode_share_identities_and_documents(
    drive: &Drive,
    contract: &Contract,
    pro_tx_hashes: &Vec<[u8; 32]>,
    transaction: TransactionArg,
) -> Vec<(Identity, Document)> {
    fetch_identities_by_pro_tx_hashes(drive, pro_tx_hashes, transaction)
        .iter()
        .map(|mn_identity| {
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

pub fn fetch_identities_by_pro_tx_hashes(
    drive: &Drive,
    pro_tx_hashes: &Vec<[u8; 32]>,
    transaction: TransactionArg,
) -> Vec<Identity> {
    pro_tx_hashes
        .iter()
        .map(|pro_tx_hash| drive.fetch_identity(pro_tx_hash, transaction).unwrap())
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
