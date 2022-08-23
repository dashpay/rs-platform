use anyhow::{anyhow, bail};

use crate::{
    consensus::signature::SignatureError,
    identity::{validation::validate_identity_existence, KeyType},
    prelude::{Identifier, Identity, IdentityPublicKey},
    state_repository::StateRepositoryLike,
    state_transition::{
        StateTransition, StateTransitionIdentitySigned, StateTransitionLike, StateTransitionType,
    },
    validation::ValidationResult,
    ProtocolError,
};

pub async fn validate_state_transition_identity_signature(
    state_repository: &impl StateRepositoryLike,
    // TODO consider changing the the &StateTransition with &impl StateTransitionIdentitySigned
    // TODO so far, not sure how the interface with 'impl' could get along with the rest of components
    state_transition: &StateTransition,
) -> Result<ValidationResult<()>, ProtocolError> {
    let mut validation_result = ValidationResult::<()>::default();

    if !(state_transition.get_type() == StateTransitionType::DataContractCreate
        || state_transition.get_type() == StateTransitionType::DocumentsBatch)
    {
        return Err(anyhow!("validator supports only the DocumentsBatch Transition or DataContract create state transition").into());
    }

    // Owner must exist
    let owner_id = get_owner_id(state_transition)?;

    let result = validate_identity_existence(state_repository, owner_id).await?;
    if !result.is_valid() {
        return Ok(result.into_result_without_data());
    }

    let identity = result
        .data()
        .ok_or_else(|| anyhow!("the result doesn't contain any Identity"))?;
    let signature_public_key_id = get_signature_public_key_id(state_transition)?;
    let maybe_public_key = identity.get_public_key_by_id(signature_public_key_id);

    let public_key = match maybe_public_key {
        None => {
            validation_result.add_error(SignatureError::MissingPublicKeyError {
                public_key_id: signature_public_key_id,
            });
            return Ok(validation_result);
        }
        Some(id) => id,
    };

    if public_key.get_type() != KeyType::ECDSA_SECP256K1
        && public_key.get_type() != KeyType::ECDSA_HASH160
    {
        validation_result.add_error(SignatureError::InvalidIdentityPublicKeyTypeError {
            key_type: public_key.get_type(),
        });
        return Ok(validation_result);
    }

    let signature_is_valid = verify_signature(state_transition, public_key);
    if signature_is_valid.is_err() {
        validation_result.add_error(SignatureError::InvalidStateTransitionSignatureError);
        return Ok(validation_result);
    }

    Ok(validation_result)
}

fn verify_signature(
    state_transition: &StateTransition,
    public_key: &IdentityPublicKey,
) -> Result<(), ProtocolError> {
    match state_transition {
        StateTransition::DataContractCreate(st) => st.verify_signature(public_key),
        StateTransition::DocumentsBatch(st) => st.verify_signature(public_key),
        _ => Err(anyhow!(
            "Unable to verify signature: state transition {} is  not supported",
            state_transition.get_type()
        )
        .into()),
    }
}

fn get_signature_public_key_id(state_transition: &StateTransition) -> Result<u64, anyhow::Error> {
    match state_transition {
        StateTransition::DataContractCreate(st) => Ok(st.get_signature_public_key_id()),
        StateTransition::DocumentsBatch(st) => Ok(st.get_signature_public_key_id()),
        _ => Err(anyhow!(
            "Unable to get public key id: state transition {} is  not supported",
            state_transition.get_type()
        )
        .into()),
    }
}

fn get_owner_id<'a>(
    state_transition: &'a StateTransition,
) -> Result<&'a Identifier, anyhow::Error> {
    match state_transition {
        StateTransition::DataContractCreate(st) => Ok(st.get_owner_id()),
        StateTransition::DocumentsBatch(st) => Ok(st.get_owner_id()),
        _ => Err(anyhow!(
            "Unable to get owner id: state transition {} is  not supported",
            state_transition.get_type()
        )
        .into()),
    }
}

#[cfg(test)]
mod test {
    use crate::tests::utils::generate_random_identifier_struct;

    #[test]
    fn should_pass_properly_signed_state_transition() {
        let owner_id = generate_random_identifier_struct();
        let public_key_id = 1;
    }
}
