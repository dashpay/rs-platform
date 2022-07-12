pub mod operations_factory;
pub mod paths;
pub mod tree_key_constants;

pub struct Epoch {
    pub index: u16,
    pub key: [u8; 2],
}

impl Epoch {
    pub fn new(index: u16) -> Epoch {
        Epoch {
            index,
            key: index.to_be_bytes(),
        }
    }
}
