use super::SystemIDs;

pub mod types {
    pub const WITHDRAWAL: &str = "withdrawal";
}

pub fn system_ids() -> SystemIDs {
    SystemIDs {
        contract_id: "4fJLR2GYTPFdomuTVvNy3VRrvWgvkKPzqehEBpNf2nk6".to_string(),
        owner_id: "CUjAw7eD64wmaznNrfC5sKdn4Lpr1wBvWKMjGLrmEs5h".to_string(),
    }
}
