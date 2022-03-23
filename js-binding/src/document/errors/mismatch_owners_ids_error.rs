use super::*;
use crate::DocumentWasm;
use thiserror::Error;

#[wasm_bindgen]
#[derive(Error, Debug)]
#[error("Documents have mixed owner ids")]
pub struct MismatchOwnersIdsError {
    documents: Vec<DocumentWasm>,
}

#[wasm_bindgen]
impl MismatchOwnersIdsError {
    #[wasm_bindgen]
    pub fn new(documents: Vec<JsValue>) -> MismatchOwnersIdsError {
        Self {
            documents: from_vec_js(&documents),
        }
    }

    #[wasm_bindgen(js_name=getDocumentTransition)]
    pub fn get_documents(&self) -> Vec<JsValue> {
        to_vec_js(self.documents.clone())
    }
}
