use grovedb::batch::{GroveDbOp, Op};
use grovedb::Element;

pub type GroveDbOpBatch = Vec<GroveDbOp>;

impl GroveDbOpBatch {
    pub fn insert_empty_tree(&mut self, path: Vec<Vec<u8>>, key: Vec<u8>) {
        self.push( GroveDbOp{
            path,
            key,
            op: Op::Insert { element: Element::empty_tree()}
        })
    }

    pub fn delete(&mut self, path: Vec<Vec<u8>>, key: Vec<u8>) {
        self.push( GroveDbOp{
            path,
            key,
            op: Op::Delete
        })
    }

    pub fn insert(&mut self, path: Vec<Vec<u8>>, key: Vec<u8>, element: Element) {
        self.push( GroveDbOp{
            path,
            key,
            op: Op::Insert { element }
        })
    }
}