use crate::drive::abci::messages::BlockEndRequest;

pub struct BlockInfo {
    pub block_height: u64,
    pub block_time: i64,
    pub previous_block_time: Option<i64>,
    pub proposer_pro_tx_hash: [u8; 32],
    pub fee_multiplier: u64,
}

impl BlockInfo {
    pub fn from_block_end_request(block_end_request: &BlockEndRequest) -> BlockInfo {
        BlockInfo {
            block_height: block_end_request.block_height,
            block_time: block_end_request.block_time,
            previous_block_time: block_end_request.previous_block_time,
            proposer_pro_tx_hash: block_end_request.proposer_pro_tx_hash,
            fee_multiplier: block_end_request.fee_multiplier,
        }
    }
}
