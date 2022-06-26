#[derive(Debug, thiserror::Error)]
pub enum IdentityError {
    #[error("identity not found error: {0}")]
    IdentityNotFound(&'static str),

    #[error("missing required key: {0}")]
    MissingRequiredKey(&'static str),

    #[error("identity key missing field: {0}")]
    IdentityKeyMissingField(&'static str),

    #[error("field requirement unmet: {0}")]
    FieldRequirementUnmet(&'static str),

    #[error("invalid identity structure: {0}")]
    InvalidIdentityStructure(&'static str),

    #[error("identity already exists error: {0}")]
    IdentityAlreadyExists(&'static str),

    #[error("balance overflow: {0}")]
    BalanceOverflow(&'static str),
}
