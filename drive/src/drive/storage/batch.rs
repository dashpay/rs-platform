use crate::drive::flags::StorageFlags;
use crate::drive::object_size_info::{KeyInfo, PathKeyElementInfo};
use crate::drive::Drive;
use crate::error::drive::DriveError;
use crate::error::Error;
use crate::fee::op::DriveOperation;
use grovedb::TransactionArg;

pub struct Batch<'d> {
    pub drive: &'d Drive,
    operations: Vec<DriveOperation>,
}

impl<'d> Batch<'d> {
    pub fn new(drive: &Drive) -> Self {
        Batch {
            drive,
            operations: Vec::new(),
        }
    }

    pub fn insert_empty_tree<'c, P>(
        &mut self,
        path: P,
        key_info: KeyInfo<'c>,
        storage_flags: Option<&StorageFlags>,
    ) -> Result<(), Error>
    where
        P: IntoIterator<Item = &'c [u8]>,
        <P as IntoIterator>::IntoIter: ExactSizeIterator + DoubleEndedIterator + Clone,
    {
        self.drive
            .batch_insert_empty_tree(path, key_info, storage_flags, &mut self.operations)
    }

    pub fn insert<const N: usize>(
        &mut self,
        path_key_element_info: PathKeyElementInfo<N>,
    ) -> Result<(), Error> {
        self.drive
            .batch_insert(path_key_element_info, &mut self.operations)
    }

    pub fn delete<'c, P>(
        &mut self,
        path: P,
        key: &'c [u8],
        only_delete_tree_if_empty: bool,
        transaction: TransactionArg,
    ) -> Result<(), Error>
    where
        P: IntoIterator<Item = &'c [u8]>,
        <P as IntoIterator>::IntoIter: ExactSizeIterator + DoubleEndedIterator + Clone,
    {
        self.drive.batch_delete(
            path,
            key,
            only_delete_tree_if_empty,
            transaction,
            &mut self.operations,
        )
    }

    pub fn apply(mut self, validate: bool, transaction: TransactionArg) -> Result<(), Error> {
        if self.operations.len() == 0 {
            return Err(Error::Drive(DriveError::BatchIsEmpty()));
        }

        self.apply_if_not_empty(validate, transaction)
    }

    pub fn apply_if_not_empty(
        mut self,
        validate: bool,
        transaction: TransactionArg,
    ) -> Result<(), Error> {
        if self.operations.len() == 0 {
            return Err(Error::Drive(DriveError::BatchIsEmpty()));
        }

        let grovedb_operations = DriveOperation::grovedb_operations(&self.operations);

        self.drive.grove_apply_batch(
            grovedb_operations,
            validate,
            transaction,
            &mut self.operations,
        )?;

        Ok(())
    }
}
