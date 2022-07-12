use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeesAggregate {
    pub processing_fees: u64,
    pub storage_fees: u64,
    pub fee_multiplier: u64,
}
