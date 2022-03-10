pub use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

use dpp::identifier::Identifier;
use dpp::identity::IdentityPublicKey;
use dpp::identity::{AssetLockProof, Identity, KeyID};
use dpp::metadata::Metadata;

use crate::identifier::IdentifierWrapper;

#[wasm_bindgen(js_name=IdentityPublicKey)]
pub struct IdentityPublicKeyWasm(IdentityPublicKey);

// TODO

#[wasm_bindgen(js_class = IdentityPublicKey)]
impl IdentityPublicKeyWasm {}

impl std::convert::From<IdentityPublicKey> for IdentityPublicKeyWasm {
    fn from(v: IdentityPublicKey) -> Self {
        IdentityPublicKeyWasm(v)
    }
}
