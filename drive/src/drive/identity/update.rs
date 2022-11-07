use crate::drive::identity::{balance_from_bytes, identity_path};
use crate::drive::object_size_info::KeyValueInfo::KeyRefRequest;
use crate::drive::Drive;
use crate::error::drive::DriveError;
use crate::error::identity::IdentityError;
use crate::error::Error;
use crate::fee::op::DriveOperation;
use grovedb::Element::Item;
use grovedb::TransactionArg;
use crate::drive::defaults::{CONTRACT_MAX_SERIALIZED_SIZE, SOME_TREE_SIZE};

impl Drive {
    /// Balances are stored in the identity under key 0
    pub fn add_to_identity_balance(
        &self,
        identity_id: [u8; 32],
        added_balance: i64,
        error_if_absent: bool,
        apply: bool,
        transaction: TransactionArg,
        drive_operations: &mut Vec<DriveOperation>,
    ) -> Result<(), Error> {

        //todo ref sizes?
        let query_state_less_max_value_size = if apply {
            None
        } else {
            Some((CONTRACT_MAX_SERIALIZED_SIZE, vec![0]))
        };

        let identity_balance_element = self.grove_get(
            identity_path(identity_id.as_slice()),
            KeyRefRequest(&[0]),
            query_state_less_max_value_size,
            transaction,
            drive_operations,
        )?;
        if error_if_absent && identity_balance_element.is_none() {
            Err(Error::Identity(IdentityError::IdentityNotFound(
                "identity not found while trying to modify an identity balance",
            )))
        } else if identity_balance_element.is_none() {
            Ok(())
        } else if let Item(identity_balance_element, element_flags) =
            identity_balance_element.unwrap()
        {
            let balance = balance_from_bytes(identity_balance_element.as_slice())?;
            let new_balance = if added_balance > 0 {
                balance
                    .checked_add(added_balance as u64)
                    .ok_or(Error::Identity(IdentityError::BalanceOverflow(
                        "identity overflow error",
                    )))?
            } else {
                (balance as i64)
                    .checked_add(added_balance)
                    .ok_or(Error::Identity(IdentityError::BalanceOverflow(
                        "identity overflow error",
                    )))? as u64
            };
            self.set_identity_balance(identity_id, new_balance, element_flags, drive_operations)
        } else {
            Err(Error::Drive(DriveError::CorruptedElementType(
                "identity balance was present but was not identified as an item",
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use grovedb::Element;
    use tempfile::TempDir;

    use crate::drive::flags::StorageFlags;
    use crate::drive::Drive;
    use crate::identity::Identity;

    #[test]
    fn test_insert_identity() {
        let tmp_dir = TempDir::new().unwrap();
        let drive: Drive = Drive::open(tmp_dir).expect("expected to open Drive successfully");

        let db_transaction = drive.grove.start_transaction();

        drive
            .create_root_tree(Some(&db_transaction))
            .expect("expected to create root tree successfully");

        let identity_bytes = hex::decode("01000000a462696458203012c19b98ec0033addb36cd64b7f510670f2a351a4304b5f6994144286efdac6762616c616e636500687265766973696f6e006a7075626c69634b65797381a6626964006464617461582102abb64674c5df796559eb3cf92a84525cc1a6068e7ad9d4ff48a1f0b179ae29e164747970650067707572706f73650068726561644f6e6c79f46d73656375726974794c6576656c00").expect("expected to decode identity hex");

        let identity = Identity::from_cbor(identity_bytes.as_slice())
            .expect("expected to deserialize an identity");

        let storage_flags = StorageFlags { epoch: 0 };

        drive
            .insert_identity(
                &identity.id,
                Element::Item(identity_bytes, storage_flags.to_element_flags()),
                true,
                Some(&db_transaction),
            )
            .expect("expected to insert identity");

        drive
            .grove
            .commit_transaction(db_transaction)
            .expect("expected to be able to commit a transaction");
    }
}
