pub mod operations_factory;
pub mod paths;
pub mod tree_key_constants;

pub struct EpochPool {
    pub index: u16,
    pub key: [u8; 2],
}

impl EpochPool {
    pub fn new(index: u16) -> EpochPool {
        EpochPool {
            index,
            key: index.to_be_bytes(),
        }
    }
}
