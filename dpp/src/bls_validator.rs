use anyhow::anyhow;
use crate::{ProtocolError, PublicKeyValidationError};
use bls_signatures::{PublicKey as BlsPublicKey, Serialize};
use bls_signatures::{
    verify_messages, PrivateKey as BLSPrivateKey, PublicKey as BLSPublicKey,
    Serialize as BLSSerialize,
};

pub trait BlsModule {
    fn validate_public_key(&self, pk: &[u8]) -> Result<(), PublicKeyValidationError>;
    fn verify_signature(&self, signature: &[u8], data: &[u8], public_key: &[u8]) -> Result<bool, ProtocolError>;
}

#[derive(Default)]
pub struct NativeBlsValidator;

impl BlsModule for NativeBlsValidator {
    fn validate_public_key(&self, pk: &[u8]) -> Result<(), PublicKeyValidationError> {
        match BlsPublicKey::from_bytes(pk) {
            Ok(_) => Ok(()),
            Err(e) => Err(PublicKeyValidationError::new(e.to_string())),
        }
    }

    fn verify_signature(&self, signature: &[u8], data: &[u8], public_key: &[u8]) -> Result<bool, ProtocolError> {
        let pk = BLSPublicKey::from_bytes(public_key).map_err(anyhow::Error::msg)?;
        let signature = bls_signatures::Signature::from_bytes(signature)
            .map_err(anyhow::Error::msg)?;
        match verify_messages(&signature, &[&data], &[pk]) {
            true => Ok(true),
            // TODO change to specific error type
            false => Err(anyhow!("Verification failed").into()),
        }
    }
}