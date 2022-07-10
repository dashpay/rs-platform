use std::cell::RefCell;
use rs_drive::drive::Drive;
use std::path::Path;
use rs_drive::drive::config::DriveConfig;
use rs_drive::query::GroveError::StorageError;
use crate::block::BlockExecutionContext;
use crate::error::Error;

pub struct Platform {
    pub drive: Drive,
    pub block_execution_context: RefCell<Option<BlockExecutionContext>>,
}

impl Platform {
    pub fn open<P: AsRef<Path>>(path: P, config: Option<DriveConfig>) -> Result<Self, Error> {
        let drive = Drive::open(path, config).map_err(StorageError)?;
        Ok(Platform {
            drive,
            block_execution_context: RefCell::new(None),
        })
    }
}