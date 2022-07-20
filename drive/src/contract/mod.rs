// TODO remove the file
pub mod document;
pub mod document_utils;
pub mod types;

// re-exports
pub use document_utils::DocumentFactory;
pub use dpp::data_contract::{
    extra::DocumentType,
    extra::{Index, IndexProperty},
    DataContract, DataContract as Contract,
};
