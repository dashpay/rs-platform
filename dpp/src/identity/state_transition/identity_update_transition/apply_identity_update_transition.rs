use crate::{
    consensus::{basic::BasicError, ConsensusError},
    prelude::{Identifier, Identity},
    state_repository::StateRepositoryLike,
    ProtocolError,
};

use super::identity_update_transition::IdentityUpdateTransition;

/// Apply Identity Update state transition
pub async fn apply_identity_update_transition(
    state_repository: &impl StateRepositoryLike,
    state_transition: IdentityUpdateTransition,
) -> Result<(), ProtocolError> {
    let maybe_identity: Option<Identity> = state_repository
        .fetch_identity(state_transition.get_identity_id())
        .await?;
    let mut identity = match maybe_identity {
        None => {
            return Err(identity_not_found_error(
                state_transition.get_identity_id().to_owned(),
            ))
        }
        Some(id) => id,
    };

    identity.revision = state_transition.get_revision();

    if !state_transition.get_public_key_ids_to_disable().is_empty() {
        for id in state_transition.get_public_key_ids_to_disable() {
            if let Some(ref mut public_key) = identity.get_public_key_by_id_mut(*id) {
                public_key.disabled_at = state_transition.get_public_keys_disabled_at();
            }
        }
    }

    if !state_transition.get_public_keys_to_add().is_empty() {
        identity.add_public_keys(state_transition.get_public_keys_to_add().iter().cloned());
        let public_key_hashes: Vec<Vec<u8>> = state_transition
            .get_public_keys_to_add()
            .iter()
            .map(|pk| pk.hash())
            .collect::<Result<_, _>>()?;

        state_repository
            .store_identity_public_key_hashes(identity.get_id(), public_key_hashes)
            .await?;
    }

    state_repository.update_identity(&identity).await?;

    Ok(())
}

fn identity_not_found_error(identity_id: Identifier) -> ProtocolError {
    ProtocolError::AbstractConsensusError(Box::new(ConsensusError::BasicError(Box::new(
        BasicError::IdentityNotFoundError { identity_id },
    ))))
}

#[cfg(test)]
mod test {
    use crate::{
        identity::state_transition::identity_update_transition::identity_update_transition::IdentityUpdateTransition,
        state_repository::MockStateRepositoryLike,
        tests::fixtures::{get_identity_update_transition_fixture, identity_fixture},
    };

    use super::apply_identity_update_transition;

    struct TestData {
        state_transition: IdentityUpdateTransition,
        state_repository_mock: MockStateRepositoryLike,
    }

    fn setup_test() -> TestData {
        let mut state_transition = get_identity_update_transition_fixture();
        state_transition.set_revision(state_transition.get_revision() + 1);

        let identity = identity_fixture();

        let mut state_repository_mock = MockStateRepositoryLike::new();
        let identity_to_return = identity.clone();
        state_repository_mock
            .expect_fetch_identity()
            .returning(move |_| Ok(Some(identity_to_return.clone())));

        TestData {
            state_transition,
            state_repository_mock,
        }
    }

    #[tokio::test]
    async fn should_add_public_key() {
        let TestData {
            mut state_transition,
            mut state_repository_mock,
        } = setup_test();

        state_transition.set_public_keys_disabled_at(None);
        state_transition.set_public_key_ids_to_disable(vec![]);
        state_repository_mock
            .expect_store_identity_public_key_hashes()
            .returning(|_, _| Ok(()));
        state_repository_mock
            .expect_update_identity()
            .returning(|_| Ok(()));

        let result =
            apply_identity_update_transition(&state_repository_mock, state_transition).await;

        assert!(result.is_ok());
    }
}
