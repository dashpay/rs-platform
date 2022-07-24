use crate::drive::fee_pools::pools_path;
use crate::drive::Drive;
use crate::error::fee::FeeError;
use crate::error::Error;
use crate::fee_pools::epochs_root_tree_key_constants::KEY_EPOCH_TO_PAY;
use grovedb::{Element, TransactionArg};

impl Drive {
    pub fn get_epoch_index_to_pay(&self, transaction: TransactionArg) -> Result<u16, Error> {
        let element = self
            .grove
            .get(pools_path(), KEY_EPOCH_TO_PAY, transaction)
            .unwrap()
            .map_err(Error::GroveDB)?;

        if let Element::Item(item, _) = element {
            Ok(u16::from_be_bytes(item.as_slice().try_into().map_err(
                |_| {
                    Error::Fee(FeeError::CorruptedProposerBlockCountItemLength(
                        "item have an invalid length",
                    ))
                },
            )?))
        } else {
            Err(Error::Fee(FeeError::CorruptedProposerBlockCountNotItem(
                "must be an item",
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    mod get_epoch_index_to_pay {
        #[test]
        fn test() {
            todo!()
        }
    }
}
