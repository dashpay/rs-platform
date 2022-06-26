use byteorder::{BigEndian, ReadBytesExt};
use ciborium::value::Value;
use integer_encoding::{VarInt, VarIntReader};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read};

use crate::common::{bytes_for_system_value_from_tree_map, read_varint_value};
use crate::drive::Drive;
use crate::error::drive::DriveError;
use crate::error::identity::IdentityError;
use crate::error::structure::StructureError;
use crate::error::Error;
use crate::identity::key::IdentityKey;

pub mod key;

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct Identity {
    pub id: [u8; 32],
    pub revision: u64,
    pub balance: u64,
    pub keys: BTreeMap<u16, IdentityKey>,
}

impl Identity {
    /// Serialize will serialize the whole identity, this should not be used for storage
    pub fn serialize(&self) -> Vec<u8> {
        let Identity {
            id,
            revision,
            balance,
            keys,
        } = self;
        let mut buffer: Vec<u8> = id.as_slice().to_vec();
        buffer.extend(revision.to_be_bytes().to_vec());
        buffer.extend(balance.to_be_bytes().to_vec());

        let mut r_vec = keys.len().encode_var_vec();
        buffer.append(&mut r_vec);

        keys.iter().for_each(|(key_id, key)| {
            let mut key_vec = key.serialize();
            let mut r_vec = key_vec.len().encode_var_vec();
            buffer.append(&mut r_vec);
            buffer.append(&mut key_vec);
        });
        buffer
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
        let mut buf = BufReader::new(bytes);
        if bytes.len() < 16 {
            return Err(Error::Drive(DriveError::CorruptedSerialization(
                "serialized value data is too small, must have revision and owner id",
            )));
        }

        let mut id = [0; 32];
        buf.read_exact(&mut id).map_err(|_| {
            Error::Drive(DriveError::CorruptedSerialization(
                "error reading from serialized identity",
            ))
        })?;

        let revision = buf.read_u64::<BigEndian>().map_err(|_| {
            Error::Drive(DriveError::CorruptedSerialization(
                "error reading from serialized identity",
            ))
        })?;
        let balance = buf.read_u64::<BigEndian>().map_err(|_| {
            Error::Drive(DriveError::CorruptedSerialization(
                "error reading from serialized identity",
            ))
        })?;

        let key_count: usize = buf.read_varint().map_err(|_| {
            Error::Drive(DriveError::CorruptedSerialization(
                "error reading from serialized identity",
            ))
        })?;

        let mut keys = BTreeMap::new();

        for _ in 0..key_count {
            let key_bytes = read_varint_value(&mut buf)?.ok_or(Error::Drive(
                DriveError::CorruptedSerialization("expected more keys than present"),
            ))?;
            let key = IdentityKey::from_bytes(key_bytes.as_slice())?;
            keys.insert(key.id, key);
        }

        Ok(Identity {
            id,
            revision,
            balance,
            keys,
        })
    }

    pub fn random_identity_with_rng(key_count: u16, rng: &mut StdRng) -> Self {
        let id = rng.gen::<[u8; 32]>();
        let revision = rng.gen::<u64>();
        let balance = rng.gen::<u64>();
        let keys = IdentityKey::random_keys_with_rng(key_count, 96, rng)
            .into_iter()
            .map(|key| (key.id, key))
            .collect();

        Identity {
            id,
            revision,
            balance,
            keys,
        }
    }

    pub fn random_identity(key_count: u16, seed: Option<u64>) -> Self {
        let mut rng = match seed {
            None => StdRng::from_entropy(),
            Some(seed_value) => StdRng::seed_from_u64(seed_value),
        };
        Self::random_identity_with_rng(key_count, &mut rng)
    }

    pub fn random_identities(count: u16, key_count: u16, seed: Option<u64>) -> Vec<Self> {
        let mut rng = match seed {
            None => StdRng::from_entropy(),
            Some(seed_value) => StdRng::seed_from_u64(seed_value),
        };
        let mut vec: Vec<Identity> = vec![];
        for _i in 0..count {
            vec.push(Self::random_identity_with_rng(key_count, &mut rng));
        }
        vec
    }

    pub fn from_cbor(identity_cbor: &[u8]) -> Result<Self, Error> {
        let (version, read_identity_cbor) = identity_cbor.split_at(4);
        if !Drive::check_protocol_version_bytes(version) {
            return Err(Error::Structure(StructureError::InvalidProtocolVersion(
                "invalid protocol version",
            )));
        }
        // Deserialize the contract
        let identity: BTreeMap<String, Value> = ciborium::de::from_reader(read_identity_cbor)
            .map_err(|_| {
                Error::Structure(StructureError::InvalidCBOR("unable to decode identity"))
            })?;

        // Get the contract id
        let identity_id: [u8; 32] = bytes_for_system_value_from_tree_map(&identity, "id")?
            .ok_or({
                Error::Identity(IdentityError::MissingRequiredKey(
                    "unable to get contract id",
                ))
            })?
            .try_into()
            .map_err(|_| {
                Error::Identity(IdentityError::FieldRequirementUnmet("id must be 32 bytes"))
            })?;

        let revision: u64 = identity
            .get("revision")
            .ok_or({
                Error::Identity(IdentityError::MissingRequiredKey("unable to get revision"))
            })?
            .as_integer()
            .ok_or({
                Error::Structure(StructureError::KeyWrongType("revision must be an integer"))
            })?
            .try_into()
            .map_err(|_| {
                Error::Structure(StructureError::KeyWrongBounds(
                    "revision must be in the range of a unsigned 64 bit integer",
                ))
            })?;

        let balance: u64 = identity
            .get("balance")
            .ok_or(Error::Identity(IdentityError::MissingRequiredKey(
                "unable to get balance",
            )))?
            .as_integer()
            .ok_or({
                Error::Structure(StructureError::KeyWrongType("balance must be an integer"))
            })?
            .try_into()
            .map_err(|_| {
                Error::Structure(StructureError::KeyWrongBounds(
                    "balance must be in the range of a unsigned 64 bit integer",
                ))
            })?;

        let keys_cbor_value = identity.get("publicKeys").ok_or(Error::Identity(
            IdentityError::MissingRequiredKey("unable to get keys"),
        ))?;
        let keys_cbor_value_raw = keys_cbor_value.as_array().ok_or({
            Error::Identity(IdentityError::InvalidIdentityStructure(
                "unable to get keys as map",
            ))
        })?;

        let mut keys: BTreeMap<u16, IdentityKey> = BTreeMap::new();

        // Build the document type hashmap
        for key in keys_cbor_value_raw {
            match key.as_map() {
                None => {
                    return Err(Error::Identity(IdentityError::InvalidIdentityStructure(
                        "key value is not a map as expected",
                    )));
                }
                Some(map) => {
                    let key = IdentityKey::from_cbor_value(map)?;
                    keys.insert(key.id, key);
                }
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

#[cfg(test)]
mod tests {
    use crate::identity::Identity;

    #[test]
    pub fn deserialize() {
        let identity_cbor = hex::decode("01000000a46269645820648d10ec3a16a37d2e62e8481820dbc2a853834625b065c036e3f998389e6a296762616c616e636500687265766973696f6e006a7075626c69634b65797382a6626964006464617461582102eaf222e32d46b97f56f890bb22c3d65e279b18bda203f30bd2d3eed769a3476264747970650067707572706f73650068726561644f6e6c79f46d73656375726974794c6576656c00a6626964016464617461582103c00af793d83155f95502b33a17154110946dcf69ca0dd188bee3b6d10c0d4f8b64747970650067707572706f73650168726561644f6e6c79f46d73656375726974794c6576656c03").unwrap();
        let identity = Identity::from_cbor(identity_cbor.as_slice())
            .expect("expected to deserialize an identity");
    }
}
