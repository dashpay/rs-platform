use std::collections::BTreeMap;
use costs::storage_cost::removal::{Identifier, StorageRemovedBytes};
use costs::storage_cost::removal::StorageRemovedBytes::{BasicStorageRemoval, NoStorageRemoval, SectionedStorageRemoval};
use enum_map::EnumMap;
use intmap::IntMap;

use crate::error::fee::FeeError;
use crate::error::Error;
use crate::fee::op::{BaseOp, DriveCost, DriveOperation};
use crate::fee_pools::epochs::Epoch;

pub mod default_costs;
pub mod op;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct FeeResult {
    pub storage_fee: u64,
    pub processing_fee: u64,
    pub removed_from_identities: BTreeMap<Identifier, IntMap<u32>>
}

pub fn calculate_fee(
    base_operations: Option<EnumMap<BaseOp, u64>>,
    drive_operations: Option<Vec<DriveOperation>>,
    epoch: &Epoch,
) -> Result<FeeResult, Error> {
    let mut storage_cost = 0u64;
    let mut processing_cost = 0u64;
    let mut storage_removed_bytes : StorageRemovedBytes = NoStorageRemoval;
    if let Some(base_operations) = base_operations {
        for (base_op, count) in base_operations.iter() {
            match base_op.cost().checked_mul(*count) {
                None => return Err(Error::Fee(FeeError::Overflow("overflow error"))),
                Some(cost) => match processing_cost.checked_add(cost) {
                    None => return Err(Error::Fee(FeeError::Overflow("overflow error"))),
                    Some(value) => processing_cost = value,
                },
            }
        }
    }

    if let Some(drive_operations) = drive_operations {
        // println!("{:#?}", drive_operations);
        for drive_operation in DriveOperation::consume_to_costs(drive_operations)? {
            match processing_cost.checked_add(drive_operation.ephemeral_cost(epoch)?) {
                None => return Err(Error::Fee(FeeError::Overflow("overflow error"))),
                Some(value) => processing_cost = value,
            }

            match storage_cost.checked_add(drive_operation.storage_cost(epoch)?) {
                None => return Err(Error::Fee(FeeError::Overflow("overflow error"))),
                Some(value) => storage_cost = value,
            }

            storage_removed_bytes += drive_operation.storage_cost.removed_bytes;
        }
    }

    let removed_from_identities = match storage_removed_bytes {
        NoStorageRemoval => { BTreeMap::default() }
        BasicStorageRemoval(_) => {
            // this is not always considered an error
            BTreeMap::default()
        }
        SectionedStorageRemoval(s) => { s }
    };

    let fee_result = FeeResult {
        storage_fee: storage_cost,
        processing_fee: processing_cost,
        removed_from_identities
    };

    Ok(fee_result)
}
