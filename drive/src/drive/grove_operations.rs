use grovedb::{Element, TransactionArg};
use crate::contract::flags::StorageFlags;
use crate::drive::Drive;
use crate::drive::object_size_info::{KeyInfo, KeyValueInfo, PathKeyElementInfo, PathKeyInfo};
use crate::drive::object_size_info::KeyInfo::{Key, KeyRef, KeySize};
use crate::drive::object_size_info::KeyValueInfo::{KeyRefRequest, KeyValueMaxSize};
use crate::drive::object_size_info::PathKeyElementInfo::{PathFixedSizeKeyElement, PathKeyElement, PathKeyElementSize};
use crate::drive::object_size_info::PathKeyInfo::{PathFixedSizeKey, PathFixedSizeKeyRef, PathKey, PathKeyRef, PathKeySize};
use crate::error::drive::DriveError;
use crate::error::Error;
use crate::fee::op::{InsertOperation, QueryOperation};
use crate::query::GroveError;

impl Drive {
    fn grove_insert_empty_tree<'a, 'c, P>(
        &'a self,
        path: P,
        key_info: KeyInfo<'c>,
        storage_flags: &StorageFlags,
        transaction: TransactionArg,
        apply: bool,
        insert_operations: &mut Vec<InsertOperation>,
    ) -> Result<(), Error>
        where
            P: IntoIterator<Item = &'c [u8]>,
            <P as IntoIterator>::IntoIter: ExactSizeIterator + DoubleEndedIterator + Clone,
    {
        match key_info {
            KeyRef(key) => {
                let path_items: Vec<Vec<u8>> = path.into_iter().map(Vec::from).collect();
                insert_operations.push(InsertOperation::for_empty_tree(path_items, key.to_vec(), storage_flags));
                if apply {
                    self.grove
                        .insert(
                            path,
                            key,
                            Element::empty_tree_with_flags(storage_flags.to_element_flags()),
                            transaction,
                        )
                        .map_err(Error::GroveDB)?
                }
                Ok(())
            }
            KeySize(key_max_length) => {
                insert_operations.push(InsertOperation::for_worst_case_key_value_size(key_max_length, 0));
                Ok(())
            }
            Key(_) => Err(Error::Drive(DriveError::GroveDBInsertion(
                "only a key ref can be inserted into groveDB",
            ))),
        }
    }

    fn grove_insert_empty_tree_if_not_exists<'a, 'c, const N: usize>(
        &'a self,
        path_key_info: PathKeyInfo<'c, N>,
        storage_flags: &StorageFlags,
        transaction: TransactionArg,
        apply: bool,
        query_operations: &mut Vec<QueryOperation>,
        insert_operations: &mut Vec<InsertOperation>,
    ) -> Result<bool, Error> {
        match path_key_info {
            PathKeyRef((path, key)) => {
                let path = path.iter().map(|x| x.as_slice());
                let inserted = if apply {
                    self.grove.insert_if_not_exists(
                        path.clone(),
                        key,
                        Element::empty_tree_with_flags(storage_flags.to_element_flags()),
                        transaction,
                    )?
                }
                if inserted {
                    insert_operations.push(InsertOperation::for_empty_tree(key.len()));
                }
                query_operations.push(QueryOperation::for_key_check_in_path(key.len(), path));
                Ok(inserted)
            }
            PathKeySize((path_length, key_length)) => {
                insert_operations.push(InsertOperation::for_worst_case_key_value_size(key_length, 0));
                query_operations.push(QueryOperation::for_key_check_with_path_length(
                    key_length,
                    path_length,
                ));
                Ok(true)
            }
            PathKey((path, key)) => {
                let path = path.iter().map(|x| x.as_slice());
                let inserted = if apply {
                    self.grove.insert_if_not_exists(
                        path.clone(),
                        key.as_slice(),
                        Element::empty_tree_with_flags(storage_flags.to_element_flags()),
                        transaction,
                    )?
                } else {
                    let mut path_items: Vec<Vec<u8>> = path.clone().map(Vec::from).collect();
                    path_items.push(key.clone());
                    let exists = self
                        .transient_batch_inserts
                        .borrow_mut()
                        .contains(&path_items)
                        || self.transient_inserts.borrow_mut().contains(&path_items);
                    if !exists {
                        self.transient_batch_inserts.borrow_mut().insert(path_items);
                    }
                    !exists
                };
                if inserted {
                    insert_operations.push(InsertOperation::for_empty_tree(key.len()));
                }
                query_operations.push(QueryOperation::for_key_check_in_path(key.len(), path));
                Ok(inserted)
            }
            PathFixedSizeKey((path, key)) => {
                let path = path.into_iter();
                let inserted = if apply {
                    self.grove.insert_if_not_exists(
                        path.clone(),
                        key.as_slice(),
                        Element::empty_tree_with_flags(storage_flags.to_element_flags()),
                        transaction,
                    )?
                } else {
                    let mut path_items: Vec<Vec<u8>> = path.clone().map(Vec::from).collect();
                    path_items.push(key.clone());
                    let exists = self
                        .transient_batch_inserts
                        .borrow_mut()
                        .contains(&path_items)
                        || self.transient_inserts.borrow_mut().contains(&path_items);
                    if !exists {
                        self.transient_batch_inserts.borrow_mut().insert(path_items);
                    }
                    !exists
                };
                if inserted {
                    insert_operations.push(InsertOperation::for_empty_tree(key.len()));
                }
                query_operations.push(QueryOperation::for_key_check_in_path(key.len(), path));
                Ok(inserted)
            }
            PathFixedSizeKeyRef((path, key)) => {
                let path = path.into_iter();
                let inserted = if apply {
                    self.grove.insert_if_not_exists(
                        path.clone(),
                        key,
                        Element::empty_tree_with_flags(storage_flags.to_element_flags()),
                        transaction,
                    )?
                } else {
                    let mut path_items: Vec<Vec<u8>> = path.clone().map(Vec::from).collect();
                    path_items.push(Vec::from(key));
                    let exists = self
                        .transient_batch_inserts
                        .borrow_mut()
                        .contains(&path_items)
                        || self
                        .transient_batch_inserts
                        .borrow_mut()
                        .contains(&path_items);
                    if !exists {
                        self.transient_batch_inserts.borrow_mut().insert(path_items);
                    }
                    !exists
                };
                if inserted {
                    insert_operations.push(InsertOperation::for_empty_tree(key.len()));
                }
                query_operations.push(QueryOperation::for_key_check_in_path(key.len(), path));
                Ok(inserted)
            }
        }
    }

    fn grove_insert<'a, 'c, const N: usize>(
        &'a self,
        path_key_element_info: PathKeyElementInfo<'c, N>,
        transaction: TransactionArg,
        apply: bool,
        insert_operations: &mut Vec<InsertOperation>,
    ) -> Result<(), Error> {
        match path_key_element_info {
            PathKeyElement((path, key, element)) => {
                let path = path.iter().map(|x| x.as_slice());
                insert_operations.push(InsertOperation::for_key_value(key.len(), &element));
                if apply {
                    self.grove
                        .insert(path, key, element, transaction)
                        .map_err(Error::GroveDB)
                } else {
                    Ok(())
                }
            }
            PathKeyElementSize((_path_max_length, key_max_length, element_max_size)) => {
                insert_operations.push(InsertOperation::for_key_value_size(
                    key_max_length,
                    element_max_size,
                ));
                Ok(())
            }
            PathFixedSizeKeyElement((path, key, element)) => {
                insert_operations.push(InsertOperation::for_key_value(key.len(), &element));
                if apply {
                    self.grove
                        .insert(path, key, element, transaction)
                        .map_err(Error::GroveDB)
                } else {
                    Ok(())
                }
            }
        }
    }

    fn grove_insert_if_not_exists<'a, 'c, const N: usize>(
        &'a self,
        path_key_element_info: PathKeyElementInfo<'c, N>,
        transaction: TransactionArg,
        apply: bool,
        query_operations: &mut Vec<QueryOperation>,
        insert_operations: &mut Vec<InsertOperation>,
    ) -> Result<bool, Error> {
        match path_key_element_info {
            PathKeyElement((path, key, element)) => {
                let path_iter = path.iter().map(|x| x.as_slice());
                let insert_operation = InsertOperation::for_key_value(key.len(), &element);
                let query_operation =
                    QueryOperation::for_key_check_in_path(key.len(), path_iter.clone());
                let inserted = if apply {
                    self.grove
                        .insert_if_not_exists(path_iter, key, element, transaction)?
                } else {
                    let mut path_items: Vec<Vec<u8>> = path.into_iter().map(Vec::from).collect();
                    path_items.push(Vec::from(key));
                    let exists = self
                        .transient_batch_inserts
                        .borrow_mut()
                        .contains(&path_items)
                        || self.transient_inserts.borrow_mut().contains(&path_items);
                    if !exists {
                        self.transient_batch_inserts.borrow_mut().insert(path_items);
                    }
                    !exists
                };
                if inserted {
                    insert_operations.push(insert_operation);
                }
                query_operations.push(query_operation);
                Ok(inserted)
            }
            PathKeyElementSize((path_size, key_max_length, element_max_size)) => {
                let insert_operation =
                    InsertOperation::for_key_value_size(key_max_length, element_max_size);
                let query_operation =
                    QueryOperation::for_key_check_with_path_length(key_max_length, path_size);
                insert_operations.push(insert_operation);
                query_operations.push(query_operation);
                Ok(true)
            }
            PathFixedSizeKeyElement((path, key, element)) => {
                let path_iter = path.into_iter();
                let insert_operation = InsertOperation::for_key_value(key.len(), &element);
                let query_operation =
                    QueryOperation::for_key_check_in_path(key.len(), path_iter.clone());
                let inserted = if apply {
                    self.grove
                        .insert_if_not_exists(path_iter, key, element, transaction)?
                } else {
                    let mut path_items: Vec<Vec<u8>> = path.into_iter().map(Vec::from).collect();
                    path_items.push(Vec::from(key));
                    let exists = self
                        .transient_batch_inserts
                        .borrow_mut()
                        .contains(&path_items)
                        || self.transient_inserts.borrow_mut().contains(&path_items);
                    if !exists {
                        self.transient_batch_inserts.borrow_mut().insert(path_items);
                    }
                    !exists
                };

                if inserted {
                    insert_operations.push(insert_operation);
                }
                query_operations.push(query_operation);
                Ok(inserted)
            }
        }
    }

    pub(crate) fn batch_insert_empty_tree<'a, 'c, P>(
        &'a self,
        path: P,
        key_info: KeyInfo<'c>,
        storage_flags: &StorageFlags,
        insert_operations: &mut Vec<InsertOperation>,
    ) -> Result<(), Error>
        where
            P: IntoIterator<Item = &'c [u8]>,
            <P as IntoIterator>::IntoIter: ExactSizeIterator + DoubleEndedIterator + Clone,
    {
        match key_info {
            KeyRef(key) => {
                let path_items: Vec<Vec<u8>> = path.into_iter().map(Vec::from).collect();
                insert_operations.push(InsertOperation::for_empty_tree(path_items, key.to_vec(), storage_flags));
                Ok(())
            }
            KeySize(key_max_length) => {
                insert_operations.push(InsertOperation::for_worst_case_key_value_size(key_max_length, 0));
                Ok(())
            }
            Key(_) => Err(Error::Drive(DriveError::GroveDBInsertion(
                "only a key ref can be inserted into groveDB",
            ))),
        }
    }

    fn grove_has_raw<'p, P>(
        &self,
        path: P,
        key: &'p [u8],
        transaction: TransactionArg,
    ) -> Result<bool, Error>
        where
            P: IntoIterator<Item = &'p [u8]>,
            <P as IntoIterator>::IntoIter: ExactSizeIterator + DoubleEndedIterator + Clone,
    {
        let query_result = self.grove.has_raw(path, key, transaction);
        match query_result {
            Err(GroveError::PathKeyNotFound(_)) | Err(GroveError::PathNotFound(_)) => {
                Ok(false)
            }
            _ => {
                Ok(query_result?)
            }
        }
    }

    pub(crate) fn batch_insert_empty_tree_if_not_exists<'a, 'c, const N: usize>(
        &'a self,
        path_key_info: PathKeyInfo<'c, N>,
        storage_flags: &StorageFlags,
        transaction: TransactionArg,
        query_operations: &mut Vec<QueryOperation>,
        insert_operations: &mut Vec<InsertOperation>,
    ) -> Result<bool, Error> {
        match path_key_info {
            PathKeyRef((path, key)) => {
                let path_iter: Vec<&[u8]> = path.iter().map(|x| x.as_slice()).collect();
                let has_raw = self.grove_has_raw(path_iter.clone(), key, transaction)?;
                query_operations.push(QueryOperation::for_key_check_in_path(key.len(), path_iter));
                if has_raw == false {
                    insert_operations.push(InsertOperation::for_empty_tree(path, key.to_vec(), storage_flags));
                }
                Ok(!has_raw)
            }
            PathKeySize((path_length, key_length)) => {
                insert_operations.push(InsertOperation::for_worst_case_key_value_size(key_length, 0));

                query_operations.push(QueryOperation::for_key_check_with_path_length(
                    key_length,
                    path_length,
                ));
                Ok(true)
            }
            PathKey((path, key)) => {
                let path_iter: Vec<&[u8]> = path.iter().map(|x| x.as_slice()).collect();
                let has_raw = self.grove_has_raw(path_iter.clone(), key.as_slice(), transaction)?;
                query_operations.push(QueryOperation::for_key_check_in_path(key.len(), path_iter));
                if has_raw == false {
                    insert_operations.push(InsertOperation::for_empty_tree(path, key.to_vec(), storage_flags));
                }
                Ok(!has_raw)
            }
            PathFixedSizeKey((path, key)) => {
                let has_raw = self.grove_has_raw(path.clone(), key.as_slice(), transaction)?;
                if has_raw == false {
                    let path_items: Vec<Vec<u8>> = path.into_iter().map(Vec::from).collect();
                    insert_operations.push(InsertOperation::for_empty_tree(path_items, key.to_vec(), storage_flags));
                }
                query_operations.push(QueryOperation::for_key_check_in_path(key.len(), path));
                Ok(!has_raw)
            }
            PathFixedSizeKeyRef((path, key)) => {
                let has_raw = self.grove_has_raw(path.clone(), key, transaction)?;
                if has_raw == false {
                    let path_items: Vec<Vec<u8>> = path.into_iter().map(Vec::from).collect();
                    insert_operations.push(InsertOperation::for_empty_tree(path_items, key.to_vec(), storage_flags));
                }
                query_operations.push(QueryOperation::for_key_check_in_path(key.len(), path));
                Ok(!has_raw)
            }
        }
    }

    pub(crate) fn batch_insert<const N: usize>(
        &self,
        path_key_element_info: PathKeyElementInfo<N>,
        insert_operations: &mut Vec<InsertOperation>,
    ) -> Result<(), Error> {
        match path_key_element_info {
            PathKeyElement((path, key, element)) => {
                insert_operations.push(InsertOperation::for_path_key_element(path, key.to_vec(), element));
                Ok(())
            }
            PathKeyElementSize((_path_max_length, key_max_length, element_max_size)) => {
                insert_operations.push(InsertOperation::for_worst_case_key_value_size(
                    key_max_length,
                    element_max_size,
                ));
                Ok(())
            }
            PathFixedSizeKeyElement((path, key, element)) => {
                let path_items: Vec<Vec<u8>> = path.into_iter().map(Vec::from).collect();
                insert_operations.push(InsertOperation::for_path_key_element(path_items, key.to_vec(), element));
                Ok(())
            }
        }
    }

    pub(crate) fn batch_insert_if_not_exists<'a, 'c, const N: usize>(
        &'a self,
        path_key_element_info: PathKeyElementInfo<'c, N>,
        transaction: TransactionArg,
        query_operations: &mut Vec<QueryOperation>,
        insert_operations: &mut Vec<InsertOperation>,
    ) -> Result<bool, Error> {
        match path_key_element_info {
            PathKeyElement((path, key, element)) => {
                let path_iter: Vec<&[u8]> = path.iter().map(|x| x.as_slice()).collect();
                let has_raw = self.grove_has_raw(path_iter.clone(), key, transaction)?;
                query_operations.push(QueryOperation::for_key_check_in_path(key.len(), path_iter));
                if has_raw == false {
                    insert_operations.push(InsertOperation::for_path_key_element(path, key.to_vec(), element));
                }
                Ok(!has_raw)
            }
            PathKeyElementSize((path_size, key_max_length, element_max_size)) => {
                let insert_operation =
                    InsertOperation::for_worst_case_key_value_size(key_max_length, element_max_size);
                let query_operation =
                    QueryOperation::for_key_check_with_path_length(key_max_length, path_size);
                insert_operations.push(insert_operation);
                query_operations.push(query_operation);
                Ok(true)
            }
            PathFixedSizeKeyElement((path, key, element)) => {
                let has_raw = self.grove_has_raw(path, key, transaction)?;
                if has_raw == false {
                    let path_items: Vec<Vec<u8>> = path.into_iter().map(Vec::from).collect();
                    insert_operations.push(InsertOperation::for_path_key_element(path_items, key.to_vec(), element));
                }
                query_operations.push(QueryOperation::for_key_check_in_path(key.len(), path));
                Ok(!has_raw)
            }
        }
    }

    pub(crate) fn grove_get<'a, 'c, P>(
        &'a self,
        path: P,
        key_value_info: KeyValueInfo<'c>,
        transaction: TransactionArg,
        query_operations: &mut Vec<QueryOperation>,
    ) -> Result<Option<Element>, Error>
        where
            P: IntoIterator<Item = &'c [u8]>,
            <P as IntoIterator>::IntoIter: ExactSizeIterator + DoubleEndedIterator + Clone,
    {
        let path_iter = path.into_iter();
        match key_value_info {
            KeyRefRequest(key) => {
                let item = self.grove.get(path_iter.clone(), key, transaction)?;
                query_operations.push(QueryOperation::for_value_retrieval_in_path(
                    key.len(),
                    path_iter,
                    item.serialized_byte_size(),
                ));
                Ok(Some(item))
            }
            KeyValueMaxSize((key_size, value_size)) => {
                query_operations.push(QueryOperation::for_value_retrieval_in_path(
                    key_size, path_iter, value_size,
                ));
                Ok(None)
            }
        }
    }

}