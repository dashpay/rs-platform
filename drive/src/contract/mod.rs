use std::collections::{BTreeMap, HashMap};

use ciborium::value::{Value as CborValue, Value};
use rand::{Rng, SeedableRng};
use serde::{Deserialize, Serialize};

use document::Document;

use crate::common::{
    bool_for_system_value_from_tree_map,
    bytes_for_system_value_from_tree_map,
    cbor_map_to_btree_map,
};
use crate::contract::document_type::DocumentType;
use crate::drive::{Drive, RootTree};
use crate::drive::config::DriveEncoding;
use crate::error::contract::ContractError;
use crate::error::Error;
use crate::error::structure::StructureError;

mod defaults;
pub mod document;
pub mod types;
pub mod document_type;
pub mod index;

// contract
// - id
// - documents
//      - document_type
//          - indices
//               - properties
//                  - name
//                  - ascending
//               - unique

// Struct Definitions
#[derive(Serialize, Deserialize, Debug, PartialEq, Default)]
pub struct Contract {
    pub id: [u8; 32],
    pub document_types: BTreeMap<String, DocumentType>,
    pub keeps_history: bool,
    pub readonly: bool,
    pub documents_keep_history_contract_default: bool,
    pub documents_mutable_contract_default: bool,
}


// Struct Implementations
impl Contract {
    pub fn deserialize(
        serialized_contract: &[u8],
        contract_id: Option<[u8; 32]>,
        encoding: DriveEncoding,
    ) -> Result<Self, Error> {
        match encoding {
            DriveEncoding::DriveCbor => Contract::from_cbor(serialized_contract, contract_id),
            DriveEncoding::DriveProtobuf => {
                todo!()
            }
        }
    }

    pub fn from_cbor(contract_cbor: &[u8], contract_id: Option<[u8; 32]>) -> Result<Self, Error> {
        let (version, read_contract_cbor) = contract_cbor.split_at(4);
        if !Drive::check_protocol_version_bytes(version) {
            return Err(Error::Structure(StructureError::InvalidProtocolVersion(
                "invalid protocol version",
            )));
        }
        // Deserialize the contract
        let contract: BTreeMap<String, CborValue> = ciborium::de::from_reader(read_contract_cbor)
            .map_err(|_| {
            Error::Structure(StructureError::InvalidCBOR("unable to decode contract"))
        })?;

        // Get the contract id
        let contract_id: [u8; 32] = if let Some(contract_id) = contract_id {
            contract_id
        } else {
            bytes_for_system_value_from_tree_map(&contract, "$id")?
                .ok_or({
                    Error::Contract(ContractError::MissingRequiredKey(
                        "unable to get contract id",
                    ))
                })?
                .try_into()
                .map_err(|_| {
                    Error::Contract(ContractError::FieldRequirementUnmet(
                        "contract_id must be 32 bytes",
                    ))
                })?
        };

        // Does the contract keep history when the contract itself changes?
        let keeps_history: bool = bool_for_system_value_from_tree_map(
            &contract,
            "keepsHistory",
            crate::contract::defaults::DEFAULT_CONTRACT_KEEPS_HISTORY,
        )?;

        // Is the contract mutable?
        let readonly: bool = bool_for_system_value_from_tree_map(
            &contract,
            "readOnly",
            !crate::contract::defaults::DEFAULT_CONTRACT_MUTABILITY,
        )?;

        // Do documents in the contract keep history?
        let documents_keep_history_contract_default: bool = bool_for_system_value_from_tree_map(
            &contract,
            "documentsKeepHistoryContractDefault",
            crate::contract::defaults::DEFAULT_CONTRACT_DOCUMENTS_KEEPS_HISTORY,
        )?;

        // Are documents in the contract mutable?
        let documents_mutable_contract_default: bool = bool_for_system_value_from_tree_map(
            &contract,
            "documentsMutableContractDefault",
            crate::contract::defaults::DEFAULT_CONTRACT_DOCUMENT_MUTABILITY,
        )?;

        let definition_references = match contract.get("$defs") {
            None => BTreeMap::new(),
            Some(definition_value) => {
                let definition_map = definition_value.as_map();
                match definition_map {
                    None => BTreeMap::new(),
                    Some(key_value) => cbor_map_to_btree_map(key_value),
                }
            }
        };

        let documents_cbor_value = contract.get("documents").ok_or({
            Error::Contract(ContractError::MissingRequiredKey("unable to get documents"))
        })?;
        let contract_document_types_raw = documents_cbor_value.as_map().ok_or({
            Error::Contract(ContractError::InvalidContractStructure(
                "documents must be a map",
            ))
        })?;

        let mut contract_document_types: BTreeMap<String, DocumentType> = BTreeMap::new();

        // Build the document type hashmap
        for (type_key_value, document_type_value) in contract_document_types_raw {
            if !type_key_value.is_text() {
                return Err(Error::Contract(ContractError::InvalidContractStructure(
                    "document type name is not a string as expected",
                )));
            }

            // Make sure the document_type_value is a map
            if !document_type_value.is_map() {
                return Err(Error::Contract(ContractError::InvalidContractStructure(
                    "document type data is not a map as expected",
                )));
            }

            let document_type = DocumentType::from_cbor_value(
                type_key_value.as_text().expect("confirmed as text"),
                document_type_value.as_map().expect("confirmed as map"),
                &definition_references,
                documents_keep_history_contract_default,
                documents_mutable_contract_default,
            )?;
            contract_document_types.insert(
                String::from(type_key_value.as_text().expect("confirmed as text")),
                document_type,
            );
        }

        Ok(Contract {
            id: contract_id,
            document_types: contract_document_types,
            keeps_history,
            readonly,
            documents_keep_history_contract_default,
            documents_mutable_contract_default,
        })
    }

    pub fn root_path(&self) -> [&[u8]; 2] {
        [
            Into::<&[u8; 1]>::into(RootTree::ContractDocuments),
            &self.id,
        ]
    }

    pub fn documents_path(&self) -> [&[u8]; 3] {
        [
            Into::<&[u8; 1]>::into(RootTree::ContractDocuments),
            &self.id,
            &[1],
        ]
    }

    pub fn document_type_path<'a>(&'a self, document_type_name: &'a str) -> [&'a [u8]; 4] {
        [
            Into::<&[u8; 1]>::into(RootTree::ContractDocuments),
            &self.id,
            &[1],
            document_type_name.as_bytes(),
        ]
    }

    pub fn documents_primary_key_path<'a>(&'a self, document_type_name: &'a str) -> [&'a [u8]; 5] {
        [
            Into::<&[u8; 1]>::into(RootTree::ContractDocuments),
            &self.id,
            &[1],
            document_type_name.as_bytes(),
            &[0],
        ]
    }

    pub fn documents_with_history_primary_key_path<'a>(
        &'a self,
        document_type_name: &'a str,
        id: &'a [u8],
    ) -> [&'a [u8]; 6] {
        [
            Into::<&[u8; 1]>::into(RootTree::ContractDocuments),
            &self.id,
            &[1],
            document_type_name.as_bytes(),
            &[0],
            id,
        ]
    }

    pub fn document_type_for_name(&self, document_type_name: &str) -> Result<&DocumentType, Error> {
        self.document_types.get(document_type_name).ok_or({
            Error::Contract(ContractError::DocumentTypeNotFound(
                "can not get document type from contract",
            ))
        })
    }
}

fn reduced_value_string_representation(value: &Value) -> String {
    match value {
        Value::Integer(integer) => {
            let i: i128 = (*integer).try_into().unwrap();
            format!("{}", i)
        }
        Value::Bytes(bytes) => hex::encode(bytes),
        Value::Float(float) => {
            format!("{}", float)
        }
        Value::Text(text) => {
            let len = text.len();
            if len > 20 {
                let first_text = text.split_at(20).0.to_string();
                format!("{}[...({})]", first_text, len)
            } else {
                text.clone()
            }
        }
        Value::Bool(b) => {
            format!("{}", b)
        }
        Value::Null => "None".to_string(),
        Value::Tag(_, _) => "Tag".to_string(),
        Value::Array(_) => "Array".to_string(),
        Value::Map(_) => "Map".to_string(),
        _ => "".to_string(),
    }
}

// Helper functions
fn contract_document_types(contract: &HashMap<String, CborValue>) -> Option<&Vec<(Value, Value)>> {
    contract.get("documents").and_then(|id_cbor| {
        if let CborValue::Map(documents) = id_cbor {
            Some(documents)
        } else {
            None
        }
    })
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::common::json_document_to_cbor;
    use crate::contract::{Contract, Document};
    use crate::drive::Drive;

    #[test]
    fn test_cbor_deserialization() {
        let document_cbor = json_document_to_cbor("simple.json", Some(1));
        let (version, read_document_cbor) = document_cbor.split_at(4);
        assert!(Drive::check_protocol_version_bytes(version));
        let document: HashMap<String, ciborium::value::Value> =
            ciborium::de::from_reader(read_document_cbor).expect("cannot deserialize cbor");
        assert!(document.get("a").is_some());
    }

    #[test]
    fn test_import_contract() {
        let dashpay_cbor = json_document_to_cbor(
            "tests/supporting_files/contract/dashpay/dashpay-contract.json",
            Some(1),
        );
        let contract = Contract::from_cbor(&dashpay_cbor, None).unwrap();

        assert!(contract.documents_mutable_contract_default);
        assert!(!contract.keeps_history);
        assert!(!contract.readonly); // the contract shouldn't be readonly
        assert!(!contract.documents_keep_history_contract_default);
        assert_eq!(contract.document_types.len(), 3);
        assert!(contract.document_types.get("profile").is_some());
        assert!(
            contract
                .document_types
                .get("profile")
                .unwrap()
                .documents_mutable
        );
        assert!(contract.document_types.get("contactInfo").is_some());
        assert!(
            contract
                .document_types
                .get("contactInfo")
                .unwrap()
                .documents_mutable
        );
        assert!(contract.document_types.get("contactRequest").is_some());
        assert!(
            !contract
                .document_types
                .get("contactRequest")
                .unwrap()
                .documents_mutable
        );
        assert!(contract.document_types.get("non_existent_key").is_none());

        let contact_info_indices = &contract.document_types.get("contactInfo").unwrap().indices;
        assert_eq!(contact_info_indices.len(), 2);
        assert!(contact_info_indices[0].unique);
        assert!(!contact_info_indices[1].unique);
        assert_eq!(contact_info_indices[0].properties.len(), 3);

        assert_eq!(contact_info_indices[0].properties[0].name, "$ownerId");
        assert_eq!(
            contact_info_indices[0].properties[1].name,
            "rootEncryptionKeyIndex"
        );
        assert_eq!(
            contact_info_indices[0].properties[2].name,
            "derivationEncryptionKeyIndex"
        );

        assert!(contact_info_indices[0].properties[0].ascending);
    }
}
