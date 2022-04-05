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

#[wasm_bindgen(js_name=ValidationResult)]
pub struct ValidationResultWasm(ValidationResult);

#[wasm_bindgen(js_class=ValidationResult)]
impl ValidationResultWasm {
    /// This is just a test method - doesn't need to be in the resulted binding. Please
    /// remove before shipping
    #[wasm_bindgen(js_name=errorsText)]
    pub fn errors_text(&self) -> Vec<JsString> {
        self.0.errors().iter().map(|e| match e {
            ConsensusError::JsonSchemaError(err) => {
                err.to_string().into()
            }
        }).collect()
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
        let identity_facade = IdentityFacade::new().unwrap();

        IdentityFacadeWasm(identity_facade)
    }

    #[wasm_bindgen()]
    pub fn validate(&self, identity_json: JsValue) -> ValidationResultWasm {
        // TODO: handle the case when
        self.0.validate(JsValue::into_serde(&identity_json).expect("unable to serialize identity")).into()
    }
}
