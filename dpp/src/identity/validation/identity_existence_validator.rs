use crate::{
    consensus::signature::SignatureError,
    prelude::{Identifier, Identity},
    state_repository::StateRepositoryLike,
    validation::ValidationResult,
    ProtocolError,
};

pub async fn validate_identity_existence(
    state_repository: &impl StateRepositoryLike,
    identity_id: &Identifier,
) -> Result<ValidationResult<Identity>, ProtocolError> {
    let mut result = ValidationResult::<Identity>::default();

    let maybe_identity: Option<Identity> = state_repository.fetch_identity(identity_id).await?;
    match maybe_identity {
        None => result.add_error(SignatureError::IdentityNotFoundError {
            identity_id: identity_id.to_owned(),
        }),

        Some(identity) => {
            result.set_data(identity);
        }
    }

    return Ok(result);
}

#[cfg(test)]
mod test {
    #[tokio::test]
    async fn should_return_invalid_result_if_identity_is_not_found() {
        todo!();
    }

    #[tokio::test]
    async fn should_return_valid_result() {
        todo!()
    }
}
