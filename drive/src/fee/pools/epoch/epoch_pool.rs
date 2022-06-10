use grovedb::{Element, TransactionArg};

use crate::drive::Drive;
use crate::error::Error;
use crate::fee::pools::fee_pools::FeePools;

use super::constants;

pub struct EpochPool<'e> {
    pub index: u16,
    key: [u8; 2],
    pub drive: &'e Drive,
}

impl<'e> EpochPool<'e> {
    pub fn new(index: u16, drive: &Drive) -> EpochPool {
        EpochPool {
            index,
            key: index.to_le_bytes(),
            drive,
        }
    }

    pub fn init(&self, transaction: TransactionArg) -> Result<(), Error> {
        // init epoch tree
        self.drive
            .grove
            .insert(
                FeePools::get_path(),
                &self.key,
                Element::empty_tree(),
                transaction,
            )
            .map_err(Error::GroveDB)?;

        // init storage fee item to 0
        self.drive
            .grove
            .insert(
                self.get_path(),
                constants::KEY_STORAGE_FEE.as_bytes(),
                Element::Item(0f64.to_le_bytes().to_vec()),
                transaction,
            )
            .map_err(Error::GroveDB)
    }

    pub fn get_path(&self) -> [&[u8]; 2] {
        [FeePools::get_path()[0], &self.key]
    }

    pub fn get_proposers_path(&self) -> [&[u8]; 3] {
        [
            FeePools::get_path()[0],
            &self.key,
            constants::KEY_PROPOSERS.as_bytes(),
        ]
    }
}
