#[derive(Debug, thiserror::Error)]
pub enum StorageFlagsError {
    #[error("deserialize unknown storage flags type error: {0}")]
    DeserializeUnknownStorageFlagsType(&'static str),
    #[error("storage flags wrong size error: {0}")]
    StorageFlagsWrongSize(&'static str),
    #[error("removing at epoch with no associated storage error: {0}")]
    RemovingAtEpochWithNoAssociatedStorage(&'static str),
    #[error("storage flags overflow error: {0}")]
    StorageFlagsOverflow(&'static str),
    #[error("merging storage flags from different owners error: {0}")]
    MergingStorageFlagsFromDifferentOwners(&'static str),
    #[error("merging storage flags with different base epoch: {0}")]
    MergingStorageFlagsWithDifferentBaseEpoch(&'static str),
}
