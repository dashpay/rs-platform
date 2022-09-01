use anyhow::anyhow;
use lazy_static::lazy_static;
use serde_json::Value as JsonValue;

use crate::{
    identity::validation::{duplicated_key_ids, duplicated_keys, TPublicKeysValidator},
    prelude::IdentityPublicKey,
    util::json_value::JsonValueExt,
    validation::SimpleValidationResult,
    ProtocolError, StateError,
};

lazy_static! {
    static ref IDENTITY_JSON_SCHEMA: JsonValue =
        serde_json::from_str(include_str!("./../../../schema/identity/identity.json"))
            .expect("Identity Schema file should exist");
}

pub struct IdentityUpdatePublicKeysValidator {}
impl TPublicKeysValidator for IdentityUpdatePublicKeysValidator {
    fn validate_keys(
        &self,
        raw_public_keys: &[JsonValue],
    ) -> Result<SimpleValidationResult, crate::NonConsensusError> {
        validate_public_keys(raw_public_keys)
            .map_err(|e| crate::NonConsensusError::SerdeJsonError(e.to_string()))
    }
}

pub fn validate_public_keys(
    raw_public_keys: &[JsonValue],
) -> Result<SimpleValidationResult, ProtocolError> {
    let mut validation_result = SimpleValidationResult::default();

    let maybe_max_items = IDENTITY_JSON_SCHEMA.get_value("properties.publicKeys.maxItems")?;
    let max_items = maybe_max_items
        .as_u64()
        .ok_or_else(|| anyhow!("the maxItems property isn't a integer"))?
        as usize;

    if raw_public_keys.len() > max_items {
        validation_result.add_error(StateError::MaxIdentityPublicKeyLimitReached { max_items });
        return Ok(validation_result);
    }

    let public_keys: Vec<IdentityPublicKey> = raw_public_keys
        .iter()
        .cloned()
        .map(serde_json::from_value)
        .collect::<Result<_, _>>()?;

    // Check that there's not duplicates key ids in the state transition
    let duplicated_ids = duplicated_key_ids(&public_keys);
    if !duplicated_ids.is_empty() {
        validation_result
            .add_error(StateError::DuplicatedIdentityPublicKeyIdError { duplicated_ids });
    }

    // Check that there's no duplicated keys
    let duplicated_key_ids = duplicated_keys(&public_keys);
    if !duplicated_key_ids.is_empty() {
        validation_result.add_error(StateError::DuplicatedIdentityPublicKeyError {
            duplicated_public_key_ids: duplicated_key_ids,
        });
    }

    Ok(validation_result)
}

#[cfg(test)]
mod test {

    use super::*;
    use crate::{
        prelude::Identity,
        tests::{fixtures::identity_fixture, utils::get_state_error_from_result},
    };

    struct TestData {
        raw_public_keys: Vec<JsonValue>,
        identity: Identity,
    }

    fn setup_test() -> TestData {
        let identity = identity_fixture();
        let raw_public_keys: Vec<JsonValue> = identity
            .public_keys
            .iter()
            .map(|pk| pk.to_raw_json_object())
            .collect::<Result<_, _>>()
            .unwrap();

        TestData {
            identity,
            raw_public_keys,
        }
    }

    #[test]
    fn should_return_invalid_result_if_there_are_duplicate_key_ids() {
        let TestData {
            mut raw_public_keys,
            ..
        } = setup_test();

        raw_public_keys[1]["id"] = raw_public_keys[0]["id"].clone();
        let result = validate_public_keys(&raw_public_keys)
            .expect("the validation result should be returned");

        let state_error = get_state_error_from_result(&result, 0);

        assert!(matches!(
            state_error,
            StateError::DuplicatedIdentityPublicKeyIdError { duplicated_ids }
            if duplicated_ids == &vec![0]
        ));
        assert_eq!(4022, result.errors[0].code());
    }

    #[test]
    fn should_return_invalid_result_if_there_are_duplicate_keys() {
        let TestData {
            mut raw_public_keys,
            ..
        } = setup_test();

        raw_public_keys[1]["data"] = raw_public_keys[0]["data"].clone();
        let result = validate_public_keys(&raw_public_keys)
            .expect("the validation result should be returned");

        let state_error = get_state_error_from_result(&result, 0);

        assert!(matches!(
            state_error,
            StateError::DuplicatedIdentityPublicKeyError { duplicated_public_key_ids }
            if duplicated_public_key_ids == &vec![1]
        ));
        assert_eq!(4021, result.errors[0].code());
    }

    #[test]
    fn should_pass_valid_public_keys() {
        let TestData {
            raw_public_keys, ..
        } = setup_test();

        let result = validate_public_keys(&raw_public_keys)
            .expect("the validation result should be returned");
        assert!(result.is_valid());
    }

    #[test]
    fn should_return_invalid_result_if_number_of_public_keys_is_bigger_than_32() {
        let TestData {
            mut raw_public_keys,
            ..
        } = setup_test();

        let max_items = IDENTITY_JSON_SCHEMA["properties"]["publicKeys"]["maxItems"]
            .as_u64()
            .unwrap() as usize;
        let num_to_add = max_items - raw_public_keys.len() + 1;

        for _ in 0..num_to_add {
            raw_public_keys.push(raw_public_keys[0].clone());
        }

        let result = validate_public_keys(&raw_public_keys)
            .expect("the validation result should be returned");
        let state_error = get_state_error_from_result(&result, 0);

        assert!(matches!(
            state_error,
            StateError::MaxIdentityPublicKeyLimitReached { max_items }
            if max_items == &32
        ));
        assert_eq!(4020, result.errors[0].code());
    }
}
