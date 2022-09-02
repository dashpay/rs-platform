use anyhow::anyhow;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

use crate::{
    identity::{KeyID, SecurityLevel},
    prelude::{Identifier, IdentityPublicKey, Revision, TimestampMillis},
    state_transition::{
        state_transition_helpers, StateTransitionConvert, StateTransitionIdentitySigned,
        StateTransitionLike, StateTransitionType,
    },
    util::json_value::JsonValueExt,
    version::LATEST_VERSION,
    ProtocolError,
};

pub mod property_names {
    pub const PROTOCOL_VERSION: &str = "protocolVersion";
    pub const TYPE: &str = "type";
    pub const IDENTITY_ID: &str = "identityId";
    pub const REVISION: &str = "revision";
    pub const ADD_PUBLIC_KEYS: &str = "addPublicKeys";
    pub const DISABLE_PUBLIC_KEYS: &str = "disablePublicKeys";
    pub const PUBLIC_KEYS_DISABLED_AT: &str = "publicKeysDisabledAt";
    pub const SIGNATURE: &str = "signature";
    pub const SIGNATURE_PUBLIC_KEY_ID: &str = "signaturePublicKeyId";
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct IdentityUpdateTransition {
    pub protocol_version: u32,
    #[serde(rename = "type")]
    pub transition_type: StateTransitionType,

    /// Cryptographic signature of the State Transition
    pub signature: Vec<u8>,

    /// The ID of the public key used to sing the State Transition
    pub signature_public_key_id: KeyID,

    /// Unique identifier of the identity to be updated
    pub identity_id: Identifier,

    /// Identity Update revision number
    pub revision: Revision,

    /// Public Keys to add to the Identity
    // we want to skip serialization of transitions, as we does it manually in `to_object()`  and `to_json()`
    #[serde(skip, default)]
    pub add_public_keys: Vec<IdentityPublicKey>,

    /// Identity Public Keys ID's to disable for the Identity
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub disable_public_keys: Vec<KeyID>,

    /// Timestamp when keys were disabled
    #[serde(skip_serializing_if = "Option::is_none")]
    pub public_keys_disabled_at: Option<TimestampMillis>,
}

impl Default for IdentityUpdateTransition {
    fn default() -> Self {
        Self {
            protocol_version: LATEST_VERSION,
            transition_type: StateTransitionType::IdentityUpdate,
            signature: Default::default(),
            signature_public_key_id: Default::default(),
            identity_id: Default::default(),
            revision: Default::default(),
            add_public_keys: Default::default(),
            disable_public_keys: Default::default(),
            public_keys_disabled_at: Default::default(),
        }
    }
}

impl IdentityUpdateTransition {
    pub fn from_raw_object(
        mut raw_object: JsonValue,
    ) -> Result<IdentityUpdateTransition, ProtocolError> {
        let protocol_version = raw_object
            .get_u64(property_names::PROTOCOL_VERSION)
            .unwrap_or(LATEST_VERSION as u64) as u32;
        let signature = raw_object
            .get_bytes(property_names::SIGNATURE)
            .unwrap_or_default();
        let signature_public_key_id = raw_object
            .get_u64(property_names::SIGNATURE_PUBLIC_KEY_ID)
            .unwrap_or_default();
        let identity_id =
            Identifier::from_bytes(&raw_object.get_bytes(property_names::IDENTITY_ID)?)?;
        let revision = raw_object.get_u64(property_names::REVISION)?;
        let add_public_keys =
            get_list_of_public_keys(&mut raw_object, property_names::ADD_PUBLIC_KEYS)?;
        let disable_public_keys =
            get_list_of_public_key_ids(&mut raw_object, property_names::DISABLE_PUBLIC_KEYS)?;
        let public_keys_disabled_at = raw_object
            .remove_into::<u64>(property_names::PUBLIC_KEYS_DISABLED_AT)
            .ok();

        Ok(IdentityUpdateTransition {
            protocol_version,
            signature,
            signature_public_key_id,
            identity_id,
            revision,
            add_public_keys,
            disable_public_keys,
            public_keys_disabled_at,
            transition_type: StateTransitionType::IdentityUpdate,
        })
    }

    /// Get State Transition Type
    pub fn get_type(&self) -> StateTransitionType {
        self.transition_type
    }

    pub fn set_identity_id(&mut self, id: Identifier) {
        self.identity_id = id;
    }

    pub fn get_identity_id(&self) -> &Identifier {
        &self.identity_id
    }

    pub fn set_revision(&mut self, revision: Revision) {
        self.revision = revision;
    }

    pub fn get_revision(&self) -> Revision {
        self.revision
    }

    pub fn set_public_keys_to_add(&mut self, add_public_keys: Vec<IdentityPublicKey>) {
        self.add_public_keys = add_public_keys;
    }

    pub fn get_public_keys_to_add(&self) -> &[IdentityPublicKey] {
        &self.add_public_keys
    }

    pub fn get_public_keys_to_add_mut(&mut self) -> &mut [IdentityPublicKey] {
        &mut self.add_public_keys
    }

    pub fn set_public_key_ids_to_disable(&mut self, disable_public_keys: Vec<KeyID>) {
        self.disable_public_keys = disable_public_keys;
    }

    pub fn get_public_key_ids_to_disable(&self) -> &[KeyID] {
        &self.disable_public_keys
    }

    pub fn set_public_keys_disabled_at(
        &mut self,
        public_keys_disabled_at: Option<TimestampMillis>,
    ) {
        self.public_keys_disabled_at = public_keys_disabled_at;
    }

    pub fn get_public_keys_disabled_at(&self) -> Option<TimestampMillis> {
        self.public_keys_disabled_at
    }
}

/// if the property isn't present the empty list is returned. If property is defined, the function
/// might return some serialization-related errors
fn get_list_of_public_keys(
    value: &mut JsonValue,
    property_name: &str,
) -> Result<Vec<IdentityPublicKey>, ProtocolError> {
    let mut identity_public_keys = vec![];
    if let Ok(maybe_list) = value.remove(property_names::ADD_PUBLIC_KEYS) {
        if let JsonValue::Array(list) = maybe_list {
            for maybe_public_key in list {
                identity_public_keys.push(IdentityPublicKey::from_raw_object(maybe_public_key)?);
            }
        } else {
            return Err(anyhow!("The property '{}' isn't a list", property_name).into());
        }
    } else {
        return Ok(vec![]);
    }

    Ok(identity_public_keys)
}

fn get_list_of_public_key_ids(
    value: &mut JsonValue,
    property_name: &str,
) -> Result<Vec<KeyID>, ProtocolError> {
    if let Ok(maybe_key_ids) = value.remove(property_name) {
        let key_ids: Vec<KeyID> = serde_json::from_value(maybe_key_ids)?;
        Ok(key_ids)
    } else {
        Ok(vec![])
    }
}

fn get_list_of_timestamps(
    value: &mut JsonValue,
    property_name: &str,
) -> Result<Vec<TimestampMillis>, ProtocolError> {
    if let Ok(maybe_timestamps) = value.remove(property_name) {
        let key_ids: Vec<KeyID> = serde_json::from_value(maybe_timestamps)?;
        Ok(key_ids)
    } else {
        Ok(vec![])
    }
}

impl StateTransitionConvert for IdentityUpdateTransition {
    fn binary_property_paths() -> Vec<&'static str> {
        vec![property_names::SIGNATURE]
    }

    fn identifiers_property_paths() -> Vec<&'static str> {
        vec![property_names::IDENTITY_ID]
    }

    fn signature_property_paths() -> Vec<&'static str> {
        vec![property_names::SIGNATURE]
    }

    fn to_object(&self, skip_signature: bool) -> Result<JsonValue, ProtocolError> {
        // The [state_transition_helpers::to_object] doesn't  convert the `add_public_keys` property.
        // The property must be serialized manually
        let mut add_public_keys: Vec<JsonValue> = vec![];
        for key in self.add_public_keys.iter() {
            add_public_keys.push(key.to_raw_json_object()?);
        }

        let mut raw_object: JsonValue = state_transition_helpers::to_object(
            self,
            Self::signature_property_paths(),
            Self::identifiers_property_paths(),
            skip_signature,
        )?;
        raw_object.insert(
            property_names::ADD_PUBLIC_KEYS.to_owned(),
            JsonValue::Array(add_public_keys),
        )?;

        Ok(raw_object)
    }

    fn to_json(&self) -> Result<JsonValue, ProtocolError> {
        // The [state_transition_helpers::to_json] doesn't  convert the `add_public_keys` property.
        // The property must be serialized manually
        let mut add_public_keys: Vec<JsonValue> = vec![];
        for key in self.add_public_keys.iter() {
            add_public_keys.push(key.to_json()?);
        }

        let mut json_object: JsonValue =
            state_transition_helpers::to_json(self, Self::binary_property_paths())?;
        json_object.insert(
            property_names::ADD_PUBLIC_KEYS.to_owned(),
            JsonValue::Array(add_public_keys),
        )?;

        Ok(json_object)
    }
}

impl StateTransitionLike for IdentityUpdateTransition {
    fn calculate_fee(&self) -> Result<u64, crate::ProtocolError> {
        todo!()
    }

    fn get_protocol_version(&self) -> u32 {
        self.protocol_version
    }

    fn get_signature(&self) -> &Vec<u8> {
        &self.signature
    }

    fn get_type(&self) -> StateTransitionType {
        self.transition_type
    }

    fn set_signature(&mut self, signature: Vec<u8>) {
        self.signature = signature;
    }
}

impl StateTransitionIdentitySigned for IdentityUpdateTransition {
    fn get_owner_id(&self) -> &Identifier {
        &self.identity_id
    }

    fn get_security_level_requirement(&self) -> SecurityLevel {
        SecurityLevel::MASTER
    }

    fn get_signature_public_key_id(&self) -> KeyID {
        self.signature_public_key_id
    }

    fn set_signature_public_key_id(&mut self, key_id: KeyID) {
        self.signature_public_key_id = key_id
    }
}

#[cfg(test)]
mod test {
    use chrono::Utc;
    use serde_json::json;

    use crate::{
        identity::{KeyType, Purpose},
        tests::{
            fixtures::{get_identity_update_transition_fixture, identity_fixture},
            utils::{generate_random_identifier, generate_random_identifier_struct},
        },
        util::string_encoding::Encoding,
    };

    use super::*;

    #[test]
    fn conversion_to_json_object() {
        let public_key = identity_fixture().get_public_keys()[0].to_owned();
        let transition = IdentityUpdateTransition {
            identity_id: generate_random_identifier_struct(),
            add_public_keys: vec![public_key],
            signature: generate_random_identifier().to_vec(),
            ..Default::default()
        };

        let result = transition
            .to_json()
            .expect("conversion to json shouldn't fail");

        assert!(matches!(
            result[property_names::IDENTITY_ID],
            JsonValue::String(_)
        ));
        assert!(matches!(
            result[property_names::SIGNATURE],
            JsonValue::String(_)
        ));
        assert!(matches!(
            result[property_names::ADD_PUBLIC_KEYS][0]["data"],
            JsonValue::String(_)
        ));
    }

    #[test]
    fn conversion_to_raw_object() {
        let public_key = identity_fixture().get_public_keys()[0].to_owned();
        let transition = IdentityUpdateTransition {
            identity_id: generate_random_identifier_struct(),
            add_public_keys: vec![public_key],
            signature: generate_random_identifier().to_vec(),

            ..Default::default()
        };

        let result = transition
            .to_object(false)
            .expect("conversion to raw object shouldn't fail");

        assert!(matches!(
            result[property_names::IDENTITY_ID],
            JsonValue::Array(_)
        ));
        assert!(matches!(
            result[property_names::SIGNATURE],
            JsonValue::Array(_)
        ));
        assert!(matches!(
            result[property_names::ADD_PUBLIC_KEYS][0]["data"],
            JsonValue::Array(_)
        ));
    }

    struct TestData {
        transition: IdentityUpdateTransition,
        raw_transition: JsonValue,
    }
    fn setup_test() -> TestData {
        let transition = get_identity_update_transition_fixture();
        let raw_transition = transition.to_object(false).unwrap();
        TestData {
            transition,
            raw_transition,
        }
    }

    #[test]
    fn get_type() {
        let TestData { transition, .. } = setup_test();
        assert_eq!(StateTransitionType::IdentityUpdate, transition.get_type());
    }

    #[test]
    fn set_identity_id() {
        let TestData { mut transition, .. } = setup_test();
        let id = generate_random_identifier_struct();
        transition.set_identity_id(id.clone());
        assert_eq!(&id, transition.get_identity_id());
    }

    #[test]
    fn get_revision() {
        let TestData { transition, .. } = setup_test();
        assert_eq!(0, transition.get_revision());
    }

    #[test]
    fn set_revision() {
        let TestData { mut transition, .. } = setup_test();
        transition.set_revision(42);
        assert_eq!(42, transition.get_revision());
    }

    #[test]
    fn get_owner_id() {
        let TestData { transition, .. } = setup_test();
        assert_eq!(&transition.identity_id, transition.get_owner_id());
    }

    #[test]
    fn get_public_keys_to_add() {
        let TestData { transition, .. } = setup_test();
        assert_eq!(
            &transition.add_public_keys,
            transition.get_public_keys_to_add()
        );
    }

    #[test]
    fn set_public_keys_to_add() {
        let TestData { mut transition, .. } = setup_test();
        let id_public_key = IdentityPublicKey {
            id : 0,
            key_type: KeyType::BLS12_381,
            purpose: Purpose::AUTHENTICATION,
            security_level : SecurityLevel::CRITICAL,
            read_only: true,
            data: hex::decode("01fac99ca2c8f39c286717c213e190aba4b7af76db320ec43f479b7d9a2012313a0ae59ca576edf801444bc694686694").unwrap(),
            disabled_at : None,
        };
        transition.set_public_keys_to_add(vec![id_public_key.clone()]);

        assert_eq!(vec![id_public_key], transition.get_public_keys_to_add());
    }

    #[test]
    fn get_disable_public_keys() {
        let TestData { transition, .. } = setup_test();
        assert_eq!(
            transition.disable_public_keys,
            transition.get_public_key_ids_to_disable()
        );
    }

    #[test]
    fn set_disable_public_keys() {
        let TestData { mut transition, .. } = setup_test();
        let id_to_disable = vec![1, 2];
        transition.set_public_key_ids_to_disable(id_to_disable.clone());

        assert_eq!(&id_to_disable, transition.get_public_key_ids_to_disable());
    }

    #[test]
    fn get_public_key_disabled_at() {
        let TestData { transition, .. } = setup_test();
        assert_eq!(
            transition.public_keys_disabled_at,
            transition.get_public_keys_disabled_at()
        );
    }

    #[test]
    fn set_public_key_disabled_at() {
        let TestData { mut transition, .. } = setup_test();
        let now = Utc::now().timestamp_millis() as u64;
        transition.set_public_keys_disabled_at(Some(now));

        assert_eq!(Some(now), transition.get_public_keys_disabled_at());
    }

    #[test]
    fn to_object() {
        let TestData { transition, .. } = setup_test();
        let result = transition
            .to_object(false)
            .expect("conversion to object shouldn't fail");

        let expected_raw_state_transition = json!({
            "protocolVersion" : 1,
            "type" : 5,
            "signature" : [],
            "signaturePublicKeyId": 0,
            "identityId" : transition.identity_id.to_buffer(),
            "revision": 0,
            "disablePublicKeys" : [0],
            "publicKeysDisabledAt" : 1234567,
            "addPublicKeys" : [
                {

                    "id" : 3,
                    "purpose" : 0,
                    "type": 0,
                    "securityLevel" : 0,
                    "data" :base64::decode("AkVuTKyF3YgKLAQlLEtaUL2HTditwGILfWUVqjzYnIgH").unwrap(),
                    "readOnly" : false
                }
            ]
        });

        assert_eq!(expected_raw_state_transition, result);
    }

    #[test]
    fn to_object_with_signature_skipped() {
        let TestData { transition, .. } = setup_test();
        let result = transition
            .to_object(true)
            .expect("conversion to object shouldn't fail");

        let expected_raw_state_transition = json!({
            "protocolVersion" : 1,
            "type" : 5,
            "signaturePublicKeyId": 0,
            "identityId" : transition.identity_id.to_buffer(),
            "revision": 0,
            "disablePublicKeys" : [0],
            "publicKeysDisabledAt" : 1234567,
            "addPublicKeys" : [
                {

                    "id" : 3,
                    "purpose" : 0,
                    "type": 0,
                    "securityLevel" : 0,
                    "data" :base64::decode("AkVuTKyF3YgKLAQlLEtaUL2HTditwGILfWUVqjzYnIgH").unwrap(),
                    "readOnly" : false
                }
            ]
        });

        assert_eq!(expected_raw_state_transition, result);
    }

    #[test]
    fn to_json() {
        let TestData { transition, .. } = setup_test();
        let result = transition
            .to_json()
            .expect("conversion to json shouldn't fail");

        let expected_raw_state_transition = json!({
            "protocolVersion" : 1,
            "type" : 5,
            "signature" : "",
            "signaturePublicKeyId": 0,
            "identityId" : transition.identity_id.to_string(Encoding::Base58),
            "revision": 0,
            "disablePublicKeys" : [0],
            "publicKeysDisabledAt" : 1234567,
            "addPublicKeys" : [
                {

                    "id" : 3,
                    "purpose" : 0,
                    "type": 0,
                    "securityLevel" : 0,
                    "data" : "AkVuTKyF3YgKLAQlLEtaUL2HTditwGILfWUVqjzYnIgH",
                    "readOnly" : false
                }
            ]
        });

        assert_eq!(expected_raw_state_transition, result);
    }
}
