use crate::drive::flags::StorageFlags::{
    MultiEpoch, MultiEpochOwned, SingleEpoch, SingleEpochOwned,
};
use grovedb::ElementFlags;
use integer_encoding::VarInt;
use nohash_hasher::IntMap;
use std::cmp::Ordering;

use crate::error::drive::DriveError;
use crate::error::storage_flags::StorageFlagsError;
use crate::error::Error;

type EpochIndex = u16;

type BaseEpoch = EpochIndex;

type BytesAddedInEpoch = u32;

type OwnerId = [u8; 32];

// Struct Definitions
#[derive(Clone, Debug, PartialEq)]
pub enum StorageFlags {
    SingleEpoch(BaseEpoch),                                                     //0
    MultiEpoch(BaseEpoch, IntMap<EpochIndex, BytesAddedInEpoch>),               //1
    SingleEpochOwned(BaseEpoch, OwnerId),                                       //2
    MultiEpochOwned(BaseEpoch, IntMap<EpochIndex, BytesAddedInEpoch>, OwnerId), //3
}

impl StorageFlags {
    fn combine_owner_id(self, rhs: &Self) -> Result<Option<&OwnerId>, Error> {
        if let Some(our_owner_id) = self.owner_id() {
            if let Some(other_owner_id) = rhs.owner_id() {
                if our_owner_id != other_owner_id {
                    return Err(Error::StorageFlags(
                        StorageFlagsError::MergingStorageFlagsFromDifferentOwners(
                            "can not merge from different owners",
                        ),
                    ));
                }
            }
            Ok(Some(our_owner_id))
        } else if let Some(other_owner_id) = rhs.owner_id() {
            Ok(Some(other_owner_id))
        } else {
            Ok(None)
        }
    }

    fn combine_non_base_epoch_bytes(
        self,
        rhs: &Self,
    ) -> Option<IntMap<EpochIndex, BytesAddedInEpoch>> {
        if let Some(our_epoch_index_map) = self.epoch_index_map() {
            if let Some(other_epoch_index_map) = rhs.epoch_index_map() {
                let mut combined_index_map = our_epoch_index_map.clone();
                other_epoch_index_map
                    .iter()
                    .for_each(|(epoch_index, bytes_added)| {
                        let original_value = combined_index_map.remove(epoch_index);
                        match original_value {
                            None => {
                                combined_index_map.insert(epoch_index.clone(), bytes_added.clone())
                            }
                            Some(original_bytes) => combined_index_map.insert(
                                epoch_index.clone(),
                                original_bytes.clone() + bytes_added.clone(),
                            ),
                        };
                    });
                Some(combined_index_map)
            } else {
                Some(our_epoch_index_map.clone())
            }
        } else if let Some(other_epoch_index_map) = rhs.epoch_index_map() {
            Some(other_epoch_index_map.clone())
        } else {
            None
        }
    }

    fn combine_same_base_epoch(self, rhs: Self) -> Result<Self, Error> {
        let base_epoch = self.base_epoch().clone();
        let owner_id = self.combine_owner_id(&rhs)?;
        let other_epoch_bytes = self.combine_non_base_epoch_bytes(&rhs);

        match (owner_id, other_epoch_bytes) {
            (None, None) => Ok(SingleEpoch(base_epoch)),
            (Some(owner_id), None) => Ok(SingleEpochOwned(base_epoch, owner_id.clone())),
            (None, Some(other_epoch_bytes)) => Ok(MultiEpoch(base_epoch, other_epoch_bytes)),
            (Some(owner_id), Some(other_epoch_bytes)) => Ok(MultiEpochOwned(
                base_epoch,
                other_epoch_bytes,
                owner_id.clone(),
            )),
        }
    }

    fn combine(self, rhs: Self) -> Result<Self, Error> {
        match self.base_epoch().cmp(rhs.base_epoch()) {
            Ordering::Equal => self.combine_same_base_epoch(rhs),
            Ordering::Less => Err(Error::StorageFlags(
                StorageFlagsError::MergingStorageFlagsWithDifferentBaseEpoch(
                    "can not merge with different base epoch",
                ),
            )),
            Ordering::Greater => Err(Error::StorageFlags(
                StorageFlagsError::MergingStorageFlagsWithDifferentBaseEpoch(
                    "can not merge with different base epoch",
                ),
            )),
        }
    }

    pub fn base_epoch(&self) -> &BaseEpoch {
        match self {
            SingleEpoch(base_epoch)
            | MultiEpoch(base_epoch, _)
            | SingleEpochOwned(base_epoch, _)
            | MultiEpochOwned(base_epoch, _, _) => base_epoch,
        }
    }

    pub fn owner_id(&self) -> Option<&OwnerId> {
        match self {
            SingleEpochOwned(_, owner_id) | MultiEpochOwned(_, _, owner_id) => Some(owner_id),
            _ => None,
        }
    }

    pub fn epoch_index_map(&self) -> Option<&IntMap<EpochIndex, BytesAddedInEpoch>> {
        match self {
            MultiEpoch(_, epoch_int_map) | MultiEpochOwned(_, epoch_int_map, _) => {
                Some(epoch_int_map)
            }
            _ => None,
        }
    }

    pub fn optional_default() -> Option<Self> {
        None
    }

    pub fn optional_default_as_ref() -> Option<&'static Self> {
        None
    }

    pub fn type_byte(&self) -> u8 {
        match self {
            SingleEpoch(_) => 0,
            MultiEpoch(..) => 1,
            SingleEpochOwned(..) => 2,
            MultiEpochOwned(..) => 3,
        }
    }

    fn append_to_vec_base_epoch(&self, buffer: &mut Vec<u8>) {
        match self {
            SingleEpoch(base_epoch)
            | MultiEpoch(base_epoch, ..)
            | SingleEpochOwned(base_epoch, ..)
            | MultiEpochOwned(base_epoch, ..) => buffer.extend(base_epoch.to_be_bytes()),
        }
    }

    fn maybe_append_to_vec_epoch_map(&self, buffer: &mut Vec<u8>) {
        match self {
            MultiEpoch(_, epoch_map) | MultiEpochOwned(_, epoch_map, _) => {
                epoch_map.iter().for_each(|(epoch_index, bytes_added)| {
                    buffer.extend(epoch_index.to_be_bytes());
                    buffer.extend(bytes_added.encode_var_vec());
                })
            }
            _ => {}
        }
    }

    fn maybe_append_to_vec_owner_id(&self, buffer: &mut Vec<u8>) {
        match self {
            SingleEpochOwned(_, owner_id) | MultiEpochOwned(_, _, owner_id) => {
                buffer.extend(owner_id);
            }
            _ => {}
        }
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut buffer = vec![self.type_byte()];
        self.maybe_append_to_vec_owner_id(&mut buffer);
        self.append_to_vec_base_epoch(&mut buffer);
        self.maybe_append_to_vec_epoch_map(&mut buffer);
        buffer
    }

    pub fn deserialize_single_epoch(data: &[u8]) -> Result<Self, Error> {
        if data.len() != 3 {
            Err(Error::StorageFlags(
                StorageFlagsError::StorageFlagsWrongSize("single epoch must be 3 bytes total"),
            ))
        } else {
            let epoch = u16::from_be_bytes(data[1..3].try_into().map_err(|_| {
                Error::StorageFlags(StorageFlagsError::StorageFlagsWrongSize(
                    "single epoch must be 3 bytes total",
                ))
            })?);
            Ok(SingleEpoch(epoch))
        }
    }

    pub fn deserialize_multi_epoch(data: &[u8]) -> Result<Self, Error> {
        let len = data.len();
        if len < 6 {
            Err(Error::StorageFlags(
                StorageFlagsError::StorageFlagsWrongSize(
                    "multi epoch must be at least 6 bytes total",
                ),
            ))
        } else {
            let base_epoch = u16::from_be_bytes(data[1..3].try_into().map_err(|_| {
                Error::StorageFlags(StorageFlagsError::StorageFlagsWrongSize(
                    "multi epoch must have enough bytes for the base epoch",
                ))
            })?);
            let mut offset = 3;
            let mut bytes_per_epoch: IntMap<u16, u32> = IntMap::default();
            while offset + 2 < len {
                // 2 for epoch size
                let epoch_index =
                    u16::from_be_bytes(data[offset..offset + 2].try_into().map_err(|_| {
                        Error::StorageFlags(StorageFlagsError::StorageFlagsWrongSize(
                            "multi epoch must have enough bytes epoch indexes",
                        ))
                    })?);
                offset += 2;
                let (bytes_at_epoch, bytes_used) = u32::decode_var(&data[offset..]).ok_or(
                    Error::StorageFlags(StorageFlagsError::StorageFlagsWrongSize(
                        "multi epoch must have enough bytes for the amount of bytes used",
                    )),
                )?;
                offset += bytes_used;
                bytes_per_epoch.insert(epoch_index, bytes_at_epoch);
            }
            Ok(MultiEpoch(base_epoch, bytes_per_epoch))
        }
    }

    pub fn deserialize_single_epoch_owned(data: &[u8]) -> Result<Self, Error> {
        if data.len() != 35 {
            Err(Error::StorageFlags(
                StorageFlagsError::StorageFlagsWrongSize(
                    "single epoch owned must be 35 bytes total",
                ),
            ))
        } else {
            let owner_id: OwnerId = data[1..33].try_into().map_err(|_| {
                Error::StorageFlags(StorageFlagsError::StorageFlagsWrongSize(
                    "single epoch owned must be 35 bytes total for owner id",
                ))
            })?;
            let epoch = u16::from_be_bytes(data[33..35].try_into().map_err(|_| {
                Error::StorageFlags(StorageFlagsError::StorageFlagsWrongSize(
                    "single epoch owned must be 35 bytes total for epoch",
                ))
            })?);
            Ok(SingleEpochOwned(epoch, owner_id))
        }
    }

    pub fn deserialize_multi_epoch_owned(data: &[u8]) -> Result<Self, Error> {
        let len = data.len();
        if len < 38 {
            Err(Error::StorageFlags(
                StorageFlagsError::StorageFlagsWrongSize(
                    "multi epoch owned must be at least 38 bytes total",
                ),
            ))
        } else {
            let owner_id: OwnerId = data[1..33].try_into().map_err(|_| {
                Error::StorageFlags(StorageFlagsError::StorageFlagsWrongSize(
                    "multi epoch owned must be 38 bytes total for owner id",
                ))
            })?;
            let base_epoch = u16::from_be_bytes(data[33..35].try_into().map_err(|_| {
                Error::StorageFlags(StorageFlagsError::StorageFlagsWrongSize(
                    "multi epoch must have enough bytes for the base epoch",
                ))
            })?);
            let mut offset = 3;
            let mut bytes_per_epoch: IntMap<u16, u32> = IntMap::default();
            while offset + 2 < len {
                // 2 for epoch size
                let epoch_index =
                    u16::from_be_bytes(data[offset..offset + 2].try_into().map_err(|_| {
                        Error::StorageFlags(StorageFlagsError::StorageFlagsWrongSize(
                            "multi epoch must have enough bytes epoch indexes",
                        ))
                    })?);
                offset += 2;
                let (bytes_at_epoch, bytes_used) = u32::decode_var(&data[offset..]).ok_or(
                    Error::StorageFlags(StorageFlagsError::StorageFlagsWrongSize(
                        "multi epoch must have enough bytes for the amount of bytes used",
                    )),
                )?;
                offset += bytes_used;
                bytes_per_epoch.insert(epoch_index, bytes_at_epoch);
            }
            Ok(MultiEpochOwned(base_epoch, bytes_per_epoch, owner_id))
        }
    }

    pub fn deserialize(data: &[u8]) -> Result<Option<Self>, Error> {
        let first_byte = data.get(0);
        match first_byte {
            None => Ok(None),
            Some(first_byte) => match *first_byte {
                0 => Ok(Some(Self::deserialize_single_epoch(data)?)),
                1 => Ok(Some(Self::deserialize_multi_epoch(data)?)),
                2 => Ok(Some(Self::deserialize_single_epoch_owned(data)?)),
                3 => Ok(Some(Self::deserialize_multi_epoch_owned(data)?)),
                _ => Err(Error::StorageFlags(
                    StorageFlagsError::DeserializeUnknownStorageFlagsType(
                        "unknown storage flags serialization",
                    ),
                )),
            },
        }
    }

    pub fn from_slice(data: &[u8]) -> Result<Option<Self>, Error> {
        Self::deserialize(data)
    }

    pub fn from_some_element_flags(data: &Option<ElementFlags>) -> Result<Option<Self>, Error> {
        let data = data.ok_or(Error::Drive(DriveError::CorruptedElementFlags(
            "no element flag on data",
        )))?;
        Self::from_slice(data.as_slice())
    }

    pub fn from_element_flags_ref(data: &Option<ElementFlags>) -> Result<Option<Self>, Error> {
        let data = data
            .as_ref()
            .ok_or(Error::Drive(DriveError::CorruptedElementFlags(
                "no element flag on data",
            )))?;
        Self::from_slice(data.as_slice())
    }

    pub fn from_some_element_flags_ref(data: &Option<ElementFlags>) -> Result<Option<Self>, Error> {
        let data = data
            .as_ref()
            .ok_or(Error::Drive(DriveError::CorruptedElementFlags(
                "no element flag on data",
            )))?;
        Self::from_slice(data.as_slice())
    }

    pub fn map_to_some_element_flags(maybe_storage_flags: Option<&Self>) -> Option<ElementFlags> {
        maybe_storage_flags.map(|storage_flags| storage_flags.serialize())
    }

    pub fn to_some_element_flags(&self) -> Option<ElementFlags> {
        Some(self.serialize())
    }

    pub fn to_element_flags(&self) -> ElementFlags {
        self.serialize()
    }
}
