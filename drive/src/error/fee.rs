#[derive(Debug, thiserror::Error)]
pub enum FeeError {
    #[error("overflow error: {0}")]
    Overflow(&'static str),

    #[error("corrupted storage fee not an item error: {0}")]
    CorruptedStorageFeeNotItem(&'static str),
    #[error("corrupted storage fee invalid item length error: {0}")]
    CorruptedStorageFeeInvalidItemLength(&'static str),
    #[error("corrupted processing fee not an item error: {0}")]
    CorruptedProcessingFeeNotItem(&'static str),
    #[error("corrupted processing fee invalid item length error: {0}")]
    CorruptedProcessingFeeInvalidItemLength(&'static str),
    #[error("corrupted first proposed block height not an item error: {0}")]
    CorruptedFirstProposedBlockHeightNotItem(&'static str),
    #[error("corrupted first proposed block height invalid item length error: {0}")]
    CorruptedFirstProposedBlockHeightItemLength(&'static str),
    #[error("corrupted proposer block count not an item error: {0}")]
    CorruptedProposerBlockCountNotItem(&'static str),
    #[error("corrupted proposer block count invalid item length error: {0}")]
    CorruptedProposerBlockCountItemLength(&'static str),
    #[error("corrupted genesis time not an item error: {0}")]
    CorruptedGenesisTimeNotItem(&'static str),
    #[error("corrupted genesis time invalid item length error: {0}")]
    CorruptedGenesisTimeInvalidItemLength(&'static str),
    #[error("corrupted storage fee pool not an item error: {0}")]
    CorruptedStorageFeePoolNotItem(&'static str),
    #[error("corrupted storage fee pool invalid item length error: {0}")]
    CorruptedStorageFeePoolInvalidItemLength(&'static str),
}
