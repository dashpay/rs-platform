use crate::PublicKeyValidationError;
use bls_signatures::{PublicKey as BlsPublicKey, Serialize};

pub trait BlsValidator {
    fn validate_public_key(&self, pk: &[u8]) -> Result<(), PublicKeyValidationError>;
}

#[derive(Default)]
pub struct NativeBlsValidator;

impl BlsValidator for NativeBlsValidator {
    fn validate_public_key(&self, pk: &[u8]) -> Result<(), PublicKeyValidationError> {
        match BlsPublicKey::from_bytes(pk) {
            Ok(_) => Ok(()),
            Err(e) => Err(PublicKeyValidationError::new(e.to_string())),
        }
    }
}