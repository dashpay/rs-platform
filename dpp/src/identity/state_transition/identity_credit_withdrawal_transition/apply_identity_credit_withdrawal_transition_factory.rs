use anyhow::{anyhow, Result};
use dashcore::{
    blockdata::transaction::special_transaction::{
        asset_lock::AssetLockPayload,
        asset_unlock::{
            qualified_asset_unlock::AssetUnlockPayload,
            unqualified_asset_unlock::{AssetUnlockBasePayload, AssetUnlockBaseTransactionInfo},
        },
        TransactionPayload,
    },
    consensus::Encodable,
    Transaction,
};

use crate::{prelude::Identity, state_repository::StateRepositoryLike};

use super::IdentityCreditWithdrawalTransition;

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
        ApplyIdentityCreditWithdrawalTransition { state_repository }
    }

    pub async fn apply_identity_credit_withdrawal_transition(
        &self,
        state_transition: &IdentityCreditWithdrawalTransition,
    ) -> Result<()> {
        let latest_withdrawal_index = self
            .state_repository
            .fetch_latest_withdrawal_transaction_index()
            .await?;

        let withdrwal_transaction = AssetUnlockBaseTransactionInfo {
            version: 1,
            lock_time: 0,
            output: vec![],
            base_payload: AssetUnlockBasePayload {
                version: 1,
                index: latest_withdrawal_index + 1,
                fee: state_transition.core_fee,
            },
        };

        let mut transaction_buffer: Vec<u8> = vec![];

        withdrwal_transaction
            .consensus_encode(&mut transaction_buffer)
            .map_err(|e| anyhow!(e))?;

        self.state_repository
            .enqueue_withdrawal_transaction(transaction_buffer)
            .await?;

        let maybe_existing_identity: Option<Identity> = self
            .state_repository
            .fetch_identity(&state_transition.identity_id)
            .await?;

        let mut existing_identity =
            maybe_existing_identity.ok_or_else(|| anyhow!("Identity not found"))?;

        existing_identity = existing_identity.reduce_balance(state_transition.amount);

        self.state_repository
            .update_identity(&existing_identity)
            .await
    }
}
