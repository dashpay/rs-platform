use dpp::document::Document;
use std::convert::TryInto;
use wasm_bindgen::prelude::*;

use crate::identifier::IdentifierWrapper;
use crate::{DataContractWasm, MetadataWasm};

#[wasm_bindgen(js_name=Document)]
pub struct DocumentWasm(Document);

// TODO error handling

#[wasm_bindgen(js_class=Document)]
impl DocumentWasm {
    #[wasm_bindgen(js_name=getProtocolVersion)]
    pub fn get_protocol_version(&self) -> u32 {
        self.0.protocol_version.clone()
    }

    #[wasm_bindgen(js_name=getId)]
    pub fn get_id(&self) -> IdentifierWrapper {
        self.0.id.clone().into()
    }

    #[wasm_bindgen(js_name=getType)]
    pub fn get_type(&self) -> String {
        self.0.document_type.clone()
    }

    #[wasm_bindgen(js_name=getDataContractId)]
    pub fn get_data_contract_id(&self) -> IdentifierWrapper {
        self.0.data_contract_id.clone().into()
    }

    #[wasm_bindgen(js_name=getDataContract)]
    pub fn get_data_contract(&self) -> DataContractWasm {
        self.0.data_contract.clone().into()
    }

    #[wasm_bindgen(js_name=getOwnerId)]
    pub fn get_owner_id(&self) -> IdentifierWrapper {
        self.0.owner_id.clone().into()
    }

    #[wasm_bindgen(js_name=setRevision)]
    pub fn set_revision(&mut self, rev: i64) {
        self.0.revision = rev
    }

    #[wasm_bindgen(js_name=getRevision)]
    pub fn get_revision(&mut self) -> i64 {
        self.0.revision
    }

    #[wasm_bindgen(js_name=setUntropy)]
    pub fn set_entropy(&mut self, e: Vec<u8>) {
        self.0.entropy = Some(e.try_into().expect("unable to convert entropy to u8;32"));
    }

    #[wasm_bindgen(js_name=getEntropy)]
    pub fn get_entropy(&mut self) -> Option<Vec<u8>> {
        match self.0.entropy {
            Some(e) => Some(e.to_vec()),
            None => None,
        }
    }

    #[wasm_bindgen(js_name=setData)]
    pub fn set_data(&mut self, d: JsValue) {
        self.0.data = d.into_serde().expect("unable convert data to json object");
    }

    #[wasm_bindgen(js_name=getData)]
    pub fn get_data(&mut self) -> JsValue {
        JsValue::from_serde(&self.0.data).expect("unable convert data to js object")
    }

    #[wasm_bindgen(js_name=set)]
    pub fn set(&mut self, path: String, d: JsValue) {
        // TODO use lodash via extern
        unimplemented!()
    }

    #[wasm_bindgen(js_name=get)]
    pub fn get(&mut self, path: String) -> JsValue {
        // TODO use lodash via extern
        unimplemented!()
    }

    #[wasm_bindgen(js_name=setCreatedAt)]
    pub fn set_created_at(&mut self, ts: i64) {
        self.0.created_at = Some(ts);
    }

    #[wasm_bindgen(js_name=setUpdatedAt)]
    pub fn set_updated_at(&mut self, ts: i64) {
        self.0.updated_at = Some(ts);
    }

    #[wasm_bindgen(js_name=getCreatedAt)]
    pub fn get_created_at(&self) -> Option<i64> {
        self.0.created_at
    }

    #[wasm_bindgen(js_name=getUpdatedAt)]
    pub fn get_updated_at(&self) -> Option<i64> {
        self.0.updated_at
    }

    #[wasm_bindgen(js_name=getMetadata)]
    pub fn get_metadata(&self) -> Option<MetadataWasm> {
        self.0.metadata.clone().map(Into::into)
    }

    #[wasm_bindgen(js_name=setMetadata)]
    pub fn set_metadata(mut self, metadata: MetadataWasm) -> Self {
        self.0.metadata = Some(metadata.into());
        self
    }

    #[wasm_bindgen(js_name=toObject)]
    pub fn to_object(&self) -> JsValue {
        return JsValue::from_serde(&self.0).expect("unable to convert Document to JS object");
    }

    #[wasm_bindgen(js_name=toJSON)]
    pub fn to_json(&self) -> JsValue {
        return self.to_object();
    }

    #[wasm_bindgen(js_name=toString)]
    pub fn to_string(&self) -> String {
        return serde_json::to_string(&self.0).expect("unable to convert Document to string");
    }

    #[wasm_bindgen(js_name=toBuffer)]
    pub fn to_buffer(&self) -> Vec<u8> {
        self.0
            .to_buffer()
            .expect("unable to serialize the Document to buffer")
    }

    #[wasm_bindgen(js_name=hash)]
    pub fn hash(&self) -> Vec<u8> {
        self.0.hash().expect("unable generate hash from Document")
    }
}
