use std::sync::Arc;
use dpp::errors::consensus::ConsensusError;
use dpp::identity::IdentityPublicKey;
use dpp::identity::{AssetLockProof, Identity, KeyID};
use dpp::metadata::Metadata;
use js_sys::JsString;
use wasm_bindgen::prelude::*;

use crate::identifier::IdentifierWrapper;
use crate::IdentityPublicKeyWasm;
use crate::MetadataWasm;
use dpp::identity::IdentityFacade;
use dpp::NonConsensusError;
use dpp::validation::ValidationResult;
use dpp::version::ProtocolVersionValidator;

#[wasm_bindgen(js_name=ValidationResult)]
pub struct ValidationResultWasm(ValidationResult);

#[wasm_bindgen(js_class=ValidationResult)]
impl ValidationResultWasm {
    /// This is just a test method - doesn't need to be in the resulted binding. Please
    /// remove before shipping
    #[wasm_bindgen(js_name=errorsText)]
    pub fn errors_text(&self) -> Vec<JsString> {
        self.0
            .errors()
            .iter()
            .map(|e| e.to_string().into())
            .collect()
    }

    #[wasm_bindgen(js_name=isValid)]
    pub fn is_valid(&self) -> bool {
        self.0.is_valid()
    }
}

impl From<ValidationResult> for ValidationResultWasm {
    fn from(validation_result: ValidationResult) -> Self {
        ValidationResultWasm(validation_result)
    }
}

#[wasm_bindgen(js_name=IdentityFacade)]
pub struct IdentityFacadeWasm(IdentityFacade);

#[wasm_bindgen(js_class=IdentityFacade)]
impl IdentityFacadeWasm {
    #[wasm_bindgen(constructor)]
    pub fn new() -> IdentityFacadeWasm {
        // TODO: REMOVE THAT LINE, TAKE IT AS AN ARGUMENT
        let protocol_version_validator = ProtocolVersionValidator::default();
        let identity_facade = IdentityFacade::new(Arc::new(protocol_version_validator)).unwrap();

        IdentityFacadeWasm(identity_facade)
    }

    #[wasm_bindgen()]
    pub fn validate(&self, raw_identity_object: JsValue) -> Result<ValidationResultWasm, NonConsensusErrorWasm> {
        // TODO: handle the case when
        self.0
            .validate(
                JsValue::into_serde(&raw_identity_object).expect("unable to serialize identity"),
            )
            .map(|res| res.into())
            .map_err(|err| err.into())
    }
}

#[wasm_bindgen(js_name=Keks)]
pub struct NonConsensusErrorWasm(NonConsensusError);

impl From<NonConsensusError> for NonConsensusErrorWasm {
    fn from(err: NonConsensusError) -> Self {
        Self(err)
    }
}
