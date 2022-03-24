mod data_contract_already_exists;
mod invalid_data_contract;

pub use data_contract_already_exists::*;
pub use invalid_data_contract::*;

use dpp::data_contract::errors::DataContractError;
use wasm_bindgen::prelude::*;

use crate::mocks;

pub fn from_data_contract_to_js_error(e: DataContractError) -> JsValue {
    match e {
        DataContractError::InvalidDataContractError {
            errors,
            raw_data_contract,
        } => {
            let js_errors = errors
                .into_iter()
                .map(mocks::from_consensus_to_js_error)
                .collect();

            InvalidDataContractError::new(js_errors, raw_data_contract.into()).into()
        }
        _ => unimplemented!(),
    }
}
