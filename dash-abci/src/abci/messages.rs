use serde::{Deserialize, Serialize};
use crate::error::Error;
use crate::error::serialization::SerializationError;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitChainRequest {}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitChainResponse {}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockBeginRequest {
    pub block_height: u64,
    pub block_time_ms: u64,
    pub previous_block_time_ms: Option<u64>,
    pub proposer_pro_tx_hash: [u8; 32],
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockBeginResponse {}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockEndRequest {
    pub fees: FeesAggregate,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeesAggregate {
    pub processing_fees: u64,
    pub storage_fees: u64,
    pub refunds_by_epoch: Vec<(u16, u64)>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockEndResponse {
    pub current_epoch_index: u16,
    pub is_epoch_change: bool,
    pub masternodes_paid_count: u16,
    pub paid_epoch_index: Option<u16>,
}

impl<'a> Serializable<'a> for InitChainRequest {}
impl<'a> Serializable<'a> for InitChainResponse {}
impl<'a> Serializable<'a> for BlockBeginRequest {}
impl<'a> Serializable<'a> for BlockBeginResponse {}
impl<'a> Serializable<'a> for BlockEndRequest {}
impl<'a> Serializable<'a> for BlockEndResponse {}

pub trait Serializable<'a>: Serialize + Deserialize<'a> {
    fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        let mut bytes = vec![];

        ciborium::ser::into_writer(&self, &mut bytes).map_err(|_| {
            Error::Serialization(SerializationError::CorruptedSerialization(
                "can't serialize ABCI message",
            ))
        })?;

        Ok(bytes)
    }

    fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
        ciborium::de::from_reader(bytes).map_err(|_| {
            Error::Serialization(SerializationError::CorruptedDeserialization(
                "can't deserialize ABCI message",
            ))
        })
    }
}
