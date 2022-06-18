pub const DEFAULT_GROVE_BATCHING_ENABLED: bool = true;

pub struct DriveConfig {
    pub batching_enabled: bool,
}

impl DriveConfig {
    pub fn default() -> Self {
        DriveConfig {
            batching_enabled: DEFAULT_GROVE_BATCHING_ENABLED,
        }
    }
}
