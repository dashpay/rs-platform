use std::collections::BTreeMap;

use ciborium::value::Value as CborValue;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

use crate::identity::identity_public_key;
use crate::identity::state_transition::asset_lock_proof::AssetLockProof;
use crate::prelude::Revision;
use crate::util::cbor_value::{CborBTreeMapHelper, CborCanonicalMap};
use crate::util::deserializer;
use crate::util::json_value::{JsonValueExt, ReplaceWith};
use crate::{errors::ProtocolError, identifier::Identifier, metadata::Metadata, util::hash};

use super::{IdentityPublicKey, KeyID};

pub const IDENTIFIER_FIELDS_JSON: [&str; 1] = ["$id"];
pub const IDENTIFIER_FIELDS_RAW_OBJECT: [&str; 1] = ["id"];

/// Implement the Identity. Identity is a low-level construct that provides the foundation
/// for user-facing functionality on the platform
#[derive(Default, Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Identity {
    pub protocol_version: u32,
    pub id: Identifier,
    pub public_keys: Vec<IdentityPublicKey>,
    pub balance: u64,
    pub revision: Revision,
    #[serde(skip)]
    pub asset_lock_proof: Option<AssetLockProof>,
    #[serde(skip)]
    pub metadata: Option<Metadata>,
}

impl Identity {
    /// Get Identity protocol version
    pub fn get_protocol_version(&self) -> u32 {
        self.protocol_version
    }

    /// Returns Identity id
    pub fn get_id(&self) -> &Identifier {
        &self.id
    }

    /// Set Identity public key
    pub fn set_public_keys(&mut self, pub_key: Vec<IdentityPublicKey>) {
        self.public_keys = pub_key;
    }

    /// Get Identity public keys revision
    pub fn get_public_keys(&self) -> &[IdentityPublicKey] {
        &self.public_keys
    }

    /// Get Identity public keys revision
    pub fn get_public_keys_mut(&mut self) -> &mut [IdentityPublicKey] {
        &mut self.public_keys
    }

    // Returns a public key for a given id
    pub fn get_public_key_by_id(&self, key_id: KeyID) -> Option<&IdentityPublicKey> {
        self.public_keys.iter().find(|i| i.id == key_id)
    }

    // Returns a public key for a given id
    pub fn get_public_key_by_id_mut(&mut self, key_id: KeyID) -> Option<&mut IdentityPublicKey> {
        self.public_keys.iter_mut().find(|i| i.id == key_id)
    }

    /// Add identity public keys
    pub fn add_public_keys(&mut self, keys: impl IntoIterator<Item = IdentityPublicKey>) {
        self.public_keys.extend(keys);
    }

    /// Returns balance
    pub fn get_balance(&self) -> u64 {
        self.balance
    }

    /// Set Identity balance
    pub fn set_balance(&mut self, balance: u64) {
        self.balance = balance;
    }

    /// Increase Identity balance
    pub fn increase_balance(&mut self, amount: u64) -> u64 {
        self.balance += amount;
        self.balance
    }

    /// Reduce the Identity balance
    pub fn reduce_balance(&mut self, amount: u64) -> u64 {
        self.balance -= amount;
        self.balance
    }

    /// Set Identity asset lock
    pub fn set_asset_lock_proof(&mut self, lock: AssetLockProof) {
        self.asset_lock_proof = Some(lock);
    }

    /// Get Identity asset lock
    pub fn get_asset_lock_proof(&self) -> Option<&AssetLockProof> {
        self.asset_lock_proof.as_ref()
    }

    /// Set Identity revision
    pub fn set_revision(&mut self, revision: Revision) {
        self.revision = revision;
    }

    /// Get Identity revision
    pub fn get_revision(&self) -> Revision {
        self.revision
    }

    /// Get metadata
    pub fn get_metadata(&self) -> Option<&Metadata> {
        self.metadata.as_ref()
    }

    /// Set metadata
    pub fn set_metadata(&mut self, m: Metadata) {
        self.metadata = Some(m);
    }

    /// Get the biggest public KeyID
    pub fn get_public_key_max_id(&self) -> KeyID {
        self.public_keys
            .iter()
            .map(|pk| pk.id)
            .max()
            .unwrap_or_default()
    }

    pub fn to_buffer(&self) -> Result<Vec<u8>, ProtocolError> {
        self.to_cbor()
    }

    pub fn to_cbor(&self) -> Result<Vec<u8>, ProtocolError> {
        // Prepend protocol version to the result
        let mut buf = self.get_protocol_version().to_le_bytes().to_vec();

        let mut identity_map = CborCanonicalMap::new();

        identity_map.insert("id", self.get_id().to_buffer().to_vec());
        identity_map.insert("balance", self.get_balance());
        identity_map.insert("revision", self.get_revision());
        identity_map.insert(
            "publicKeys",
            self.get_public_keys()
                .iter()
                .map(|pk| pk.into())
                .collect::<Vec<CborValue>>(),
        );

        let mut identity_cbor = identity_map
            .to_bytes()
            .map_err(|e| ProtocolError::EncodingError(e.to_string()))?;
        buf.append(&mut identity_cbor);

        Ok(buf)
    }

    pub fn from_buffer(b: impl AsRef<[u8]>) -> Result<Self, ProtocolError> {
        Self::from_cbor(b.as_ref())
    }

    pub fn from_cbor(identity_cbor: &[u8]) -> Result<Self, ProtocolError> {
        let (protocol_version_bytes, identity_cbor_bytes) = identity_cbor.split_at(4);

        let protocol_version = deserializer::get_protocol_version(protocol_version_bytes)?;

        // Deserialize the contract
        let identity_map: BTreeMap<String, CborValue> =
            ciborium::de::from_reader(identity_cbor_bytes).map_err(|e| {
                ProtocolError::DecodingError(format!("Unable to decode identity CBOR: {}", e))
            })?;

        let identity_id = identity_map.get_identifier("id")?;
        let revision = identity_map.get_u64("revision")?;
        let balance: u64 = identity_map.get_u64("balance")?;

        let keys_cbor_value = identity_map.get("publicKeys").ok_or_else(|| {
            ProtocolError::DecodingError(String::from(
                "Unable to decode identity CBOR: missing property publicKeys",
            ))
        })?;
        let keys_cbor = keys_cbor_value.as_array().ok_or_else(|| {
            ProtocolError::DecodingError(String::from(
                "Unable to decode identity CBOR: invalid public keys",
            ))
        })?;

        let public_keys = keys_cbor
            .iter()
            .map(IdentityPublicKey::from_cbor_value)
            .collect::<Result<Vec<IdentityPublicKey>, ProtocolError>>()?;

        Ok(Self {
            protocol_version,
            id: identity_id.into(),
            public_keys,
            balance,
            revision,
            asset_lock_proof: None,
            metadata: None,
        })
    }

    pub fn from_json(mut json_object: JsonValue) -> Result<Identity, ProtocolError> {
        if let Some(public_keys_value) = json_object.get_mut("publicKeys") {
            if let Some(public_keys_array) = public_keys_value.as_array_mut() {
                for public_key in public_keys_array.iter_mut() {
                    public_key.replace_binary_paths(
                        identity_public_key::BINARY_DATA_FIELDS,
                        ReplaceWith::Bytes,
                    )?;
                }
            }
        }

        let identity: Identity = serde_json::from_value(json_object)?;

        Ok(identity)
    }

    pub fn from_raw_object(mut raw_object: JsonValue) -> Result<Identity, ProtocolError> {
        raw_object.replace_identifier_paths(IDENTIFIER_FIELDS_RAW_OBJECT, ReplaceWith::Base58)?;

        let identity: Identity = serde_json::from_value(raw_object)?;

        Ok(identity)
    }

    pub fn from_json_object(raw_object: JsonValue) -> Result<Identity, ProtocolError> {
        let pks = raw_object.get("publicKeys").unwrap().as_array().unwrap();

        for pk in pks {
            let _pkd: IdentityPublicKey = serde_json::from_value(pk.clone())
                .map_err(|_e| ProtocolError::Generic(format!("Can't parse public key: {}", pk)))?;
        }

        let identity: Identity = serde_json::from_value(raw_object)?;

        Ok(identity)
    }

    pub fn hash(&self) -> Result<Vec<u8>, ProtocolError> {
        Ok(hash::hash(&self.to_buffer()?))
    }
}
