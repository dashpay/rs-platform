use std::sync::Arc;

use crate::errors::DashPlatformProtocolInitError;
use crate::identity::validation::{BlsValidator, NativeBlsValidator, PublicKeysValidator};
use crate::identity::IdentityFacade;
use crate::version::{ProtocolVersionValidator, COMPATIBILITY_MAP, LATEST_VERSION};

pub struct DashPlatformProtocol<SR, BLS: BlsValidator> {
    /// Version of protocol
    pub protocol_version: u32,
    /// Public facing facades to interact with the library
    pub identities: IdentityFacade<BLS>,
    /// State Repository provides the access to the stateful validation
    pub state_repository: SR,
}

/// DashPlatformProtocol is the main interface of the library used to perform validation
/// and creating of different data structures
impl<SR, BLS: BlsValidator> DashPlatformProtocol<SR, BLS> {
    pub fn new(
        options: DPPOptions,
        state_repository: SR,
        bls_validator: BLS,
    ) -> Result<Self, DashPlatformProtocolInitError> {
        let current_protocol_version = options.current_protocol_version.unwrap_or(LATEST_VERSION);

        let protocol_version_validator = Arc::new(ProtocolVersionValidator::new(
            current_protocol_version,
            LATEST_VERSION,
            COMPATIBILITY_MAP.clone(),
        ));

        let public_keys_validator = Arc::new(PublicKeysValidator::new(bls_validator)?);

        Ok(Self {
            state_repository,
            protocol_version: current_protocol_version,
            identities: IdentityFacade::new(protocol_version_validator, public_keys_validator)?,
        })
    }

    pub fn identities(&self) -> &IdentityFacade<BLS> {
        &self.identities
    }
}

#[derive(Default)]
pub struct DPPOptions {
    pub current_protocol_version: Option<u32>,
}
