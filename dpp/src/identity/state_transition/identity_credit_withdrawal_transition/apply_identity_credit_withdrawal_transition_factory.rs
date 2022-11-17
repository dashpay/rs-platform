use std::convert::TryInto;

use anyhow::{anyhow, Result};
use serde_json::{Map, Value as JsonValue};

use crate::{
    data_contract::DataContract,
    document::Document,
    prelude::{Identifier, Identity},
    state_repository::StateRepositoryLike,
    state_transition::StateTransitionConvert,
    util::{entropy_generator::generate, json_value::JsonValueExt},
};

use super::IdentityCreditWithdrawalTransition;

pub const WITHDRAWAL_DATA_CONTRACT_ID_BYTES: [u8; 32] = [
    54, 98, 187, 97, 225, 127, 174, 62, 162, 148, 207, 96, 49, 151, 251, 10, 171, 109, 81, 24, 11,
    216, 182, 16, 76, 73, 68, 166, 47, 226, 217, 127,
];
pub const WITHDRAWAL_DATA_CONTRACT_OWNER_ID_BYTES: [u8; 32] = [
    170, 138, 235, 213, 173, 122, 202, 36, 243, 48, 61, 185, 146, 50, 146, 255, 194, 133, 221, 176,
    188, 82, 144, 69, 234, 198, 106, 35, 245, 167, 46, 192,
];
const PLATFORM_BLOCK_HEADER_TIME_PROPERTY: &str = "time";
const PLATFORM_BLOCK_HEADER_TIME_SECONDS_PROPERTY: &str = "seconds";

pub struct ApplyIdentityCreditWithdrawalTransition<SR>
where
    SR: StateRepositoryLike,
{
    state_repository: SR,
}

impl<SR> ApplyIdentityCreditWithdrawalTransition<SR>
where
    SR: StateRepositoryLike,
{
    pub fn new(state_repository: SR) -> Self {
        Self { state_repository }
    }

    pub async fn apply_identity_credit_withdrawal_transition(
        &self,
        state_transition: &IdentityCreditWithdrawalTransition,
    ) -> Result<()> {
        let data_contract_id = Identifier::new(WITHDRAWAL_DATA_CONTRACT_ID_BYTES);
        let data_contract_owner_id = Identifier::new(WITHDRAWAL_DATA_CONTRACT_OWNER_ID_BYTES);

        let maybe_withdrawals_data_contract: Option<DataContract> = self
            .state_repository
            .fetch_data_contract(&data_contract_id)
            .await?;

        let withdrawals_data_contract = maybe_withdrawals_data_contract
            .ok_or_else(|| anyhow!("Withdrawals data contract not found"))?;

        let latest_platform_block_header: JsonValue = self
            .state_repository
            .fetch_latest_platform_block_header()
            .await?;

        let document_type = String::from("withdrawal");
        let document_entropy = generate();
        let document_created_at_millis = latest_platform_block_header
            .get(PLATFORM_BLOCK_HEADER_TIME_PROPERTY)
            .ok_or_else(|| anyhow!("time property is not set in block header"))?
            .get_i64(PLATFORM_BLOCK_HEADER_TIME_SECONDS_PROPERTY)?
            * 1000;

        let mut document_data_map = Map::new();

        document_data_map.insert(
            "amount".to_string(),
            serde_json::to_value(state_transition.amount)?,
        );
        document_data_map.insert(
            "coreFeePerByte".to_string(),
            serde_json::to_value(state_transition.core_fee_per_byte)?,
        );
        document_data_map.insert("pooling".to_string(), serde_json::to_value(0)?);
        document_data_map.insert(
            "outputScript".to_string(),
            serde_json::to_value(state_transition.output_script.as_bytes())?,
        );
        document_data_map.insert("status".to_string(), serde_json::to_value(0)?);

        let document_data = JsonValue::Object(document_data_map);

        let document_id_bytes: [u8; 32] = state_transition
            .hash(true)?
            .try_into()
            .map_err(|_| anyhow!("Can't convert state transition hash to a document id"))?;

        let withdrawal_document = Document {
            protocol_version: state_transition.protocol_version,
            id: Identifier::new(document_id_bytes),
            document_type,
            revision: 0,
            data_contract_id,
            owner_id: data_contract_owner_id.clone(),
            created_at: Some(document_created_at_millis),
            updated_at: Some(document_created_at_millis),
            data: document_data,
            data_contract: withdrawals_data_contract,
            metadata: None,
            entropy: document_entropy,
        };

        self.state_repository
            .store_document(&withdrawal_document)
            .await?;

        let maybe_existing_identity: Option<Identity> = self
            .state_repository
            .fetch_identity(&state_transition.identity_id)
            .await?;

        let mut existing_identity =
            maybe_existing_identity.ok_or_else(|| anyhow!("Identity not found"))?;

        existing_identity = existing_identity.reduce_balance(state_transition.amount);

        let updated_identity_revision = existing_identity.get_revision() + 1;

        existing_identity = existing_identity.set_revision(updated_identity_revision);

        // TODO: we need to be able to batch state repository operations
        self.state_repository
            .update_identity(&existing_identity)
            .await
    }
}
