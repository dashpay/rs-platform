use std::collections::{BTreeMap, HashMap};
use serde::{Deserialize, Serialize};
use grovedb::Error;
use crate::drive::Drive;
use ciborium::value::{Value as CborValue};
use crate::common::bytes_for_system_value_from_hash_map;

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct Identity {
    pub id: [u8; 32],
    pub revision: u64,
    pub balance: u64,
    pub keys: BTreeMap<u16, Key>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct Key {
    pub key_type: u8,
    pub public_bytes: Vec<u8>,
}

impl Identity {
    pub fn from_cbor(identity_cbor: &[u8]) -> Result<Self, Error> {
        let (version, read_identity_cbor) = identity_cbor.split_at(4);
        if !Drive::check_protocol_version_bytes(version) {
            return Err(Error::CorruptedData(String::from(
                "invalid protocol version",
            )));
        }
        // Deserialize the contract
        let identity: HashMap<String, CborValue> = ciborium::de::from_reader(read_identity_cbor)
            .map_err(|e| {
                Error::CorruptedData(String::from("unable to decode identity"))
            })?;

        // Get the contract id
        let identity_id: [u8; 32] = bytes_for_system_value_from_hash_map(&identity, "id")
            .ok_or_else(|| Error::CorruptedData(String::from("unable to get identity id")))?
            .try_into()
            .map_err(|_| Error::CorruptedData(String::from("id must be 32 bytes")))?;

        let revision : u64 = identity.get("revision")
            .ok_or_else(|| Error::CorruptedData(String::from("unable to get revision")))?
            .as_integer()
            .ok_or_else(|| Error::CorruptedData(String::from("revision must be an integer")))?
            .try_into()
            .map_err(|_| Error::CorruptedData(String::from("revision must be in the range of a unsigned 64 bit integer")))?;

        let balance : u64 = identity.get("balance")
            .ok_or_else(|| Error::CorruptedData(String::from("unable to get revision")))?
            .as_integer()
            .ok_or_else(|| Error::CorruptedData(String::from("revision must be an integer")))?
            .try_into()
            .map_err(|_| Error::CorruptedData(String::from("revision must be in the range of a unsigned 64 bit integer")))?;

        let keys_cbor_value = identity
            .get("publicKeys")
            .ok_or_else(|| Error::CorruptedData(String::from("unable to get keys")))?;
        let keys_cbor_value_raw = keys_cbor_value
            .as_array()
            .ok_or_else(|| Error::CorruptedData(String::from("unable to get keys as map")))?;

        let mut keys: BTreeMap<u16, Key> = BTreeMap::new();

        // Build the document type hashmap
        for key in keys_cbor_value_raw {
            if !key.is_map() {
                return Err(Error::CorruptedData(String::from(
                    "key value is not a map as expected",
                )));
            }
        }

        Ok(Identity {
            id: identity_id,
            revision,
            balance,
            keys,
        })
    }
}