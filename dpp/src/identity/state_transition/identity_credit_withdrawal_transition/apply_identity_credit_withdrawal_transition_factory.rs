use anyhow::{anyhow, Result};
use chrono::Utc;
use dashcore::{
    blockdata::transaction::special_transaction::asset_unlock::unqualified_asset_unlock::{
        AssetUnlockBasePayload, AssetUnlockBaseTransactionInfo,
    },
    consensus::Encodable,
    Script, TxOut,
};
use lazy_static::__Deref;
use serde_json::{Map, Value as JsonValue};

use crate::{
    data_contract::DataContract,
    document::{generate_document_id::generate_document_id, Document},
    identity::convert_credits_to_satoshi,
    prelude::{Identifier, Identity},
    state_repository::StateRepositoryLike,
    util::entropy_generator::generate,
    version::LATEST_VERSION,
};

use super::IdentityCreditWithdrawalTransition;

const WITHDRAWAL_DATA_CONTRACT_ID_BYTES: [u8; 32] = [
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
];
const WITHDRAWAL_DATA_CONTRACT_OWNER_ID_BYTES: [u8; 32] = [
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
];

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
        let latest_withdrawal_index = self
            .state_repository
            .fetch_latest_withdrawal_transaction_index()
            .await?;

        let output_script: Script = state_transition.output_script.deref().clone();

        let tx_out = TxOut {
            value: convert_credits_to_satoshi(state_transition.amount),
            script_pubkey: output_script,
        };

        let withdrawal_transaction = AssetUnlockBaseTransactionInfo {
            version: 1,
            lock_time: 0,
            output: vec![tx_out],
            base_payload: AssetUnlockBasePayload {
                version: 1,
                index: latest_withdrawal_index + 1,
                fee: state_transition.core_fee_per_byte, // TODO: redo fee calculation
            },
        };

        let mut transaction_buffer: Vec<u8> = vec![];

        withdrawal_transaction
            .consensus_encode(&mut transaction_buffer)
            .map_err(|e| anyhow!(e))?;

        self.state_repository
            .enqueue_withdrawal_transaction(latest_withdrawal_index, transaction_buffer)
            .await?;

        let data_contract_id = Identifier::new(WITHDRAWAL_DATA_CONTRACT_ID_BYTES);
        let data_contract_owner_id = Identifier::new(WITHDRAWAL_DATA_CONTRACT_OWNER_ID_BYTES);

        let maybe_withdrawals_data_contract: Option<DataContract> = self
            .state_repository
            .fetch_data_contract(&data_contract_id)
            .await?;

        let withdrawals_data_contract = maybe_withdrawals_data_contract
            .ok_or_else(|| anyhow!("Withdrawals data contract not found"))?;

        let document_type = String::from("withdrawal");
        let document_entropy = generate();
        let document_created_at = Utc::now();

        let mut document_data_map = Map::new();

        // TODO: figure out about transactionId
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

        let withdrawal_document = Document {
            protocol_version: LATEST_VERSION,
            id: generate_document_id(
                &data_contract_id,
                &data_contract_owner_id,
                &document_type,
                &document_entropy,
            ),
            document_type,
            revision: 0,
            data_contract_id,
            owner_id: data_contract_owner_id.clone(),
            created_at: Some(document_created_at.timestamp_millis()),
            updated_at: Some(document_created_at.timestamp_millis()),
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

        // TODO: we need to be able to batch state repository operations
        self.state_repository
            .update_identity(&existing_identity)
            .await
    }
}
