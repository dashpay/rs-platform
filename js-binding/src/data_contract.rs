use dpp::document::DataContract;
pub use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

#[wasm_bindgen(js_name=DataContract)]
pub struct DataContractWasm(DataContract);

impl std::convert::From<DataContract> for DataContractWasm {
    fn from(v: DataContract) -> Self {
        DataContractWasm(v)
    }
}
impl std::convert::Into<DataContract> for DataContractWasm {
    fn into(self: Self) -> DataContract {
        self.0
    }
}
