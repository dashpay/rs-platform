use crate::error::Error;
use crate::platform::Platform;
use rs_drive::common::value_to_cbor;
use rs_drive::contract::document::Document;
use rs_drive::query::TransactionArg;

pub const MN_REWARD_SHARES_CONTRACT_ID: [u8; 32] = [
    0x0c, 0xac, 0xe2, 0x05, 0x24, 0x66, 0x93, 0xa7, 0xc8, 0x15, 0x65, 0x23, 0x62, 0x0d, 0xaa, 0x93,
    0x7d, 0x2f, 0x22, 0x47, 0x93, 0x44, 0x63, 0xee, 0xb0, 0x1f, 0xf7, 0x21, 0x95, 0x90, 0x95, 0x8c,
];

pub const MN_REWARD_SHARES_DOCUMENT_TYPE: &str = "rewardShare";

impl Platform {
    fn get_reward_shares_list_for_masternode(
        masternode_owner_id: &Vec<u8>,
        transaction: TransactionArg,
    ) -> Result<Vec<Document>, Error> {
        let query_json = json!({
            "where": [
                ["$ownerId", "==", bs58::encode(masternode_owner_id).into_string()]
            ],
        });

        let query_cbor = value_to_cbor(query_json, None);

        let (document_cbors, _, _) = drive.query_documents(
            &query_cbor,
            MN_REWARD_SHARES_CONTRACT_ID,
            MN_REWARD_SHARES_DOCUMENT_TYPE,
            transaction,
        )?;

        document_cbors
            .iter()
            .map(|cbor| Document::from_cbor(cbor, None, None))
            .collect::<Result<Vec<Document>, Error>>()
    }
}
