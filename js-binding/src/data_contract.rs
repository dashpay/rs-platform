use dpp::data_contract::DataContract;
use dpp::util::string_encoding::Encoding;
pub use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::convert::{TryFrom, TryInto};
use wasm_bindgen::prelude::*;

use crate::identifier::IdentifierWrapper;
use crate::metadata::MetadataWasm;

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

// TODO errors handling

#[wasm_bindgen(js_class=DataContract)]
impl DataContractWasm {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        DataContract::default().into()
    }

    #[wasm_bindgen(js_name=getProtocolVersion)]
    pub fn get_protocol_version(&self) -> u32 {
        self.0.protocol_version
    }

    #[wasm_bindgen(js_name=getId)]
    pub fn get_id(&self) -> IdentifierWrapper {
        self.0.id.clone().into()
    }

    #[wasm_bindgen(js_name=getOwnerId)]
    pub fn get_owner_id(&self) -> IdentifierWrapper {
        self.0.owner_id.clone().into()
    }

    #[wasm_bindgen(js_name=getVersion)]
    pub fn get_version(&self) -> u32 {
        self.0.version
    }

    #[wasm_bindgen(js_name=setVersion)]
    pub fn set_version(&mut self, v: u32) {
        self.0.version = v;
    }

    #[wasm_bindgen(js_name=incrementVersion)]
    pub fn increment_version(&mut self) {
        self.0.increment_version()
    }

    #[wasm_bindgen(js_name=getJsonSchemaId)]
    pub fn get_json_schema_id(&self) -> String {
        self.0.id.to_string(Encoding::Base58)
    }

    #[wasm_bindgen(js_name=setJsonMetaSchema)]
    pub fn set_json_meta_schema(&mut self, schema: String) {
        self.0.schema = schema;
    }

    #[wasm_bindgen(js_name=getJsonMetaSchema)]
    pub fn get_json_meta_schema(&self) -> String {
        self.0.schema.clone()
    }
    #[wasm_bindgen(js_name=setDocuments)]
    pub fn set_documents(&self, documents: JsValue) {
        let json_value: Value =
            JsValue::into_serde(&documents).expect("unable to convert into JSON Value");
        let mut docs: BTreeMap<String, Value> = BTreeMap::new();
        if let Value::Object(o) = json_value {
            for (k, v) in o.into_iter() {
                // v must be a Object
                if !v.is_object() {
                    panic!("{:?} is not an Object", v);
                }
                docs.insert(k, v);
            }
        } else {
            panic!("the parameter is not an Object")
        }
    }

    #[wasm_bindgen(js_name=getDocuments)]
    pub fn get_documents(&self) -> JsValue {
        JsValue::from_serde(&self.0.documents).expect("unable to convert documents to JSValue")
    }

    #[wasm_bindgen(js_name=isDocumentDefined)]
    pub fn is_document_defined(&self, doc_type: String) -> bool {
        self.0.is_document_defined(&doc_type)
    }

    #[wasm_bindgen(js_name=setDocumentSchema)]
    pub fn set_document_schema(&mut self, doc_type: String, schema: JsValue) {
        let json_schema: Value =
            JsValue::into_serde(&schema).expect("unable to convert schema into JSON Value");
        self.0.documents.insert(doc_type, json_schema);
    }

    #[wasm_bindgen(js_name=getDocumentSchema)]
    pub fn get_document_schema(&mut self, doc_type: &str) -> JsValue {
        JsValue::from_serde(
            self.0
                .get_document_schema(doc_type)
                .expect("unable to find document schema"),
        )
        .expect("unable to create JsValue from JSON Value")
    }

    #[wasm_bindgen(js_name=getDocumentSchemaRef)]
    pub fn get_document_schema_ref(&self, doc_type: &str) -> JsValue {
        JsValue::from_serde(&self.0.get_document_schema_ref(doc_type).unwrap()).unwrap()
    }

    #[wasm_bindgen(js_name=setDefinitions)]
    pub fn set_definitions(&self, definitions: JsValue) {
        let json_value: Value =
            JsValue::into_serde(&definitions).expect("unable to convert into JSON Value");
        let mut docs: BTreeMap<String, Value> = BTreeMap::new();
        if let Value::Object(o) = json_value {
            for (k, v) in o.into_iter() {
                // v must be a Object
                if !v.is_object() {
                    panic!("{:?} is not an Object", v);
                }
                docs.insert(k, v);
            }
        } else {
            panic!("the parameter is not an Object")
        }
    }

    #[wasm_bindgen(js_name=getDefinitions)]
    pub fn get_definitions(&self) -> JsValue {
        JsValue::from_serde(&self.0.defs).expect("unable to convert to JsValue")
    }

    #[wasm_bindgen(js_name=setEntropy)]
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

    #[wasm_bindgen(js_name=getBinaryProperties)]
    pub fn get_binary_properties(&self, doc_type: &str) -> JsValue {
        JsValue::from_serde(&self.0.get_binary_properties(doc_type))
            .expect("unable to convert to JsValue")
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
        return JsValue::from_serde(&self.0).expect("unable to convert Data Contract to JS object");
    }

    #[wasm_bindgen(js_name=toJSON)]
    pub fn to_json(&self) -> JsValue {
        return self.to_object();
    }

    #[wasm_bindgen(js_name=toString)]
    pub fn to_string(&self) -> String {
        return serde_json::to_string(&self.0).expect("unable to convert Data Contract to string");
    }

    #[wasm_bindgen(js_name=toBuffer)]
    pub fn to_buffer(&self) -> Vec<u8> {
        self.0
            .to_buffer()
            .expect("unable to serialize the Data Contract to buffer")
    }

    #[wasm_bindgen(js_name=hash)]
    pub fn hash(&self) -> Vec<u8> {
        self.0
            .hash()
            .expect("unable generate hash from Data Contract")
    }

    #[wasm_bindgen(js_name=from)]
    pub fn from(v: JsValue) -> DataContractWasm {
        let json_contract: Value = v.into_serde().unwrap();

        DataContract::try_from(json_contract)
            .expect("unable to convert to contract")
            .into()
    }

    #[wasm_bindgen(js_name=from_buffer)]
    pub fn from_buffer(b: Vec<u8>) -> DataContractWasm {
        DataContract::from_buffer(b).unwrap().into()
    }

    #[wasm_bindgen(js_name=from_string)]
    pub fn from_string(v: &str) -> DataContractWasm {
        DataContract::try_from(v).unwrap().into()
    }
}
