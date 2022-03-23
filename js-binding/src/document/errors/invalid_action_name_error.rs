use super::*;
use crate::mocks::DocumentTransitionWasm;
use thiserror::Error;

#[derive(Error, Debug)]
#[error("Invalid Document action submitted")]
pub struct InvalidActionNameError {
    actions: Vec<String>,
}

#[wasm_bindgen]
#[derive(Error, Debug)]
#[error("Invalid Document action '{}'", document_transition.get_action())]
pub struct InvalidDocumentActionError {
    document_transition: DocumentTransitionWasm,
}
