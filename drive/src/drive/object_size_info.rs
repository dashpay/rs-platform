use crate::contract::{Contract, Document, DocumentType};
use crate::drive::object_size_info::KeyInfo::{Key, KeyRef, KeySize};
use crate::drive::object_size_info::PathInfo::{PathFixedSizeIterator, PathIterator, PathSize};
use crate::drive::object_size_info::PathKeyElementInfo::{PathKeyElement, PathKeyElementSize};
use crate::drive::object_size_info::PathKeyInfo::{PathKey, PathKeyRef, PathKeySize};
use grovedb::{Element, Error};

pub enum PathInfo<'a, P>
where
    P: IntoIterator<Item = &'a [u8]>,
    <P as IntoIterator>::IntoIter: ExactSizeIterator + DoubleEndedIterator + Clone,
{
    /// An into iter Path
    PathFixedSizeIterator(P),

    /// An into iter Path
    PathIterator(Vec<Vec<u8>>),

    /// A path size
    PathSize(usize),
}

impl<'a, P> PathInfo<'a, P>
where
    P: IntoIterator<Item = &'a [u8]>,
    <P as IntoIterator>::IntoIter: ExactSizeIterator + DoubleEndedIterator + Clone,
{

    pub fn len(&self) -> usize {
        match self {
            PathFixedSizeIterator(path_iterator) => {
                path_iterator.clone().into_iter().map(|a| a.len()).sum()
            }
            PathIterator(path_iterator) => path_iterator.clone().into_iter().map(|a| a.len()).sum(),
            PathSize(path_size) => *path_size,
        }
    }

    pub fn push(&mut self, key_info: KeyInfo) -> Result<(), Error> {
        match self {
            PathFixedSizeIterator(_) => {
                return Err(Error::CorruptedData(String::from(
                    "can not add a key to a fixed sized path iterator",
                )))
            }
            PathIterator(path_iterator) => match key_info {
                Key(key) => path_iterator.push(key),
                KeyRef(key_ref) => path_iterator.push(Vec::from(key_ref)),
                KeySize(key_size) => {
                    return Err(Error::CorruptedData(String::from(
                        "can not add a key size to path iterator",
                    )))
                }
            },
            PathSize(mut path_size) => match key_info {
                Key(key) => path_size += key.len(),
                KeyRef(key_ref) => path_size += key_ref.len(),
                KeySize(key_size) => path_size += key_size,
            },
        }
        Ok(())
    }
}

pub enum KeyInfo<'a> {
    /// A key
    Key(Vec<u8>),
    /// A key by reference
    KeyRef(&'a [u8]),
    /// A key size
    KeySize(usize),
}

impl<'a> Default for KeyInfo<'a> {
    fn default() -> Self {
        Key(vec![])
    }
}

impl<'a> KeyInfo<'a> {
    pub fn len(&'a self) -> usize {
        match self {
            Key(key) => key.len(),
            KeyRef(key) => key.len(),
            KeySize(key_size) => *key_size,
        }
    }

    pub fn add_path_info<P>(self, path_info: &PathInfo<'a, P>) -> Result<PathKeyInfo<'a, P>, Error>
        where
            P: IntoIterator<Item = &'a [u8]>,
            <P as IntoIterator>::IntoIter: ExactSizeIterator + DoubleEndedIterator + Clone,
    {
        match self {
            Key(key) => {
                if let PathIterator(iter) = path_info {
                    Ok(PathKey((iter, key)))
                } else if let PathFixedSizeIterator(iter) = path_info {
                    Ok(PathKey((iter.clone(), key)))
                } else {
                    Err(Error::CorruptedData(String::from(
                        "request for path iterator on path size",
                    )))
                }
            },
            KeyRef(key_ref) => {
                if let PathIterator(iter) = path_info {
                    Ok(PathKeyRef((iter, key_ref)))
                } else if let PathFixedSizeIterator(iter) = path_info {
                    Ok(PathKeyRef((iter, key_ref)))
                } else {
                    Err(Error::CorruptedData(String::from(
                        "request for path iterator on path size",
                    )))
                }
            },
            KeySize(key_size) => Ok(PathKeySize((path_info.len(), key_size))),
        }
    }

    pub fn add_path<P>(self, path: P) -> PathKeyInfo<'a, P>
        where
            P: IntoIterator<Item = &'a [u8]>,
            <P as IntoIterator>::IntoIter: ExactSizeIterator + DoubleEndedIterator + Clone,
    {
        match self {
            Key(key) => PathKey((path, key)),
            KeyRef(key_ref) => PathKeyRef((path, key_ref)),
            KeySize(key_size) => PathKeySize((path.len(), key_size)),
        }
    }
}

pub enum PathKeyInfo<'a, P>
where
    P: IntoIterator<Item = &'a [u8]>,
    <P as IntoIterator>::IntoIter: ExactSizeIterator + DoubleEndedIterator + Clone,
{
    /// An into iter Path with a Key
    PathKey((P, Vec<u8>)),
    /// An into iter Path with a Key
    PathKeyRef((P, &'a [u8])),
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
            PathKey((path_iterator, key)) => {
                path_iterator.clone().into_iter().map(|a| a.len()).sum() + key.len()
            }
            PathKeyRef((path_iterator, key)) => {
                path_iterator.clone().into_iter().map(|a| a.len()).sum() + key.len()
            }
            PathKeySize((path_size, key_size)) => *path_size + *key_size,
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
    KeyElementSize((usize, usize)),
}

pub enum PathKeyElementInfo<'a, P>
where
    P: IntoIterator<Item = &'a [u8]>,
    <P as IntoIterator>::IntoIter: ExactSizeIterator + DoubleEndedIterator + Clone,
{
    /// A triple Path Key and Element
    PathKeyElement((P, &'a [u8], Element)),
    /// A triple of sum of Path lengths, Key length and Element size
    PathKeyElementSize((usize, usize, usize)),
}

impl<'a, P> PathKeyElementInfo<'a, P>
where
    P: IntoIterator<Item = &'a [u8]>,
    <P as IntoIterator>::IntoIter: ExactSizeIterator + DoubleEndedIterator + Clone,
{
    pub fn from_path_info_and_key_element(
        path_info: PathInfo<'a, P>,
        key_element: KeyElementInfo,
    ) -> Result<Self, Error> {
        match path_info {
            PathIterator(path_interator) => match key_element {
                KeyElementInfo::KeyElement((key, element)) => {
                    Ok(PathKeyElement((path_interator, key, element)))
                }
                KeyElementInfo::KeyElementSize(_) => Err(Error::CorruptedData(String::from(
                    "path matched with key element size",
                ))),
            },
            PathSize(path_size) => match key_element {
                KeyElementInfo::KeyElement((key, element)) => Ok(PathKeyElementSize((
                    path_size,
                    key.len(),
                    element.node_byte_size(key),
                ))),
                KeyElementInfo::KeyElementSize((key_len, element_size)) => {
                    Ok(PathKeyElementSize((path_size, key_len, element_size)))
                }
            },
            PathFixedSizeIterator(path_interator) => match key_element {
                KeyElementInfo::KeyElement((key, element)) => {
                    Ok(PathKeyElement((path_interator, key, element)))
                }
                KeyElementInfo::KeyElementSize(_) => Err(Error::CorruptedData(String::from(
                    "path matched with key element size",
                ))),
            },
        }
    }

    pub fn from_path_and_key_element(path: P, key_element: KeyElementInfo) -> Result<Self, Error> {
        match key_element {
            KeyElementInfo::KeyElement((key, element)) => Ok(PathKeyElement((path, key, element))),
            KeyElementInfo::KeyElementSize(_) => Err(Error::CorruptedData(String::from(
                "path matched with key element size",
            ))),
        }
    }

    pub fn insert_len(&'a self) -> usize {
        match self {
            //todo v23: this is an incorrect approximation
            PathKeyElementInfo::PathKeyElement((_, key, element)) => element.node_byte_size(key),
            PathKeyElementInfo::PathKeyElementSize((_, key_size, element_size)) => {
                *key_size + *element_size
            }
        }
    }
}

pub struct DocumentAndContractInfo<'a> {
    pub document_info: DocumentInfo<'a>,
    pub contract: &'a Contract,
    pub document_type: &'a DocumentType,
    pub owner_id: Option<&'a [u8]>,
}

pub enum DocumentInfo<'a> {
    /// The document and it's serialized form
    DocumentAndSerialization((&'a Document, &'a [u8])),
    /// An element size
    DocumentSize(usize),
}

impl<'a> DocumentInfo<'a> {
    pub fn is_document_and_serialization(&self) -> bool {
        match self {
            DocumentInfo::DocumentAndSerialization(_) => true,
            DocumentInfo::DocumentSize(_) => false,
        }
    }

    pub fn id_key_info(&self) -> KeyInfo {
        match self {
            DocumentInfo::DocumentAndSerialization((document, _)) => {
                KeyInfo::KeyRef(document.id.as_slice())
            }
            DocumentInfo::DocumentSize(_) => 32,
        }
    }

    pub fn get_raw_for_document_type(
        &self,
        key_path: &str,
        document_type: &DocumentType,
        owner_id: Option<&[u8]>,
    ) -> Result<Option<KeyInfo>, Error> {
        match self {
            DocumentInfo::DocumentAndSerialization((document, _)) => {
                let raw_value =
                    document.get_raw_for_document_type(key_path, document_type, owner_id)?;
                match raw_value {
                    None => Ok(None),
                    Some(value) => Ok(Some(Key(value))),
                }
            }
            DocumentInfo::DocumentSize(_) => {
                let document_field_type =
                    document_type.properties.get(key_path).ok_or_else(|| {
                        Error::CorruptedData(String::from("incorrect key path for document type"))
                    })?;
                let max_size = document_field_type.max_size().ok_or_else(|| {
                    Error::CorruptedData(String::from("document type must have a max size"))
                })?;
                Ok(Some(KeySize(max_size)))
            }
        }
    }
}
