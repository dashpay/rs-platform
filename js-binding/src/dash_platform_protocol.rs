use std::sync::Arc;
use js_sys::JsString;
use dpp::identity::IdentityPublicKey;
use dpp::identity::{AssetLockProof, Identity, KeyID};
use dpp::metadata::Metadata;
use wasm_bindgen::prelude::*;
use dpp::errors::consensus::ConsensusError;

use crate::identifier::IdentifierWrapper;
use crate::IdentityPublicKeyWasm;
use crate::MetadataWasm;
use dpp::identity::IdentityFacade;
use dpp::validation::ValidationResult;
use dpp::version::ProtocolVersionValidator;

#[wasm_bindgen(js_name=DashPlatformProtocol)]
pub struct DashPlatformProtocol(IdentityFacade);

#[wasm_bindgen(js_class=DashPlatformProtocol)]
impl DashPlatformProtocol {
    #[wasm_bindgen(constructor)]
    pub fn new() -> DashPlatformProtocol {
        let validator = ProtocolVersionValidator::default();
        let identity_facade = IdentityFacade::new(Arc::new(validator)).unwrap();

        DashPlatformProtocol(identity_facade)
    }
}
