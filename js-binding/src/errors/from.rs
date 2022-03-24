use crate::data_contract::errors::*;
use crate::document::errors::from_document_to_js_error;

use dpp::data_contract::errors::DataContractError;

use dpp::errors::ProtocolError;
use wasm_bindgen::JsValue;

pub fn from_dpp_err(pe: ProtocolError) -> JsValue {
    match pe {
        ProtocolError::DataContractError(dce) => match dce {
            DataContractError::InvalidDataContractError {
                errors,
                raw_data_contract,
            } => InvalidDataContractError::new(errors, raw_data_contract).into(),

            _ => unimplemented!(),
        },

        ProtocolError::DataContractError(e) => from_document_to_js_error(e),

        _ => unimplemented!(),
    }
}
