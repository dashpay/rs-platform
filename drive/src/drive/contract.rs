use crate::contract::Contract;
use crate::drive::defaults::DEFAULT_HASH_SIZE;
use crate::drive::object_size_info::KeyInfo::{KeyRef, KeySize};
use crate::drive::object_size_info::KeyValueInfo::KeyRefRequest;
use crate::drive::object_size_info::PathKeyElementInfo::{
    PathFixedSizeKeyElement, PathKeyElementSize,
};
use crate::drive::object_size_info::PathKeyInfo::{PathFixedSizeKeyRef, PathKeySize};
use crate::drive::{contract_documents_path, defaults, Drive, RootTree};
use crate::error::drive::DriveError;
use crate::error::Error;
use crate::fee::calculate_fee;
use crate::fee::op::{InsertOperation, QueryOperation};
use grovedb::{Element, TransactionArg};
use std::sync::Arc;

fn contract_root_path(contract_id: &[u8]) -> [&[u8]; 2] {
    [
        Into::<&[u8; 1]>::into(RootTree::ContractDocuments),
        contract_id,
    ]
}

fn contract_keeping_history_storage_path(contract_id: &[u8]) -> [&[u8]; 3] {
    [
        Into::<&[u8; 1]>::into(RootTree::ContractDocuments),
        contract_id,
        &[0],
    ]
}

fn contract_keeping_history_storage_time_reference_path(
    contract_id: &[u8],
    encoded_time: Vec<u8>,
) -> Vec<Vec<u8>> {
    vec![
        Into::<&[u8; 1]>::into(RootTree::ContractDocuments).to_vec(),
        contract_id.to_vec(),
        vec![0],
        encoded_time,
    ]
}

impl Drive {
    fn add_contract_to_storage(
        &self,
        contract_bytes: Element,
        contract: &Contract,
        block_time: f64,
        apply: bool,
        transaction: TransactionArg,
        insert_operations: &mut Vec<InsertOperation>,
    ) -> Result<(), Error> {
        let contract_root_path = contract_root_path(&contract.id);
        if contract.keeps_history {
            let key_info = if apply { KeyRef(&[0]) } else { KeySize(1) };
            self.grove_insert_empty_tree(
                contract_root_path,
                key_info,
                transaction,
                insert_operations,
            )?;
            let encoded_time = crate::contract::types::encode_float(block_time)?;
            let contract_keeping_history_storage_path =
                contract_keeping_history_storage_path(&contract.id);
            let path_key_element_info = if apply {
                PathFixedSizeKeyElement((
                    contract_keeping_history_storage_path,
                    encoded_time.as_slice(),
                    contract_bytes,
                ))
            } else {
                PathKeyElementSize((
                    defaults::BASE_CONTRACT_KEEPING_HISTORY_STORAGE_PATH_SIZE,
                    defaults::DEFAULT_FLOAT_SIZE,
                    contract_bytes.byte_size(),
                ))
            };
            self.grove_insert(path_key_element_info, transaction, insert_operations)?;

            // we should also insert a reference at 0 to the current value
            let contract_storage_path =
                contract_keeping_history_storage_time_reference_path(&contract.id, encoded_time);
            let path_key_element_info = if apply {
                PathFixedSizeKeyElement((
                    contract_keeping_history_storage_path,
                    &[0],
                    Element::Reference(contract_storage_path),
                ))
            } else {
                PathKeyElementSize((
                    defaults::BASE_CONTRACT_KEEPING_HISTORY_STORAGE_PATH_SIZE,
                    1,
                    defaults::BASE_CONTRACT_KEEPING_HISTORY_STORAGE_PATH_SIZE
                        + defaults::DEFAULT_FLOAT_SIZE,
                ))
            };
            self.grove_insert(path_key_element_info, transaction, insert_operations)?;
        } else {
            // the contract is just stored at key 0
            let path_key_element_info = if apply {
                PathFixedSizeKeyElement((contract_root_path, &[0], contract_bytes))
            } else {
                PathKeyElementSize((
                    defaults::BASE_CONTRACT_ROOT_PATH_SIZE,
                    1,
                    contract_bytes.byte_size(),
                ))
            };
            self.grove_insert(path_key_element_info, transaction, insert_operations)?;
        }
        Ok(())
    }

    fn insert_contract(
        &self,
        contract_bytes: Element,
        contract: &Contract,
        block_time: f64,
        apply: bool,
        transaction: TransactionArg,
        insert_operations: &mut Vec<InsertOperation>,
    ) -> Result<(), Error> {
        let key_info = if apply {
            KeyRef(contract.id.as_slice())
        } else {
            KeySize(DEFAULT_HASH_SIZE)
        };
        self.grove_insert_empty_tree(
            [Into::<&[u8; 1]>::into(RootTree::ContractDocuments).as_slice()],
            key_info,
            transaction,
            insert_operations,
        )?;

        self.add_contract_to_storage(
            contract_bytes,
            contract,
            block_time,
            apply,
            transaction,
            insert_operations,
        )?;

        // the documents
        let contract_root_path = contract_root_path(&contract.id);
        let key_info = if apply { KeyRef(&[1]) } else { KeySize(1) };
        self.grove_insert_empty_tree(contract_root_path, key_info, transaction, insert_operations)?;

        // next we should store each document type
        // right now we are referring them by name
        // toDo: change this to be a reference by index
        let contract_documents_path = contract_documents_path(&contract.id);

        for (type_key, document_type) in &contract.document_types {
            let key_info = if apply {
                KeyRef(type_key.as_bytes())
            } else {
                KeySize(type_key.as_bytes().len())
            };
            self.grove_insert_empty_tree(
                contract_documents_path,
                key_info,
                transaction,
                insert_operations,
            )?;

            let type_path = [
                contract_documents_path[0],
                contract_documents_path[1],
                contract_documents_path[2],
                type_key.as_bytes(),
            ];

            // primary key tree
            let key_info = if apply { KeyRef(&[0]) } else { KeySize(1) };
            self.grove_insert_empty_tree(type_path, key_info, transaction, insert_operations)?;

            // for each type we should insert the indices that are top level
            for index in document_type.top_level_indices()? {
                // toDo: change this to be a reference by index
                let key_info = if apply {
                    KeyRef(index.name.as_bytes())
                } else {
                    KeySize(index.name.as_bytes().len())
                };
                self.grove_insert_empty_tree(type_path, key_info, transaction, insert_operations)?;
            }
        }

        Ok(())
    }

    fn update_contract(
        &self,
        contract_bytes: Element,
        contract: &Contract,
        original_contract: &Contract,
        block_time: f64,
        apply: bool,
        transaction: TransactionArg,
        query_operations: &mut Vec<QueryOperation>,
        insert_operations: &mut Vec<InsertOperation>,
    ) -> Result<(), Error> {
        if original_contract.readonly {
            return Err(Error::Drive(DriveError::UpdatingReadOnlyImmutableContract(
                "contract is readonly",
            )));
        }

        if contract.readonly {
            return Err(Error::Drive(DriveError::ChangingContractToReadOnly(
                "contract can not be changed to readonly",
            )));
        }

        if contract.keeps_history ^ original_contract.keeps_history {
            return Err(Error::Drive(DriveError::ChangingContractKeepsHistory(
                "contract can not change whether it keeps history",
            )));
        }

        if contract.documents_keep_history_contract_default
            ^ original_contract.documents_keep_history_contract_default
        {
            return Err(Error::Drive(
                DriveError::ChangingContractDocumentsKeepsHistoryDefault(
                    "contract can not change the default of whether documents keeps history",
                ),
            ));
        }

        if contract.documents_mutable_contract_default
            ^ original_contract.documents_mutable_contract_default
        {
            return Err(Error::Drive(
                DriveError::ChangingContractDocumentsMutabilityDefault(
                    "contract can not change the default of whether documents are mutable",
                ),
            ));
        }

        // this will override the previous contract if we do not keep history
        self.add_contract_to_storage(
            contract_bytes,
            contract,
            block_time,
            apply,
            transaction,
            insert_operations,
        )?;

        let contract_documents_path = contract_documents_path(&contract.id);
        for (type_key, document_type) in &contract.document_types {
            let original_document_type = &original_contract.document_types.get(type_key);
            if let Some(original_document_type) = original_document_type {
                if original_document_type.documents_mutable ^ document_type.documents_mutable {
                    return Err(Error::Drive(DriveError::ChangingDocumentTypeMutability(
                        "contract can not change whether a specific document type is mutable",
                    )));
                }
                if original_document_type.documents_keep_history
                    ^ document_type.documents_keep_history
                {
                    return Err(Error::Drive(DriveError::ChangingDocumentTypeKeepsHistory(
                        "contract can not change whether a specific document type keeps history",
                    )));
                }

                let type_path = [
                    contract_documents_path[0],
                    contract_documents_path[1],
                    contract_documents_path[2],
                    type_key.as_bytes(),
                ];

                // for each type we should insert the indices that are top level
                for index in document_type.top_level_indices()? {
                    // toDo: we can save a little by only inserting on new indexes
                    let path_key_info = if apply {
                        PathFixedSizeKeyRef((type_path, index.name.as_bytes()))
                    } else {
                        PathKeySize((
                            defaults::BASE_CONTRACT_DOCUMENTS_PATH + type_key.as_bytes().len(),
                            index.name.as_bytes().len(),
                        ))
                    };
                    self.grove_insert_empty_tree_if_not_exists(
                        path_key_info,
                        transaction,
                        query_operations,
                        insert_operations,
                    )?;
                }
            } else {
                // We can just insert this directly because the original document type already exists
                let key_info = if apply {
                    KeyRef(type_key.as_bytes())
                } else {
                    KeySize(type_key.as_bytes().len())
                };
                self.grove_insert_empty_tree(
                    contract_documents_path,
                    key_info,
                    transaction,
                    insert_operations,
                )?;

                let type_path = [
                    contract_documents_path[0],
                    contract_documents_path[1],
                    contract_documents_path[2],
                    type_key.as_bytes(),
                ];

                // primary key tree
                let key_info = if apply { KeyRef(&[0]) } else { KeySize(1) };
                self.grove_insert_empty_tree(type_path, key_info, transaction, insert_operations)?;

                // for each type we should insert the indices that are top level
                for index in document_type.top_level_indices()? {
                    // toDo: change this to be a reference by index
                    let key_info = if apply {
                        KeyRef(index.name.as_bytes())
                    } else {
                        KeySize(index.name.as_bytes().len())
                    };
                    self.grove_insert_empty_tree(
                        type_path,
                        key_info,
                        transaction,
                        insert_operations,
                    )?;
                }
            }
        }

        Ok(())
    }

    pub fn apply_contract_cbor(
        &self,
        contract_cbor: Vec<u8>,
        contract_id: Option<[u8; 32]>,
        block_time: f64,
        transaction: TransactionArg,
    ) -> Result<(i64, u64), Error> {
        // first we need to deserialize the contract
        let contract = Contract::from_cbor(&contract_cbor, contract_id)?;
        self.apply_contract(&contract, contract_cbor, block_time, transaction)
    }

    pub fn get_contract(
        &self,
        contract_id: [u8; 32],
        transaction: TransactionArg,
    ) -> Result<Option<Arc<Contract>>, Error> {
        let cached_contracts = self.cached_contracts.borrow();
        match cached_contracts.get(&contract_id) {
            None => self.fetch_contract(contract_id, transaction),
            Some(contract) => {
                let contract_ref = Arc::clone(&contract);
                Ok(Some(contract_ref))
            }
        }
    }

    pub fn get_cached_contract(
        &self,
        contract_id: [u8; 32],
    ) -> Result<Option<Arc<Contract>>, Error> {
        let cached_contracts = self.cached_contracts.borrow();
        match cached_contracts.get(&contract_id) {
            None => Ok(None),
            Some(contract) => {
                let contract_ref = Arc::clone(&contract);
                Ok(Some(contract_ref))
            }
        }
    }

    pub fn fetch_contract(
        &self,
        contract_id: [u8; 32],
        transaction: TransactionArg,
    ) -> Result<Option<Arc<Contract>>, Error> {
        let stored_element = self
            .grove
            .get(contract_root_path(&contract_id), &[0], transaction)?;
        if let Element::Item(stored_contract_bytes) = stored_element {
            let contract = Arc::new(Contract::from_cbor(&stored_contract_bytes, None)?);
            let cached_contracts = self.cached_contracts.borrow();
            cached_contracts.insert(contract_id, Arc::clone(&contract));
            Ok(Some(Arc::clone(&contract)))
        } else {
            Err(Error::Drive(DriveError::CorruptedContractPath(
                "contract path did not refer to a contract element",
            )))
        }
    }

    pub fn apply_contract(
        &self,
        contract: &Contract,
        contract_serialization: Vec<u8>,
        block_time: f64,
        transaction: TransactionArg,
    ) -> Result<(i64, u64), Error> {
        self.run_contract(
            contract,
            contract_serialization,
            block_time,
            true,
            transaction,
        )
    }

    pub fn fees_for_contract(
        &self,
        contract: &Contract,
        contract_serialization: Vec<u8>,
        block_time: f64,
        transaction: TransactionArg,
    ) -> Result<(i64, u64), Error> {
        self.run_contract(
            contract,
            contract_serialization,
            block_time,
            false,
            transaction,
        )
    }

    fn run_contract(
        &self,
        contract: &Contract,
        contract_serialization: Vec<u8>,
        block_time: f64,
        apply: bool,
        transaction: TransactionArg,
    ) -> Result<(i64, u64), Error> {
        let mut query_operations: Vec<QueryOperation> = vec![];
        let mut insert_operations: Vec<InsertOperation> = vec![];

        // overlying structure
        let mut already_exists = false;
        let mut original_contract_stored_data = vec![];

        if let Ok(Some(stored_element)) = self.grove_get(
            contract_root_path(&contract.id),
            KeyRefRequest(&[0]),
            transaction,
            &mut query_operations,
        ) {
            already_exists = true;
            match stored_element {
                Element::Item(stored_contract_bytes) => {
                    if contract_serialization != stored_contract_bytes {
                        original_contract_stored_data = stored_contract_bytes;
                    }
                }
                _ => {
                    already_exists = false;
                }
            }
        };

        let contract_element = Element::Item(contract_serialization);

        if already_exists {
            if !original_contract_stored_data.is_empty() {
                let original_contract = Contract::from_cbor(&original_contract_stored_data, None)?;
                // if the contract is not mutable update_contract will return an error
                self.update_contract(
                    contract_element,
                    contract,
                    &original_contract,
                    block_time,
                    apply,
                    transaction,
                    &mut query_operations,
                    &mut insert_operations,
                )?;
            }
        } else {
            self.insert_contract(
                contract_element,
                contract,
                block_time,
                apply,
                transaction,
                &mut insert_operations,
            )?;
        }
        let fees = calculate_fee(None, Some(query_operations), Some(insert_operations), None)?;
        Ok(fees)
    }
}
