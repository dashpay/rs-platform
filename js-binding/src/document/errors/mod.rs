use thiserror::Error;
use wasm_bindgen::prelude::*;

use crate::{mocks::DocumentTransitionWasm, DocumentWasm};

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

#[wasm_bindgen]
#[derive(Error, Debug)]
#[error("Document was not provided for apply of state transition")]
pub struct DocumentNotProvidedError {
    document_transition: DocumentTransitionWasm,
}

#[wasm_bindgen]
impl DocumentNotProvidedError {
    #[wasm_bindgen]
    pub fn new(document_transition: DocumentTransitionWasm) -> DocumentNotProvidedError {
        Self {
            document_transition,
        }
    }

    #[wasm_bindgen(js_name=getDocumentTransition)]
    pub fn get_document_transition(&self) -> DocumentTransitionWasm {
        self.document_transition.clone()
    }
}

#[wasm_bindgen]
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

#[wasm_bindgen]
impl InvalidDocumentActionError {
    #[wasm_bindgen]
    pub fn new(document_transition: DocumentTransitionWasm) -> InvalidDocumentActionError {
        Self {
            document_transition,
        }
    }

    #[wasm_bindgen(js_name=getDocumentTransition)]
    pub fn get_document_transition(&self) -> DocumentTransitionWasm {
        self.document_transition.clone()
    }
}

#[wasm_bindgen]
#[derive(Error, Debug)]
#[error("Invalid document: {:?}", errors)]
pub struct InvalidDocumentError {
    // the point is how we hold all there different types in  the Vector
    errors: Vec<JsValue>,
    document: DocumentWasm,
}

#[wasm_bindgen]
impl InvalidDocumentActionError {}

#[wasm_bindgen]
#[derive(Error, Debug)]
#[error("Invalid Document Initial revision '{}'", document.get_revision())]
pub struct InvalidInitialRevisionError {
    document: DocumentWasm,
}

#[wasm_bindgen]
#[derive(Error, Debug)]
#[error("Documents have mixed owner ids")]
pub struct MismatchOwnersIdsError {
    documents: DocumentWasm,
}

#[wasm_bindgen]
#[derive(Error, Debug)]
#[error("No documents were supplied to state transition")]
pub struct NotDocumentsSuppliedError {}
