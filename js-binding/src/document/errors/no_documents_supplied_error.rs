use super::*;
use thiserror::Error;

#[wasm_bindgen]
#[derive(Error, Debug)]
#[error("No documents were supplied to state transition")]
pub struct NotDocumentsSuppliedError {}
