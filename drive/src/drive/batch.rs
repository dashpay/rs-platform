use crate::drive::flags::StorageFlags;
use grovedb::batch::{GroveDbOp, KeyInfo, KeyInfoPath};
use grovedb::Element;

// TODO move to GroveDB
#[derive(Debug)]
pub struct GroveDbOpBatch {
    pub(crate) operations: Vec<GroveDbOp>,
}

impl GroveDbOpBatch {
    pub fn new() -> Self {
        GroveDbOpBatch {
            operations: Vec::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.operations.len()
    }

    pub fn push(&mut self, op: GroveDbOp) {
        self.operations.push(op);
    }

    pub fn from_operations(operations: Vec<GroveDbOp>) -> Self {
        GroveDbOpBatch { operations }
    }

    pub fn add_insert_empty_tree(&mut self, path: Vec<Vec<u8>>, key: Vec<u8>) {
        self.operations
            .push(GroveDbOp::insert_run_op(path, key, Element::empty_tree()))
    }

    pub fn add_insert_empty_tree_with_flags(
        &mut self,
        path: Vec<Vec<u8>>,
        key: Vec<u8>,
        storage_flags: Option<&StorageFlags>,
    ) {
        self.operations.push(GroveDbOp::insert_run_op(
            path,
            key,
            Element::empty_tree_with_flags(StorageFlags::map_to_some_element_flags(storage_flags)),
        ))
    }

    pub fn add_delete(&mut self, path: Vec<Vec<u8>>, key: Vec<u8>) {
        self.operations.push(GroveDbOp::delete_run_op(path, key))
    }

    pub fn add_insert(&mut self, path: Vec<Vec<u8>>, key: Vec<u8>, element: Element) {
        self.operations
            .push(GroveDbOp::insert_run_op(path, key, element))
    }

    pub fn add_worst_case_insert_empty_tree(&mut self, path: Vec<KeyInfo>, key: KeyInfo) {
        self.operations.push(GroveDbOp::insert_worst_case_op(
            KeyInfoPath(path),
            key,
            Element::empty_tree(),
        ));
    }

    pub fn add_worst_case_insert_empty_tree_with_flags(
        &mut self,
        path: Vec<KeyInfo>,
        key: KeyInfo,
        storage_flags: Option<&StorageFlags>,
    ) {
        self.operations.push(GroveDbOp::insert_worst_case_op(
            KeyInfoPath(path),
            key,
            Element::empty_tree_with_flags(StorageFlags::map_to_some_element_flags(storage_flags)),
        ));
    }

    pub fn add_worst_case_delete(&mut self, path: Vec<KeyInfo>, key: KeyInfo) {
        self.operations
            .push(GroveDbOp::delete_worst_case_op(KeyInfoPath(path), key));
    }

    pub fn add_worst_case_insert(&mut self, path: Vec<KeyInfo>, key: KeyInfo, element: Element) {
        self.operations.push(GroveDbOp::insert_worst_case_op(
            KeyInfoPath(path),
            key,
            element,
        ));
    }
}
