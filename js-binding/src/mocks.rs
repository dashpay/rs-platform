use dpp::mocks;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
#[derive(Debug, Clone)]
pub struct DocumentTransitionWasm(mocks::DocumentTransition);

impl DocumentTransitionWasm {
    pub fn get_action(&self) -> String {
        unimplemented!()
    }
}

impl From<mocks::DocumentTransition> for DocumentTransitionWasm {
    fn from(v: mocks::DocumentTransition) -> Self {
        DocumentTransitionWasm(v)
    }
}

#[derive(Debug)]
pub struct ConsensusError {}

pub fn from_consensus_to_js_error(_: mocks::ConsensusError) -> JsValue {
    unimplemented!()
}
