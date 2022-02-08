use std::collections::HashMap;
use crate::contract::Contract;
use crate::drive::Drive;
use byteorder::{BigEndian, WriteBytesExt};
use std::fs::File;
use std::io;
use std::io::{BufRead, BufReader};
use std::path::Path;
use ciborium::value::Value as CborValue;
use storage::rocksdb_storage::OptimisticTransactionDBTransaction;
use grovedb::Error;

pub fn setup_contract(
    drive: &mut Drive,
    path: &str,
    transaction: Option<&OptimisticTransactionDBTransaction>,
) -> Contract {
    let contract_cbor = json_document_to_cbor(path, Some(crate::drive::defaults::PROTOCOL_VERSION));
    let contract = Contract::from_cbor(&contract_cbor).expect("contract should be deserialized");
    drive
        .apply_contract(contract_cbor, transaction)
        .expect("contract should be applied");
    contract
}

pub fn json_document_to_cbor(path: impl AsRef<Path>, protocol_version: Option<u32>) -> Vec<u8> {
    let file = File::open(path).expect("file not found");
    let reader = BufReader::new(file);
    let json: serde_json::Value = serde_json::from_reader(reader).expect("expected a valid json");
    value_to_cbor(json, protocol_version)
}

pub fn value_to_cbor(value: serde_json::Value, protocol_version: Option<u32>) -> Vec<u8> {
    let mut buffer: Vec<u8> = Vec::new();
    if let Some(protocol_version) = protocol_version {
        buffer.write_u32::<BigEndian>(protocol_version);
    }
    ciborium::ser::into_writer(&value, &mut buffer).expect("unable to serialize into cbor");
    buffer
}

pub fn text_file_strings(path: impl AsRef<Path>) -> Vec<String> {
    let file = File::open(path).expect("file not found");
    let reader = io::BufReader::new(file).lines();
    reader.into_iter().map(|a| a.unwrap()).collect()
}

pub fn bytes_for_system_value(value: &CborValue) -> Option<Vec<u8>> {
    match value {
        CborValue::Bytes(bytes) => Some(bytes.clone()),
        CborValue::Text(text) => match bs58::decode(text).into_vec() {
            Ok(data) => Some(data),
            Err(_) => None,
        },
        CborValue::Array(array) => {
            let bytes_result: Result<Vec<u8>, Error> = array
                .iter()
                .map(|byte| match byte {
                    CborValue::Integer(int) => {
                        let value_as_u8: u8 = (*int)
                            .try_into()
                            .map_err(|_| Error::CorruptedData(String::from("expected u8 value")))?;
                        Ok(value_as_u8)
                    }
                    _ => Err(Error::CorruptedData(String::from(
                        "not an array of integers",
                    ))),
                })
                .collect::<Result<Vec<u8>, Error>>();
            match bytes_result {
                Ok(bytes) => Some(bytes),
                Err(_) => None,
            }
        }
        _ => None,
    }
}

pub fn bytes_for_system_value_from_hash_map(
    document: &HashMap<String, CborValue>,
    key: &str,
) -> Option<Vec<u8>> {
    document.get(key).and_then(bytes_for_system_value)
}
