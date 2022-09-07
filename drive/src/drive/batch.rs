use crate::drive::flags::StorageFlags;
use grovedb::batch::{GroveDbOp, GroveDbOpMode, KeyInfo, Op};
use grovedb::Element;
use dpp::identity::KeyType;

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
        self.operations.push(GroveDbOp {
            path,
            key,
            op: Op::Insert {
                element: Element::empty_tree(),
            },
            mode: GroveDbOpMode::RunOp
        })
    }

    pub fn add_insert_empty_tree_with_flags(
        &mut self,
        path: Vec<Vec<u8>>,
        key: Vec<u8>,
        storage_flags: Option<&StorageFlags>,
    ) {
        self.operations.push(GroveDbOp::RunOp {
            path,
            key,
            op: Op::Insert {
                element: Element::empty_tree_with_flags(storage_flags.to_some_element_flags()),
            },
        })
    }

    pub fn add_delete(&mut self, path: Vec<Vec<u8>>, key: Vec<u8>) {
        self.operations.push(GroveDbOp::RunOp {
            path,
            key,
            op: Op::Delete,
        })
    }

    pub fn add_insert(&mut self, path: Vec<Vec<u8>>, key: Vec<u8>, element: Element) {
        self.operations.push(GroveDbOp::RunOp {
            path,
            key,
            op: Op::Insert { element },
        })
    }

    pub fn add_worst_case_insert_empty_tree(&mut self, path: Vec<KeyInfo>, key: KeyInfo) {
        self.operations.push(GroveDbOp::insert_worst_case_op(path, key, Element::empty_tree()));
    }

    pub fn add_worst_case_insert_empty_tree_with_flags(
        &mut self,
        path: Vec<KeyInfo>,
        key: KeyInfo,
        storage_flags: &StorageFlags,
    ) {
        self.operations.push(GroveDbOp::insert_worst_case_op(path, key, Element::empty_tree_with_flags(storage_flags.to_some_element_flags())));
    }

    pub fn add_worst_case_delete(&mut self, path: Vec<KeyInfo>, key: KeyInfo) {
        self.operations.push(GroveDbOp::delete_worst_case_op(path, key));
    }

    pub fn add_worst_case_insert(&mut self, path: Vec<KeyInfo>, key: KeyInfo, element: Element) {
        self.operations.push(GroveDbOp::insert_worst_case_op(path, key, element));
    }
}
