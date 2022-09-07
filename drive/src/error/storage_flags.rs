#[derive(Debug, thiserror::Error)]
pub enum StorageFlagsError {
    #[error("deserialize unknown storage flags type error: {0}")]
    DeserializeUnknownStorageFlagsType(&'static str),
    #[error("storage flags wrong size error: {0}")]
    StorageFlagsWrongSize(&'static str),
}
