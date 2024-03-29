use crate::identity::KeyType;

pub const BASE_ST_PROCESSING_FEE: i64 = 10000; // 84000
pub const FEE_MULTIPLIER: i64 = 2;
pub const DEFAULT_USER_TIP: i64 = 0;
pub const STORAGE_CREDIT_PER_BYTE: i64 = 5000;
pub const PROCESSING_CREDIT_PER_BYTE: i64 = 12;
pub const DELETE_BASE_PROCESSING_COST: i64 = 2000; // 20000
pub const READ_BASE_PROCESSING_COST: i64 = 8400; // 8400
pub const WRITE_BASE_PROCESSING_COST: i64 = 6000; // 60000

pub const fn signature_verify_cost(key_type: KeyType) -> i64 {
    match key_type {
        KeyType::ECDSA_SECP256K1 => 3000,
        KeyType::BLS12_381 => 6000,
        KeyType::ECDSA_HASH160 => 3000,
        KeyType::BIP13_SCRIPT_HASH => 6000,
    }
}
