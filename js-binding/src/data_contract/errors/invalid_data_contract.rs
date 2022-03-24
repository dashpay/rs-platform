use dpp::data_contract::DataContract;
use dpp::errors::AbstractConsensusErrorMock;
use dpp::mocks;
use thiserror::Error;
use wasm_bindgen::prelude::*;

use crate::errors::*;
use crate::DataContractWasm;

#[wasm_bindgen]
#[derive(Error, Debug)]
#[error("Invalid Data Contract")]
pub struct InvalidDataContractError {
    errors: Vec<AbstractConsensusError>,
    raw_data_contract: DataContractWasm,
}

impl InvalidDataContractError {
    pub fn new(errors: Vec<AbstractConsensusErrorMock>, raw_data_contract: DataContract) -> Self {
        InvalidDataContractError {
            errors: errors
                .into_iter()
                .map(AbstractConsensusError::from)
                .collect(),
            raw_data_contract: raw_data_contract.into(),
        }
    }
}

#[wasm_bindgen]
impl InvalidDataContractError {
    #[wasm_bindgen]
    pub fn get_errors(&self) -> Vec<JsValue> {
        self.errors.clone().into_iter().map(JsValue::from).collect()
    }

    #[wasm_bindgen]
    pub fn get_data_contract(&self) -> DataContractWasm {
        self.raw_data_contract.clone()
    }
}
