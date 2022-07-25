pub mod document;
pub mod factory;

pub use dpp::data_contract::{
    extra::{DocumentField, DocumentFieldType, DocumentType},
    extra::{Index, IndexProperty},
    DataContract, DataContract as Contract,
};
pub use factory::CreateRandomDocument;
