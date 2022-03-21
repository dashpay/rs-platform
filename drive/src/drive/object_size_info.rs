use grovedb::Element;
use crate::contract::{Contract, Document, DocumentType};

pub enum PathInfo<'a, P>
where
P: IntoIterator<Item = &'a [u8]>,
<P as IntoIterator>::IntoIter: ExactSizeIterator + DoubleEndedIterator + Clone,
{
    /// An into iter Path
    PathIterator(P),
    /// A path size
    PathSize(usize),
}

pub enum KeyInfo<'a> {
    /// An ordinary key
    Key(&'a [u8]),
    /// A key size
    KeySize(usize),
}

impl<'a> KeyInfo<'a>  {
    pub fn len(&'a self) -> usize {
        match self {
            KeyInfo::Key(key) => { key.len()}
            KeyInfo::KeySize(key_size) => {*key_size}
        }
    }
}

pub enum PathKeyInfo<'a, P>
    where
        P: IntoIterator<Item = &'a [u8]>,
        <P as IntoIterator>::IntoIter: ExactSizeIterator + DoubleEndedIterator + Clone,
{
    /// An into iter Path with a Key
    PathKey((P, &'a [u8])),
    /// A path size
    PathKeySize((usize, usize)),
}

impl<'a, P> PathKeyInfo<'a, P>
    where
    P: IntoIterator<Item = &'a [u8]>,
    <P as IntoIterator>::IntoIter: ExactSizeIterator + DoubleEndedIterator + Clone,
 {
    pub fn len(&'a self) -> usize {
        match self {
            PathKeyInfo::PathKey(key) => { key.len()}
            PathKeyInfo::PathKeySize(path_key_size) => {*path_key_size}
        }
    }
}

pub enum ElementInfo {
    /// An element
    Element(Element),
    /// An element size
    ElementSize(usize),
}

pub enum KeyElementInfo<'a> {
    /// An element
    KeyElement((&'a [u8], Element)),
    /// An element size
    KeyElementSize((usize,usize)),
}

pub struct DocumentAndContractInfo<'a> {
    pub document_info: DocumentInfo<'a>,
    pub contract: &'a Contract,
    pub document_type_name: &'a str,
    pub document_type: &'a DocumentType,
    pub owner_id: Option<&'a [u8]>,
}

pub enum DocumentInfo<'a> {
    /// The document and it's serialized form
    DocumentAndSerialization((&'a Document, &'a [u8])),
    /// An element size
    DocumentSize(usize),
}