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

pub struct ConsensusError {
    // how to convert all these errors into a logical structure
}
