use crate::contract::document::Document;
use crate::contract::Contract;
use crate::drive::flags::StorageFlags;
use crate::drive::object_size_info::DocumentAndContractInfo;

/// Operations on Contracts
pub enum ContractOperationType<'a> {
    /// Deserializes a contract from CBOR and applies it.
    ApplyContractCbor {
        /// The cbor serialized contract
        contract_cbor: Vec<u8>,
        /// The contract id, if it is not present will try to recover it from the contract
        contract_id: Option<[u8; 32]>,
        /// Storage flags for the contract
        storage_flags: Option<&'a StorageFlags>,
    },
    /// Applies a contract and returns the fee for applying.
    /// If the contract already exists, an update is applied, otherwise an insert.
    ApplyContractWithSerialization {
        /// The contract
        contract: &'a Contract,
        /// The serialized contract
        contract_serialization: Vec<u8>,
        /// Storage flags for the contract
        storage_flags: Option<&'a StorageFlags>,
    },
}

/// Operations on Documents
pub enum DocumentOperationType<'a> {
    /// Deserializes a document and a contract and adds the document to the contract.
    AddSerializedDocumentForSerializedContract {
        /// The serialized document
        serialized_document: &'a [u8],
        /// The serialized contract
        serialized_contract: &'a [u8],
        /// The name of the document type
        document_type_name: &'a str,
        /// The owner id, if none is specified will try to recover from serialized document
        owner_id: Option<&'a [u8]>,
        /// Should we override the document if one already exists?
        override_document: bool,
        /// Add storage flags (like epoch, owner id, etc)
        storage_flags: Option<&'a StorageFlags>,
    },
    /// Deserializes a document and adds it to a contract.
    AddSerializedDocumentForContract {
        /// The serialized document
        serialized_document: &'a [u8],
        /// The contract
        contract: &'a Contract,
        /// The name of the document type
        document_type_name: &'a str,
        /// The owner id, if none is specified will try to recover from serialized document
        owner_id: Option<&'a [u8]>,
        /// Should we override the document if one already exists?
        override_document: bool,
        /// Add storage flags (like epoch, owner id, etc)
        storage_flags: Option<&'a StorageFlags>,
    },
    /// Adds a document to a contract.
    AddDocumentForContract {
        /// The document and contract info, also may contain the owner_id
        document_and_contract_info: DocumentAndContractInfo<'a>,
        /// Should we override the document if one already exists?
        override_document: bool,
        /// Add storage flags (like epoch, owner id, etc)
        storage_flags: Option<&'a StorageFlags>,
    },
    /// Deletes a document and returns the associated fee.
    DeleteDocumentForContract {
        /// The document id
        document_id: &'a [u8],
        /// The contract
        contract: &'a Contract,
        /// The name of the document type
        document_type_name: &'a str,
        /// The owner id, if none is specified will try to recover from serialized document
        owner_id: Option<&'a [u8]>,
    },
    /// Deletes a document and returns the associated fee.
    /// The contract CBOR is given instead of the contract itself.
    DeleteDocumentForContractCbor {
        /// The document id
        document_id: &'a [u8],
        /// The serialized contract
        contract_cbor: &'a [u8],
        /// The name of the document type
        document_type_name: &'a str,
        /// The owner id, if none is specified will try to recover from serialized document
        owner_id: Option<&'a [u8]>,
    },
    /// Updates a serialized document given a contract CBOR and returns the associated fee.
    UpdateDocumentForContractCbor {
        /// The serialized document
        serialized_document: &'a [u8],
        /// The serialized contract
        contract_cbor: &'a [u8],
        /// The name of the document type
        document_type: &'a str,
        /// The owner id, if none is specified will try to recover from serialized document
        owner_id: Option<&'a [u8]>,
        /// Add storage flags (like epoch, owner id, etc)
        storage_flags: Option<&'a StorageFlags>,
    },
    /// Updates a serialized document and returns the associated fee.
    UpdateSerializedDocumentForContract {
        /// The serialized document
        serialized_document: &'a [u8],
        /// The contract
        contract: &'a Contract,
        /// The name of the document type
        document_type: &'a str,
        /// The owner id, if none is specified will try to recover from serialized document
        owner_id: Option<&'a [u8]>,
        /// Add storage flags (like epoch, owner id, etc)
        storage_flags: Option<&'a StorageFlags>,
    },
    /// Updates a document and returns the associated fee.
    UpdateDocumentForContract {
        /// The document to update
        document: &'a Document,
        /// The document in pre-serialized form
        serialized_document: &'a [u8],
        /// The contract
        contract: &'a Contract,
        /// The name of the document type
        document_type_name: &'a str,
        /// The owner id, if none is specified will try to recover from serialized document
        owner_id: Option<&'a [u8]>,
        /// Add storage flags (like epoch, owner id, etc)
        storage_flags: Option<&'a StorageFlags>,
    },
}

/// Operations on Identities
pub enum IdentityOperationType {}

/// All types of Drive Operations
pub enum DriveOperationType<'a> {
    /// A contract operation
    ContractOperation(ContractOperationType<'a>),
    /// A document operation
    DocumentOperation(DocumentOperationType<'a>),
    /// An identity operation
    IdentityOperation(IdentityOperationType),
}
