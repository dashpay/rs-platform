/// Identity errors
#[derive(Debug, thiserror::Error)]
pub enum IdentityError {
    #[error("identity not found error: {0}")]
    IdentityNotFound(&'static str),

    /// Missing required key error
    #[error("missing required key: {0}")]
    MissingRequiredKey(&'static str),

    /// Identity key missing field error
    #[error("identity key missing field: {0}")]
    IdentityKeyMissingField(&'static str),

    /// Field requirement unmet error
    #[error("field requirement unmet: {0}")]
    FieldRequirementUnmet(&'static str),

    /// Invalid identity structure error
    #[error("invalid identity structure: {0}")]
    InvalidIdentityStructure(&'static str),

    /// Identity serialization error
    #[error("identity serialization error: {0}")]
    IdentitySerialization(&'static str),

    #[error("identity already exists error: {0}")]
    IdentityAlreadyExists(&'static str),

    #[error("identity insufficient balance error: {0}")]
    IdentityInsufficientBalance(&'static str),

    #[error("balance overflow: {0}")]
    BalanceOverflow(&'static str),
}
