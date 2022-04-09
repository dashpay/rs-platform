use crate::data_contract::errors::*;

use dpp::data_contract::errors::DataContractError;

use dpp::errors::ProtocolError;
use wasm_bindgen::JsValue;

pub fn from(pe: ProtocolError) -> JsValue {
    match pe {
        ProtocolError::DataContractError(dce) => match dce {
            DataContractError::InvalidDataContractError {
                errors,
                raw_data_contract,
            } => InvalidDataContractError::new(errors, raw_data_contract).into(),
            _ => unimplemented!(),
        },
        _ => unimplemented!(),
    }
}
