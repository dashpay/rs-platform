use enum_map::{enum_map, Enum, EnumMap};
use grovedb::{Element, Error};

#[derive(Debug, Enum)]
pub enum Op {
    Stop,
    Add,
    Mul,
    Sub,
    Div,
    Sdiv,
    Mod,
    Smod,
    Addmod,
    Mulmod,
    Signextend,
    Lt,
    Gt,
    Slt,
    Sgt,
    Eq,
    Iszero,
    And,
    Or,
    Xor,
    Not,
    Byte,
    Exp,
    Sha256,
    Sha256_2,
    Blake3,
    Read,
    Store
}

pub struct InsertOperation {
    pub size: u64,
}

impl InsertOperation {
    pub fn for_empty_tree(key_size: usize) -> Self {
        InsertOperation{
            size: key_size as u64
        }
    }
    pub fn for_key_value(key_size: usize, element: Element) -> Self {
        let value_size = match element {
            Element::Item(item) => { item.len()}
            Element::Reference(path) => {path.iter().map(| inner| inner.sum()).collect()}
            Element::Tree(_) => { 32 }
        };
        InsertOperation{
            size: key_size as u64 + value_size as u64
        }
    }
}