// MIT LICENSE
//
// Copyright (c) 2021 Dash Core Group
//
// Permission is hereby granted, free of charge, to any
// person obtaining a copy of this software and associated
// documentation files (the "Software"), to deal in the
// Software without restriction, including without
// limitation the rights to use, copy, modify, merge,
// publish, distribute, sublicense, and/or sell copies of
// the Software, and to permit persons to whom the Software
// is furnished to do so, subject to the following
// conditions:
//
// The above copyright notice and this permission notice
// shall be included in all copies or substantial portions
// of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF
// ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED
// TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A
// PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT
// SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY
// CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION
// OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR
// IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
// DEALINGS IN THE SOFTWARE.
//

//! Tenderdash ABCI Messages.
//!
//! This module defines the structs used for handling ABCI messages
//! as well as defining and implementing the trait for serializing/deserializing them.
//!

use crate::error::serialization::SerializationError;
use crate::error::Error;
use crate::execution::fee_pools::epoch::EpochInfo;
use crate::execution::fee_pools::process_block_fees::ProcessedBlockFeesResult;
use serde::{Deserialize, Serialize};

/// A struct for handling chain initialization requests
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitChainRequest {}

/// A struct for handling chain initialization responses
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitChainResponse {}

/// A struct for handling block begin requests
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockBeginRequest {
    pub block_height: u64,
    pub block_time_ms: u64,
    pub previous_block_time_ms: Option<u64>,
    pub proposer_pro_tx_hash: [u8; 32],
}

/// A struct for handling block begin responses
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockBeginResponse {}

/// A struct for handling block end requests
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockEndRequest {
    pub fees: FeesAggregate,
}

/// Total credit refund amount for the Epoch
pub type EpochRefund = (u16, u64);

/// A struct to aggregate processing and storage fees
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeesAggregate {
    pub processing_fees: u64,
    pub storage_fees: u64,
    // pub refunds_by_epoch: Vec<EpochRefund>,
}

/// A struct for handling block end responses
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockEndResponse {
    pub current_epoch_index: u16,
    pub is_epoch_change: bool,
    pub proposers_paid_count: Option<u16>,
    pub paid_epoch_index: Option<u16>,
}

impl BlockEndResponse {
    /// Retrieves Epoch info and fee info for the block to be implemented in the BlockEndResponse
    pub(crate) fn from_epoch_info_and_process_block_fees_result(
        epoch_info: &EpochInfo,
        process_block_fees_result: &ProcessedBlockFeesResult,
    ) -> Self {
        let (proposers_paid_count, paid_epoch_index) = process_block_fees_result
            .payouts
            .as_ref()
            .map_or((None, None), |proposer_payouts| {
                (
                    Some(proposer_payouts.proposers_paid_count),
                    Some(proposer_payouts.paid_epoch_index),
                )
            });

        Self {
            current_epoch_index: epoch_info.current_epoch_index,
            is_epoch_change: epoch_info.is_epoch_change,
            proposers_paid_count,
            paid_epoch_index,
        }
    }
}

impl<'a> Serializable<'a> for InitChainRequest {}
impl<'a> Serializable<'a> for InitChainResponse {}
impl<'a> Serializable<'a> for BlockBeginRequest {}
impl<'a> Serializable<'a> for BlockBeginResponse {}
impl<'a> Serializable<'a> for BlockEndRequest {}
impl<'a> Serializable<'a> for BlockEndResponse {}

/// A trait for serializing or deserializing ABCI messages
pub trait Serializable<'a>: Serialize + Deserialize<'a> {
    /// Serialize ABCI message
    fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        let mut bytes = vec![];

        ciborium::ser::into_writer(&self, &mut bytes).map_err(|_| {
            Error::Serialization(SerializationError::CorruptedSerialization(
                "can't serialize ABCI message",
            ))
        })?;

        Ok(bytes)
    }

    /// Deserialize ABCI message
    fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
        ciborium::de::from_reader(bytes).map_err(|_| {
            Error::Serialization(SerializationError::CorruptedDeserialization(
                "can't deserialize ABCI message",
            ))
        })
    }
}
