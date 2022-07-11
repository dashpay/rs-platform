use grovedb::batch::{GroveDbOp, Op};
use grovedb::Element;
use crate::drive::genesis_time::KEY_GENESIS_TIME;
use crate::drive::RootTree;

pub(super) fn update_genesis_time_operation(
    genesis_time: u64,
) -> GroveDbOp {
    GroveDbOp {
        path: vec![vec![RootTree::Pools as u8]],
        key: KEY_GENESIS_TIME.to_vec(),
        //todo make this into a Op::Replace
        op: Op::Insert {
            element: Element::Item(genesis_time.to_be_bytes().to_vec(), None)
        }
    }
}