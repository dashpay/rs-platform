use crate::utils::*;

use wasm_bindgen::prelude::*;

mod document_already_exists_error;
pub use document_already_exists_error::*;

mod document_not_provided_error;
pub use document_not_provided_error::*;

mod invalid_action_name_error;
pub use invalid_action_name_error::*;

mod invalid_document_error;
pub use invalid_document_error::*;

mod invalid_document_action_error;
pub use invalid_document_action_error::*;

mod invalid_initial_revision_error;
pub use invalid_initial_revision_error::*;

mod mismatch_owners_ids_error;
pub use mismatch_owners_ids_error::*;

mod no_documents_supplied_error;
pub use no_documents_supplied_error::*;
