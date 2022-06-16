#[derive(Debug, thiserror::Error)]
pub enum FeeError {
    #[error("overflow error: {0}")]
    Overflow(&'static str),

    #[error("corrupted storage fee not an item error: {0}")]
    CorruptedStorageFeeNotItem(&'static str),
    #[error("corrupted processing fee not an item error: {0}")]
    CorruptedProcessingFeeNotItem(&'static str),
    #[error("corrupted first proposed block height not an item error: {0}")]
    CorruptedFirstProposedBlockHeightNotItem(&'static str),
    #[error("corrupted proposer block count not an item error: {0}")]
    CorruptedProposerBlockCountNotItem(&'static str),
    #[error("corrupted genesis time not an item error: {0}")]
    CorruptedGenesisTimeNotItem(&'static str),
    #[error("corrupted storage fee pool not an item error: {0}")]
    CorruptedStorageFeePoolNotItem(&'static str),
}
