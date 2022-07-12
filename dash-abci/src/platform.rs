use crate::block::BlockExecutionContext;
use crate::error::Error;
use rs_drive::drive::config::DriveConfig;
use rs_drive::drive::Drive;
use rs_drive::query::GroveError::StorageError;
use std::cell::RefCell;
use std::path::Path;

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
