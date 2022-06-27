use crate::drive::RootTree;
use crate::error::drive::DriveError;
use crate::error::Error;

pub mod insert;
pub mod update;

pub(crate) fn identity_path(identity_id: &[u8]) -> [&[u8]; 2] {
    [Into::<&[u8; 1]>::into(RootTree::Identities), identity_id]
}

pub(crate) fn identity_path_vec(identity_id: &[u8]) -> Vec<Vec<u8>> {
    vec![
        Into::<&[u8; 1]>::into(RootTree::Identities).to_vec(),
        identity_id.to_vec(),
    ]
}

pub(crate) fn identity_key_tree_path(identity_id: &[u8]) -> [&[u8]; 3] {
    [
        Into::<&[u8; 1]>::into(RootTree::Identities),
        identity_id,
        Into::<&[u8; 1]>::into(IdentityRootStructure::IdentityTreeKeys),
    ]
}

pub(crate) fn identity_query_keys_tree_path(identity_id: &[u8]) -> [&[u8]; 4] {
    [
        Into::<&[u8; 1]>::into(RootTree::Identities),
        identity_id,
        Into::<&[u8; 1]>::into(IdentityRootStructure::IdentityTreeKeys),
        &[],
    ]
}

pub(crate) fn identity_query_keys_purpose_tree_path<'a>(
    identity_id: &'a [u8],
    purpose: &'a [u8],
) -> [&'a [u8]; 5] {
    [
        Into::<&[u8; 1]>::into(RootTree::Identities),
        identity_id,
        Into::<&[u8; 1]>::into(IdentityRootStructure::IdentityTreeKeys),
        &[],
        purpose,
    ]
}

pub(crate) fn identity_query_keys_full_tree_path<'a>(
    identity_id: &'a [u8],
    purpose: &'a [u8],
    security_level: &'a [u8],
) -> [&'a [u8]; 6] {
    [
        Into::<&[u8; 1]>::into(RootTree::Identities),
        identity_id,
        Into::<&[u8; 1]>::into(IdentityRootStructure::IdentityTreeKeys),
        &[],
        purpose,
        security_level,
    ]
}

#[repr(u8)]
pub enum IdentityRootStructure {
    // Input data errors
    IdentityTreeRevision = 0,
    IdentityTreeBalance = 1, // the balance being at 1 means it will be at the top of the tree
    IdentityTreeKeys = 2,
}

impl From<IdentityRootStructure> for u8 {
    fn from(root_tree: IdentityRootStructure) -> Self {
        root_tree as u8
    }
}

impl From<IdentityRootStructure> for [u8; 1] {
    fn from(root_tree: IdentityRootStructure) -> Self {
        [root_tree as u8]
    }
}

impl From<IdentityRootStructure> for &'static [u8; 1] {
    fn from(identity_tree: IdentityRootStructure) -> Self {
        match identity_tree {
            IdentityRootStructure::IdentityTreeRevision => &[0],
            IdentityRootStructure::IdentityTreeBalance => &[1],
            IdentityRootStructure::IdentityTreeKeys => &[2],
        }
    }
}

pub fn balance_from_bytes(identity_balance_bytes: &[u8]) -> Result<u64, Error> {
    let balance_bytes: [u8; 8] = identity_balance_bytes.try_into().map_err(|_| {
        Error::Drive(DriveError::CorruptedElementType(
            "identity balance was not represented in 8 bytes",
        ))
    })?;
    Ok(u64::from_be_bytes(balance_bytes))
}
