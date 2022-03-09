use std::iter::Sum;
use enum_map::{enum_map, Enum, EnumMap};
use grovedb::{Element, Error};

pub(crate) const STORAGE_CREDIT_PER_BYTE: u32 = 5000;
pub(crate) const QUERY_CREDIT_PER_BYTE: u32 = 10;

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

pub struct QueryOperation {
    pub key_size: u16,
    pub path_size: u32,
}

impl QueryOperation {
    pub fn for_key_in_path<'a: 'b, 'b, 'c, P>(key: &[u8], path: P) -> Self
        where
            P: IntoIterator<Item = &'c [u8]>,
            <P as IntoIterator>::IntoIter: ExactSizeIterator + DoubleEndedIterator + Clone,
    {
        let path_size: u32 = path.into_iter().map(| inner: &[u8] | inner.len() as u32).sum();
        QueryOperation {
            key_size: key.len() as u16,
            path_size,
        }
    }

    pub fn data_size(&self) -> u32 {
        self.path_size + self.key_size as u32
    }

    pub fn cost(&self) -> u32 {
        self.data_size() * QUERY_CREDIT_PER_BYTE
    }
}

pub struct InsertOperation {
    pub key_size: u16,
    pub value_size: u32,
}

impl InsertOperation {
    pub fn for_empty_tree(key_size: usize) -> Self {
        InsertOperation{
            key_size: key_size as u16,
            value_size: 0
        }
    }
    pub fn for_key_value(key_size: usize, element: &Element) -> Self {
        let value_size = match element {
            Element::Item(item) => { item.len()}
            Element::Reference(path) => {path.iter().map(| inner| inner.len()).sum()}
            Element::Tree(_) => { 32 }
        };
        InsertOperation{
            key_size: key_size as u16,
            value_size: value_size as u32,
        }
    }

    pub fn data_size(&self) -> u32 {
        self.value_size + self.key_size as u32
    }

    pub fn cost(&self) -> u32 {
        self.data_size() * STORAGE_CREDIT_PER_BYTE
    }
}