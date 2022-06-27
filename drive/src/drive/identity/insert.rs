use crate::drive::flags::StorageFlags;
use crate::drive::identity::{
    identity_key_location_vec, identity_key_tree_path, identity_path, identity_path_vec,
    identity_query_keys_full_tree_path, identity_query_keys_purpose_tree_path,
    identity_query_keys_tree_path, IdentityRootStructure,
};

use crate::contract::types::encode_u16;
use crate::drive::object_size_info::PathKeyElementInfo::PathFixedSizeKeyElement;
use crate::drive::{identity_tree_path, key_hashes_tree_path, Drive, RootTree};
use crate::error::identity::IdentityError;
use crate::error::Error;
use crate::fee::calculate_fee;
use crate::fee::op::DriveOperation::FunctionOperation;
use crate::fee::op::{DriveOperation, FunctionOp, HashFunction};
use crate::identity::key::IdentityKey;
use crate::identity::Identity;
use grovedb::Element::{Item, Reference};
use grovedb::{Element, ElementFlags, TransactionArg};
use sha2::{Digest, Sha256};

impl Drive {
    fn create_key_tree_with_keys_operations(
        &self,
        identity_id: [u8; 32],
        keys: Vec<IdentityKey>,
        element_flags: ElementFlags,
        apply: bool,
        transaction: TransactionArg,
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

        // We create the query trees structure
        self.create_new_identity_query_trees_operations(
            identity_id,
            element_flags.clone(),
            drive_operations,
        )?;

        for key in keys.into_iter() {
            self.insert_new_key_operations(
                identity_id.as_slice(),
                key,
                element_flags.clone(),
                false,
                apply,
                transaction,
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

    pub fn insert_new_key_operations(
        &self,
        identity_id: &[u8],
        identity_key: IdentityKey,
        element_flags: ElementFlags,
        verify: bool,
        apply: bool,
        transaction: TransactionArg,
        drive_operations: &mut Vec<DriveOperation>,
    ) -> Result<(), Error> {
        let serialized_identity_key = identity_key.serialize();
        let IdentityKey {
            id,
            key_type,
            purpose,
            security_level,
            readonly,
            public_key_bytes,
        } = identity_key;
        let key_hashes_tree = key_hashes_tree_path();
        let key_len = public_key_bytes.len();
        let key_hash = Sha256::digest(public_key_bytes.clone());
        drive_operations.push(FunctionOperation(FunctionOp {
            hash: HashFunction::Sha256,
            byte_count: key_len as u16,
        }));
        if verify {
            let exists = self.grove_has_raw(
                key_hashes_tree,
                key_hash.as_slice(),
                true, //if you want to verify you need to know the state
                transaction,
                drive_operations,
            )?;
            if exists {
                return Err(Error::Identity(IdentityError::IdentityAlreadyExists(
                    "trying to insert a key that already exists",
                )));
            }
        }

        let identity_path = identity_path_vec(identity_id);

        // Let's first insert the hash with a reference to the identity
        self.batch_insert(
            PathFixedSizeKeyElement((
                key_hashes_tree,
                key_hash.as_slice(),
                Reference(identity_path, element_flags.clone()),
            )),
            drive_operations,
        )?;

        // Now lets insert the public key
        let identity_key_tree = identity_key_tree_path(identity_id);

        let key_id_bytes = encode_u16(id)?;
        self.batch_insert(
            PathFixedSizeKeyElement((
                identity_key_tree,
                key_id_bytes.as_slice(),
                Item(serialized_identity_key, element_flags.clone()),
            )),
            drive_operations,
        )?;

        let purpose_vec = vec![purpose];
        let security_level_vec = vec![security_level];

        // Now lets add in references so we can query keys.
        // We assume the following, the identity already has a the basic Query Tree

        if purpose != 0 {
            // Not authentication
            if security_level != 3 {
                // Not Medium (Medium is already pre-inserted)

                let purpose_path =
                    identity_query_keys_purpose_tree_path(identity_id, purpose_vec.as_slice());

                let exists = self.grove_has_raw(
                    purpose_path,
                    &[security_level],
                    apply,
                    transaction,
                    drive_operations,
                )?;

                if exists == false {
                    // We need to insert the security level if it doesn't yet exist
                    self.batch_insert(
                        PathFixedSizeKeyElement((
                            purpose_path,
                            &[security_level],
                            Element::empty_tree_with_flags(element_flags.clone()),
                        )),
                        drive_operations,
                    )?;
                }
            }
        }

        // Now let's set the reference
        let reference_path = identity_query_keys_full_tree_path(
            identity_id,
            purpose_vec.as_slice(),
            security_level_vec.as_slice(),
        );

        let key_reference = identity_key_location_vec(identity_id, key_id_bytes.as_slice());
        self.batch_insert(
            PathFixedSizeKeyElement((
                reference_path,
                key_id_bytes.as_slice(),
                Reference(key_reference, element_flags),
            )),
            drive_operations,
        )
    }

    pub fn create_new_identity_query_trees_operations(
        &self,
        identity_id: [u8; 32],
        element_flags: ElementFlags,
        drive_operations: &mut Vec<DriveOperation>,
    ) -> Result<(), Error> {
        let identity_key_tree = identity_key_tree_path(identity_id.as_slice());

        // We need to insert the query tree
        self.batch_insert(
            PathFixedSizeKeyElement((
                identity_key_tree,
                &[],
                Element::empty_tree_with_flags(element_flags.clone()),
            )),
            drive_operations,
        )?;

        let identity_query_key_tree = identity_query_keys_tree_path(identity_id.as_slice());

        // There are 3 Purposes: Authentication, Encryption, Decryption
        for purpose in 0..3 {
            self.batch_insert(
                PathFixedSizeKeyElement((
                    identity_query_key_tree,
                    &[purpose],
                    Element::empty_tree_with_flags(element_flags.clone()),
                )),
                drive_operations,
            )?;
        }
        // There are 4 Security Levels: Master, Critical, High, Medium
        // For the Authentication Purpose we insert every tree
        let identity_key_authentication_tree =
            identity_query_keys_purpose_tree_path(identity_id.as_slice(), &[0]);
        for security_level in 0..4 {
            self.batch_insert(
                PathFixedSizeKeyElement((
                    identity_key_authentication_tree,
                    &[security_level],
                    Element::empty_tree_with_flags(element_flags.clone()),
                )),
                drive_operations,
            )?;
        }
        // For Encryption and Decryption we only insert the medium security level
        for purpose in 1..3 {
            let purpose_vec = vec![purpose];
            let identity_key_purpose_tree = identity_query_keys_purpose_tree_path(
                identity_id.as_slice(),
                purpose_vec.as_slice(),
            );

            self.batch_insert(
                PathFixedSizeKeyElement((
                    identity_key_purpose_tree,
                    &[3], //medium
                    Element::empty_tree_with_flags(element_flags.clone()),
                )),
                drive_operations,
            )?;
        }
        Ok(())
    }

    pub fn insert_new_identity(
        &self,
        identity: Identity,
        storage_flags: StorageFlags,
        verify: bool,
        apply: bool,
        transaction: TransactionArg,
    ) -> Result<(i64, u64), Error> {
        let mut batch_operations: Vec<DriveOperation> = vec![];
        self.create_identity_operations(
            identity,
            storage_flags,
            verify,
            apply,
            transaction,
            &mut batch_operations,
        )?;

        let mut drive_operations: Vec<DriveOperation> = vec![];

        self.apply_batch(apply, transaction, batch_operations, &mut drive_operations)?;

        calculate_fee(None, Some(drive_operations))
    }

    pub fn create_identity_operations(
        &self,
        identity: Identity,
        storage_flags: StorageFlags,
        verify: bool,
        apply: bool,
        transaction: TransactionArg,
        drive_operations: &mut Vec<DriveOperation>,
    ) -> Result<(), Error> {
        let identity_tree_path = identity_tree_path();

        let Identity {
            id,
            revision,
            balance,
            keys,
        } = identity;

        // If we are asking to verify we check to make sure the tree for this identity doesn't yet exist
        if verify {
            let exists = self.grove_has_raw(
                identity_tree_path,
                id.as_slice(),
                apply,
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
                id.as_slice(),
                Element::empty_tree_with_flags(storage_flags.to_element_flags()),
            )),
            drive_operations,
        )?;

        // We insert the balance
        self.set_identity_balance(
            id,
            balance,
            storage_flags.to_element_flags(),
            drive_operations,
        )?;

        // We insert the revision
        self.set_revision(
            id,
            revision,
            storage_flags.to_element_flags(),
            drive_operations,
        )?;

        // We insert the key tree and keys
        self.create_key_tree_with_keys_operations(
            id,
            keys.into_values().collect(),
            storage_flags.to_element_flags(),
            apply,
            transaction,
            drive_operations,
        )
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

        let identity = Identity::random_identity(5, Some(12345));
        let db_transaction = drive.grove.start_transaction();

        drive
            .create_root_tree(Some(&db_transaction))
            .expect("expected to create root tree successfully");

        drive
            .insert_new_identity(
                identity,
                StorageFlags { epoch: 0 },
                false,
                true,
                Some(&db_transaction),
            )
            .expect("expected to insert identity");

        drive
            .grove
            .commit_transaction(db_transaction)
            .expect("expected to be able to commit a transaction");
    }

    #[test]
    fn test_insert_identity_old() {
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
