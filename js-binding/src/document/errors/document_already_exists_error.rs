use super::*;
use crate::mocks::DocumentTransitionWasm;
use thiserror::Error;

#[wasm_bindgen]
#[derive(Error, Debug)]
#[error("Document already exists")]
pub struct DocumentAlreadyExistsError {
    document_transition: DocumentTransitionWasm,
}

#[wasm_bindgen]
impl DocumentAlreadyExistsError {
    #[wasm_bindgen]
    pub fn new(document_transition: DocumentTransitionWasm) -> DocumentAlreadyExistsError {
        Self {
            document_transition,
        }
    }

    #[wasm_bindgen(js_name=getDocumentTransition)]
    pub fn get_document_transition(&self) -> DocumentTransitionWasm {
        self.document_transition.clone()
    }
}
