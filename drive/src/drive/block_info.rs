use crate::fee_pools::epochs::Epoch;

/// Block information
#[derive(Clone, Default)]
pub struct BlockInfo {
    /// Block time
    pub time: f64,

    /// Block height
    pub height: u64,

    /// Current fee epoch
    pub epoch: Epoch,
}

impl BlockInfo {
    /// Create block info for genesis block
    pub fn genesis() -> BlockInfo {
        BlockInfo::default()
    }

    /// Create default block with specified time
    pub fn default_with_time(time: f64) -> BlockInfo {
        BlockInfo {
            time,
            ..Default::default()
        }
    }

    /// Create default block with specified fee epoch
    pub fn default_with_epoch(epoch: Epoch) -> BlockInfo {
        BlockInfo {
            epoch,
            ..Default::default()
        }
    }
}
