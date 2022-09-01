use serde_json::Value;
use std::sync::Arc;

use crate::{
    block_time_window::validate_time_in_block_time_window::validate_time_in_block_time_window,
    consensus::basic::BasicError,
    document::validation::state::validate_documents_batch_transition_state::BlockHeader,
    identity::validation::{RequiredPurposeAndSecurityLevelValidator, TPublicKeysValidator},
    prelude::Identity,
    state_repository::StateRepositoryLike,
    validation::SimpleValidationResult,
    NonConsensusError, SerdeParsingError, StateError,
};

use super::identity_update_transition::{property_names, IdentityUpdateTransition};

pub struct ValidateIdentityUpdateTransitionState<T, ST> {
    state_repository: Arc<ST>,
    public_keys_validator: Arc<T>,
}

impl<T, SR> ValidateIdentityUpdateTransitionState<T, SR>
where
    T: TPublicKeysValidator,
    SR: StateRepositoryLike,
{
    pub fn new(state_repository: Arc<SR>, public_keys_validator: Arc<T>) -> Self {
        ValidateIdentityUpdateTransitionState {
            state_repository,
            public_keys_validator,
        }
    }

    pub async fn validate(
        &self,
        state_transition: &IdentityUpdateTransition,
    ) -> Result<SimpleValidationResult, NonConsensusError> {
        let mut validation_result = SimpleValidationResult::default();

        let maybe_stored_identity: Option<Identity> = self
            .state_repository
            .fetch_identity(state_transition.get_identity_id())
            .await
            .map_err(|e| NonConsensusError::StateRepositoryFetchError(e.to_string()))?;

        let stored_identity = match maybe_stored_identity {
            None => {
                validation_result.add_error(BasicError::IdentityNotFoundError {
                    identity_id: state_transition.get_identity_id().to_owned(),
                });
                return Ok(validation_result);
            }
            Some(identity) => identity,
        };

        // copy identity
        let mut identity = stored_identity.clone();

        // Check revision
        if identity.get_revision() != (state_transition.get_revision() - 1) {
            validation_result.add_error(StateError::InvalidIdentityRevisionError {
                identity_id: state_transition.get_identity_id().to_owned(),
                current_revision: identity.get_revision() as u32,
            });
            return Ok(validation_result);
        }

        for key_id in state_transition.get_public_key_ids_to_disable().iter() {
            match identity.get_public_key_by_id(*key_id) {
                None => {
                    validation_result
                        .add_error(StateError::InvalidIdentityPublicKeyIdError { id: *key_id });
                }
                Some(public_key) => {
                    if public_key.read_only {
                        validation_result.add_error(StateError::IdentityPublicKeyIsReadOnlyError {
                            public_key_index: *key_id,
                        })
                    }
                }
            }
        }
        if !validation_result.is_valid() {
            return Ok(validation_result);
        }

        if !state_transition.get_public_key_ids_to_disable().is_empty() {
            // Keys can only be disabled if another valid key is enabled in the same security level
            for key_id in state_transition.get_public_key_ids_to_disable().iter() {
                // the `unwrap()` can be used as the presence if of `key_id` is guaranteed by previous
                // validation
                identity
                    .get_public_key_by_id_mut(*key_id)
                    .unwrap()
                    .disabled_at = state_transition.get_public_keys_disabled_at();
            }

            let block_header: BlockHeader = self
                .state_repository
                .fetch_latest_platform_block_header()
                .await
                .map_err(|e| NonConsensusError::StateRepositoryFetchError(e.to_string()))?;
            let last_block_header_time = (block_header.time.seconds * 1000) as u64;
            let disabled_at_time = state_transition.get_public_keys_disabled_at().ok_or(
                NonConsensusError::RequiredPropertyError {
                    property_name: property_names::PUBLIC_KEYS_DISABLED_AT.to_owned(),
                },
            )?;
            let window_validation_result =
                validate_time_in_block_time_window(last_block_header_time, disabled_at_time);

            if !window_validation_result.is_valid() {
                validation_result.add_error(
                    StateError::IdentityPublicKeyDisabledAtWindowViolationError {
                        disabled_at: disabled_at_time,
                        time_window_start: window_validation_result.time_window_start,
                        time_window_end: window_validation_result.time_window_end,
                    },
                );
                return Ok(validation_result);
            }
        }

        let raw_public_keys: Vec<Value> = identity
            .public_keys
            .iter()
            .map(|pk| pk.to_raw_json_object())
            .collect::<Result<_, SerdeParsingError>>()?;

        if !state_transition.get_public_keys_to_add().is_empty() {
            identity.add_public_keys(state_transition.get_public_keys_to_add().iter().cloned());

            let result = self.public_keys_validator.validate_keys(&raw_public_keys)?;
            if !result.is_valid() {
                return Ok(result);
            }
        }

        let validator = RequiredPurposeAndSecurityLevelValidator {};
        let result = validator.validate_keys(&raw_public_keys)?;

        Ok(result)
    }
}

#[cfg(test)]
mod test {
    use std::sync::Arc;

    use chrono::Utc;

    use crate::{
        block_time_window::validate_time_in_block_time_window::BLOCK_TIME_WINDOW_MILLIS,
        consensus::{basic::TestConsensusError, ConsensusError},
        document::validation::state::validate_documents_batch_transition_state::{
            BlockHeader, HeaderTime,
        },
        identity::{
            state_transition::identity_update_transition::identity_update_transition::IdentityUpdateTransition,
            validation::MockTPublicKeysValidator, Purpose, SecurityLevel,
        },
        prelude::Identity,
        state_repository::MockStateRepositoryLike,
        state_transition::StateTransitionLike,
        tests::{
            fixtures::{get_identity_update_transition_fixture, identity_fixture},
            utils::get_state_error_from_result,
        },
        validation::{SimpleValidationResult, ValidationResult},
        StateError,
    };

    use super::ValidateIdentityUpdateTransitionState;

    struct TestData {
        identity: Identity,
        validate_public_keys_mock: MockTPublicKeysValidator,
        state_repository_mock: MockStateRepositoryLike,
        state_transition: IdentityUpdateTransition,
        block_header: BlockHeader,
    }

    fn setup_test() -> TestData {
        let identity = identity_fixture();
        let block_header = BlockHeader {
            time: HeaderTime {
                seconds: Utc::now().timestamp() as usize,
            },
        };
        let mut validate_public_keys_mock = MockTPublicKeysValidator::new();
        validate_public_keys_mock
            .expect_validate_keys()
            .returning(|_| Ok(Default::default()));

        let mut state_repository_mock = MockStateRepositoryLike::new();
        let identity_to_return = identity.clone();
        let block_header_to_return = block_header.clone();
        state_repository_mock
            .expect_fetch_identity()
            .returning(move |_| Ok(Some(identity_to_return.clone())));
        state_repository_mock
            .expect_fetch_latest_platform_block_header::<BlockHeader>()
            .returning(move || Ok(block_header_to_return.clone()));

        let mut state_transition = get_identity_update_transition_fixture();
        state_transition.set_revision(identity.get_revision() + 1);
        state_transition.set_public_key_ids_to_disable(vec![]);
        state_transition.set_public_keys_disabled_at(None);

        let private_key =
            hex::decode("9b67f852093bc61cea0eeca38599dbfba0de28574d2ed9b99d10d33dc1bde7b2")
                .unwrap();
        state_transition
            .sign_by_private_key(&private_key, crate::identity::KeyType::ECDSA_SECP256K1)
            .expect("transition should be signed");

        TestData {
            identity,
            state_repository_mock,
            validate_public_keys_mock,
            state_transition,
            block_header,
        }
    }

    #[tokio::test]
    async fn should_return_invalid_identity_revision_error_if_new_revision_is_not_incremented_by_1()
    {
        let TestData {
            identity,
            state_repository_mock,
            validate_public_keys_mock,
            mut state_transition,
            ..
        } = setup_test();
        state_transition.set_revision(identity.get_revision() + 2);

        let validator = ValidateIdentityUpdateTransitionState::new(
            Arc::new(state_repository_mock),
            Arc::new(validate_public_keys_mock),
        );
        let result = validator
            .validate(&state_transition)
            .await
            .expect("the validation result should be returned");
        let state_error = get_state_error_from_result(&result, 0);

        assert!(matches!(
            state_error,
            StateError::InvalidIdentityRevisionError {
                identity_id,
                current_revision
            } if  {
                identity_id ==  state_transition.get_identity_id()  &&
                current_revision == &0
            }
        ));
    }

    #[tokio::test]
    async fn should_return_identity_public_key_is_read_only_error_if_disabling_public_key_is_read_only(
    ) {
        let TestData {
            mut identity,
            validate_public_keys_mock,
            mut state_transition,
            ..
        } = setup_test();
        identity.public_keys.get_mut(0).unwrap().read_only = true;
        state_transition.set_public_key_ids_to_disable(vec![0]);

        let identity_to_return = identity.clone();
        let mut state_repository_mock = MockStateRepositoryLike::new();
        state_repository_mock
            .expect_fetch_identity()
            .returning(move |_| Ok(Some(identity_to_return.clone())));

        let validator = ValidateIdentityUpdateTransitionState::new(
            Arc::new(state_repository_mock),
            Arc::new(validate_public_keys_mock),
        );
        let result = validator
            .validate(&state_transition)
            .await
            .expect("the validation result should be returned");
        let state_error = get_state_error_from_result(&result, 0);

        assert!(matches!(
            state_error,
            StateError::IdentityPublicKeyIsReadOnlyError {
                public_key_index
            } if   public_key_index == &0
        ));
    }

    #[tokio::test]
    async fn should_return_invalid_result_if_disabled_at_has_violated_time_window() {
        let TestData {
            validate_public_keys_mock,
            state_repository_mock,
            mut state_transition,
            ..
        } = setup_test();
        state_transition.set_public_key_ids_to_disable(vec![1]);
        state_transition.set_public_keys_disabled_at(Some(
            Utc::now().timestamp_millis() as u64 - (BLOCK_TIME_WINDOW_MILLIS * 2),
        ));

        let validator = ValidateIdentityUpdateTransitionState::new(
            Arc::new(state_repository_mock),
            Arc::new(validate_public_keys_mock),
        );
        let result = validator
            .validate(&state_transition)
            .await
            .expect("the validation result should be returned");
        let state_error = get_state_error_from_result(&result, 0);

        assert!(matches!(
            state_error,
            StateError::IdentityPublicKeyDisabledAtWindowViolationError { .. }
        ));
    }

    #[tokio::test]
    async fn should_throw_invalid_identity_public_key_id_error_if_identity_does_not_contain_public_key_with_disabling_id(
    ) {
        let TestData {
            validate_public_keys_mock,
            state_repository_mock,
            mut state_transition,
            ..
        } = setup_test();
        state_transition.set_public_key_ids_to_disable(vec![3]);
        state_transition.set_public_keys_disabled_at(Some(Utc::now().timestamp_millis() as u64));

        let validator = ValidateIdentityUpdateTransitionState::new(
            Arc::new(state_repository_mock),
            Arc::new(validate_public_keys_mock),
        );
        let result = validator
            .validate(&state_transition)
            .await
            .expect("the validation result should be returned");
        let state_error = get_state_error_from_result(&result, 0);

        assert!(matches!(
            state_error,
            StateError::InvalidIdentityPublicKeyIdError { id } if  {
                id == &3
            }
        ));
    }

    #[tokio::test]
    async fn should_pass_when_disabling_public_key() {
        let TestData {
            validate_public_keys_mock,
            state_repository_mock,
            mut state_transition,
            ..
        } = setup_test();
        state_transition.set_public_key_ids_to_disable(vec![1]);
        state_transition.set_public_keys_disabled_at(Some(Utc::now().timestamp_millis() as u64));

        let validator = ValidateIdentityUpdateTransitionState::new(
            Arc::new(state_repository_mock),
            Arc::new(validate_public_keys_mock),
        );
        let result = validator
            .validate(&state_transition)
            .await
            .expect("the validation result should be returned");
        assert!(result.is_valid());
    }

    #[tokio::test]
    async fn should_pass_when_adding_public_key() {
        let TestData {
            validate_public_keys_mock,
            state_repository_mock,
            mut state_transition,
            ..
        } = setup_test();
        state_transition.set_public_key_ids_to_disable(vec![]);
        state_transition.set_public_keys_disabled_at(None);

        let validator = ValidateIdentityUpdateTransitionState::new(
            Arc::new(state_repository_mock),
            Arc::new(validate_public_keys_mock),
        );
        let result = validator
            .validate(&state_transition)
            .await
            .expect("the validation result should be returned");
        assert!(result.is_valid());
    }

    #[tokio::test]
    async fn should_pass_when_both_adding_and_disabling_public_keys() {
        let TestData {
            validate_public_keys_mock,
            state_repository_mock,
            mut state_transition,
            ..
        } = setup_test();
        state_transition.set_public_key_ids_to_disable(vec![1]);
        state_transition.set_public_keys_disabled_at(Some(Utc::now().timestamp_millis() as u64));

        let validator = ValidateIdentityUpdateTransitionState::new(
            Arc::new(state_repository_mock),
            Arc::new(validate_public_keys_mock),
        );
        let result = validator
            .validate(&state_transition)
            .await
            .expect("the validation result should be returned");
        assert!(result.is_valid());
    }

    #[tokio::test]
    async fn should_validate_purpose_and_security_level() {
        let TestData {
            validate_public_keys_mock,
            mut state_transition,
            mut identity,
            block_header,
            ..
        } = setup_test();

        // the identity after transition must contain at least one
        // key with: purpose: AUTHENTICATION AND security level: MASTER
        identity.get_public_keys_mut().iter_mut().for_each(|k| {
            k.purpose = Purpose::ENCRYPTION;
            k.security_level = SecurityLevel::CRITICAL;
        });
        state_transition
            .get_public_keys_to_add_mut()
            .iter_mut()
            .for_each(|k| {
                k.purpose = Purpose::ENCRYPTION;
                k.security_level = SecurityLevel::CRITICAL;
            });

        let identity_to_return = identity.clone();
        let mut state_repository_mock = MockStateRepositoryLike::new();
        state_repository_mock
            .expect_fetch_identity()
            .returning(move |_| Ok(Some(identity_to_return.clone())));
        state_repository_mock
            .expect_fetch_latest_platform_block_header::<BlockHeader>()
            .returning(move || Ok(block_header.clone()));

        let validator = ValidateIdentityUpdateTransitionState::new(
            Arc::new(state_repository_mock),
            Arc::new(validate_public_keys_mock),
        );
        let result = validator
            .validate(&state_transition)
            .await
            .expect("the validation result should be returned");

        assert!(matches!(
            result.errors[0],
            ConsensusError::MissingMasterPublicKeyError(_)
        ));
    }

    #[tokio::test]
    async fn should_validate_pubic_keys_to_add() {
        let TestData {
            state_repository_mock,
            state_transition,
            ..
        } = setup_test();
        let mut validate_public_keys_mock = MockTPublicKeysValidator::new();
        let some_consensus_error =
            ConsensusError::TestConsensusError(TestConsensusError::new("test"));
        let validation_result = SimpleValidationResult::new(Some(vec![some_consensus_error]));
        validate_public_keys_mock
            .expect_validate_keys()
            .return_once(|_| Ok(validation_result));

        let validator = ValidateIdentityUpdateTransitionState::new(
            Arc::new(state_repository_mock),
            Arc::new(validate_public_keys_mock),
        );
        let result = validator
            .validate(&state_transition)
            .await
            .expect("the validation result should be returned");

        assert!(matches!(
            result.errors[0],
            ConsensusError::TestConsensusError(_)
        ));
    }
}
