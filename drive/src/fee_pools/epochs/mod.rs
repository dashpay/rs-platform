pub mod epoch_key_constants;
pub mod operations_factory;
pub mod paths;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
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

// TODO Would be more convenient to have all methods here,
//   because when you jump to implementation you arriving to this file and don't see any
