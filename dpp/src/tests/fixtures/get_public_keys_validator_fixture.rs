use crate::identity::validation::{PublicKeysValidator, PUBLIC_KEY_SCHEMA, PUBLIC_KEY_SCHEMA_FOR_TRANSITION, NativeBlsValidator};

pub fn get_public_keys_validator_for_transition() -> PublicKeysValidator<NativeBlsValidator> {
    PublicKeysValidator::new_with_schema(PUBLIC_KEY_SCHEMA_FOR_TRANSITION.clone(), NativeBlsValidator::default()).unwrap()
}

pub fn get_public_keys_validator() -> PublicKeysValidator<NativeBlsValidator> {
    PublicKeysValidator::new_with_schema(PUBLIC_KEY_SCHEMA.clone(), NativeBlsValidator::default()).unwrap()
}
