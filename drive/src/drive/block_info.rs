use crate::fee_pools::epochs::Epoch;

#[derive(Clone, Default)]
pub struct BlockInfo {
    pub time: f64,
    pub height: u64,
    pub epoch: Epoch,
}

impl BlockInfo {
    pub fn default_with_time(time: f64) -> BlockInfo {
        BlockInfo {
            time,
            ..Default::default()
        }
    }

    pub fn default_with_epoch(epoch: Epoch) -> BlockInfo {
        BlockInfo {
            epoch,
            ..Default::default()
        }
    }
}
