use crate::contract::types::{encode_u16, encode_u64};
use crate::drive::flags::StorageFlags;
use crate::drive::identity::{
    balance_from_bytes, identity_key_tree_path, identity_path, IdentityRootStructure,
};
use crate::drive::object_size_info::KeyValueInfo::KeyRefRequest;
use crate::drive::object_size_info::PathKeyElementInfo::PathFixedSizeKeyElement;
use crate::drive::{identity_tree_path, Drive, RootTree};
use crate::error::drive::DriveError;
use crate::error::identity::IdentityError;
use crate::error::Error;
use crate::fee::calculate_fee;
use crate::fee::op::DriveOperation;
use crate::identity::key::IdentityKey;
use crate::identity::Identity;
use grovedb::Element::Item;
use grovedb::{Element, ElementFlags, TransactionArg};
use std::collections::BTreeMap;

impl Drive {
    fn create_key_tree_with_keys(
        &self,
        identity_id: [u8; 32],
        keys: BTreeMap<u16, IdentityKey>,
        element_flags: ElementFlags,
        drive_operations: &mut Vec<DriveOperation>,
    ) -> Result<(), Error> {
        let identity_path = identity_path(identity_id.as_slice());
        self.batch_insert(
            PathFixedSizeKeyElement((
                identity_path,
                Into::<&[u8; 1]>::into(IdentityRootStructure::IdentityTreeKeys),
                Element::empty_tree_with_flags(element_flags.clone()),
            )),
            drive_operations,
        )?;

        let identity_tree_path = identity_key_tree_path(identity_id.as_slice());

        for (key_id, key) in keys.into_iter() {
            let serialized_key = key.serialize();
            let encoded_key = encode_u16(key_id)?;
            self.batch_insert(
                PathFixedSizeKeyElement((
                    identity_tree_path,
                    encoded_key.as_slice(),
                    Element::empty_tree_with_flags(element_flags.clone()),
                )),
                drive_operations,
            )?;
        }
        Ok(())
    }

    pub(crate) fn set_identity_balance(
        &self,
        identity_id: [u8; 32],
        balance: u64,
        element_flags: ElementFlags,
        drive_operations: &mut Vec<DriveOperation>,
    ) -> Result<(), Error> {
        let identity_path = identity_path(identity_id.as_slice());
        let new_balance_bytes = balance.to_be_bytes().to_vec();

        self.batch_insert(
            PathFixedSizeKeyElement((
                identity_path,
                Into::<&[u8; 1]>::into(IdentityRootStructure::IdentityTreeBalance),
                Item(new_balance_bytes, element_flags),
            )),
            drive_operations,
        )
    }

    fn set_revision(
        &self,
        identity_id: [u8; 32],
        revision: u64,
        element_flags: ElementFlags,
        drive_operations: &mut Vec<DriveOperation>,
    ) -> Result<(), Error> {
        let identity_path = identity_path(identity_id.as_slice());
        let revision_bytes = revision.to_be_bytes().to_vec();
        self.batch_insert(
            PathFixedSizeKeyElement((
                identity_path,
                Into::<&[u8; 1]>::into(IdentityRootStructure::IdentityTreeRevision),
                Item(revision_bytes, element_flags),
            )),
            drive_operations,
        )
    }

    pub fn create_identity(
        &self,
        identity_id: [u8; 32],
        balance: u64,
        keys: Vec<IdentityKey>,
        storage_flags: StorageFlags,
        verify: bool,
        apply: bool,
        transaction: TransactionArg,
        drive_operations: &mut Vec<DriveOperation>,
    ) -> Result<(), Error> {
        let mut batch_operations: Vec<DriveOperation> = vec![];

        let identity_tree_path = identity_tree_path();

        // If we are asking to verify we check to make sure the tree for this identity doesn't yet exist
        if verify {
            let exists = self.grove_has_raw(
                identity_tree_path,
                identity_id.as_slice(),
                transaction,
                drive_operations,
            )?;
            if exists {
                return Err(Error::Identity(IdentityError::IdentityAlreadyExists(
                    "trying to insert an identity that already exists",
                )));
            }
        }

        // We insert the identity tree
        self.batch_insert(
            PathFixedSizeKeyElement((
                identity_tree_path,
                identity_id.as_slice(),
                Element::empty_tree_with_flags(storage_flags.to_element_flags()),
            )),
            drive_operations,
        )?;

        // We insert the balance
        self.set_identity_balance(
            identity_id,
            balance,
            storage_flags.to_element_flags(),
            &mut batch_operations,
        )?;

        // We insert the revision
        self.set_revision(
            identity_id,
            1,
            storage_flags.to_element_flags(),
            &mut batch_operations,
        )?;

        let keys = keys.into_iter().map(|key| (key.id, key)).collect();

        // We insert the key tree and keys
        self.create_key_tree_with_keys(
            identity_id,
            keys,
            storage_flags.to_element_flags(),
            &mut batch_operations,
        )?;

        self.apply_batch(apply, transaction, batch_operations, drive_operations)?;
        Ok(())
    }

    pub fn insert_identity(
        &self,
        identity_key: &[u8],
        identity_bytes: Element,
        apply: bool,
        transaction: TransactionArg,
    ) -> Result<(i64, u64), Error> {
        let mut batch_operations: Vec<DriveOperation> = vec![];

        self.batch_insert(
            PathFixedSizeKeyElement((
                [Into::<&[u8; 1]>::into(RootTree::Identities).as_slice()],
                identity_key,
                identity_bytes,
            )),
            &mut batch_operations,
        )?;

        let mut drive_operations: Vec<DriveOperation> = vec![];

        self.apply_batch(apply, transaction, batch_operations, &mut drive_operations)?;

        calculate_fee(None, Some(drive_operations))
    }

    pub fn insert_identity_cbor(
        &self,
        identity_id: Option<&[u8]>,
        identity_bytes: Vec<u8>,
        apply: bool,
        transaction: TransactionArg,
    ) -> Result<(i64, u64), Error> {
        let identity_id = match identity_id {
            None => {
                let identity = Identity::from_cbor(identity_bytes.as_slice())?;
                Vec::from(identity.id)
            }
            Some(identity_id) => Vec::from(identity_id),
        };

        let epoch = self.epoch_info.borrow().current_epoch;

        let storage_flags = StorageFlags { epoch };

        self.insert_identity(
            identity_id.as_slice(),
            Element::Item(identity_bytes, storage_flags.to_element_flags()),
            apply,
            transaction,
        )
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
