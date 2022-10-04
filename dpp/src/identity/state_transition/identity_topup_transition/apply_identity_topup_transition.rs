use std::sync::Arc;

use anyhow::{anyhow, Result};

use crate::identity::state_transition::asset_lock_proof::AssetLockTransactionOutputFetcher;
use crate::identity::state_transition::identity_topup_transition::IdentityTopUpTransition;
use crate::identity::{convert_satoshi_to_credits, get_biggest_possible_identity, Identity};
use crate::state_repository::StateRepositoryLike;
use crate::state_transition::StateTransitionLike;

pub struct ApplyIdentityTopUpTransition<SR>
where
    SR: StateRepositoryLike,
{
    state_repository: Arc<SR>,
    asset_lock_transaction_output_fetcher: Arc<AssetLockTransactionOutputFetcher<SR>>,
}

impl<SR> ApplyIdentityTopUpTransition<SR>
where
    SR: StateRepositoryLike,
{
    pub async fn apply_identity_topup_transition(
        &self,
        state_transition: &IdentityTopUpTransition,
    ) -> Result<()> {
        let output = self
            .asset_lock_transaction_output_fetcher
            .fetch(
                state_transition.get_asset_lock_proof(),
                state_transition.get_execution_context(),
            )
            .await?;

        let credits_amount = convert_satoshi_to_credits(output.value);

        let out_point = state_transition
            .get_asset_lock_proof()
            .out_point()
            .ok_or_else(|| anyhow!("Out point is missing from asset lock proof"))?;
        let identity_id = state_transition.get_identity_id();

        let mut maybe_identity = self
            .state_repository
            .fetch_identity::<Identity>(identity_id, state_transition.get_execution_context())
            .await?;

        if state_transition.get_execution_context().is_dry_run() {
            maybe_identity = Some(get_biggest_possible_identity())
        }

        if let Some(identity) = maybe_identity {
            let identity = identity.increase_balance(credits_amount);

            self.state_repository
                .update_identity(&identity, state_transition.get_execution_context())
                .await?;

            self.state_repository
                .mark_asset_lock_transaction_out_point_as_used(&out_point)
                .await?;

            Ok(())
        } else {
            Err(anyhow!("Identity not found"))
        }
    }
}
