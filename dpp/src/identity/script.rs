use dashcore::Script as DashcoreScript;
use serde::{Deserialize, Serialize};

use crate::{
    util::string_encoding::{self, Encoding},
    ProtocolError,
};

#[derive(Clone, Debug)]
pub struct Script(DashcoreScript);

impl Script {
    pub fn is_p2sh(&self) -> bool {
        self.0.is_p2sh()
    }

    pub fn is_p2pkh(&self) -> bool {
        self.0.is_p2pkh()
    }

    pub fn to_string(&self, encoding: Encoding) -> String {
        string_encoding::encode(&self.0.to_bytes(), encoding)
    }

    pub fn from_string(encoded_value: &str, encoding: Encoding) -> Result<Self, ProtocolError> {
        let vec = string_encoding::decode(encoded_value, encoding)?;

        Ok(Self(DashcoreScript::from(vec)))
    }

    pub fn get_original_script(&self) -> DashcoreScript {
        self.0.clone()
    }
}

impl Default for Script {
    fn default() -> Self {
        Self(Default::default())
    }
}

impl Serialize for Script {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string(Encoding::Base64))
    }
}

impl<'de> Deserialize<'de> for Script {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let data: String = Deserialize::deserialize(deserializer)?;

        Self::from_string(&data, Encoding::Base64)
            .map_err(|e| serde::de::Error::custom(e.to_string()))
    }
}
