use crate::drive::abci::messages::{
    BlockBeginRequest, BlockBeginResponse, BlockEndRequest, BlockEndResponse, InitChainRequest,
    InitChainResponse,
};
use std::ops::Deref;

use crate::drive::block::BlockExecutionContext;
use crate::drive::block::BlockInfo;
use grovedb::TransactionArg;

use crate::drive::Drive;
use crate::error;
use crate::error::Error;
use crate::fee::epoch::EpochInfo;

pub fn init_chain(
    drive: &Drive,
    _request: InitChainRequest,
    transaction: TransactionArg,
) -> Result<InitChainResponse, Error> {
    drive.start_current_batch()?;

    // TODO: should use batches?
    drive.create_root_tree(transaction)?;

    // initialize the pools with epochs
    drive.fee_pools.borrow().init(drive)?;

    drive.apply_current_batch(false, transaction)?;

    let response = InitChainResponse {};

    Ok(response)
}

pub fn block_begin(
    drive: &Drive,
    request: BlockBeginRequest,
    transaction: TransactionArg,
) -> Result<BlockBeginResponse, Error> {
    // Init block execution context
    let block_execution_context = BlockExecutionContext {
        block_info: BlockInfo::from_block_begin_request(&request),
    };

    drive
        .block_execution_context
        .replace(Some(block_execution_context));

    // Set genesis time
    // TODO Move genesis time out of pools
    if request.block_height == 1 {
        drive
            .fee_pools
            .borrow_mut()
            .update_genesis_time(&drive, request.block_time)?;
    }

    let genesis_time = drive
        .fee_pools
        .borrow_mut()
        .get_genesis_time(&drive, transaction)?;

    // Init epoch info
    let epoch_info = EpochInfo::calculate(
        genesis_time,
        request.block_time,
        request.previous_block_time,
    )?;

    drive.epoch_info.replace(epoch_info);

    let response = BlockBeginResponse {};

    Ok(response)
}

pub fn block_end(
    drive: &Drive,
    request: BlockEndRequest,
    transaction: TransactionArg,
) -> Result<BlockEndResponse, Error> {
    let block_execution_context = drive.block_execution_context.borrow();
    let block_execution_context = match block_execution_context.deref() {
        Some(block_execution_context) => block_execution_context,
        None => {
            return Err(Error::Drive(
                error::drive::DriveError::CorruptedCodeExecution(
                    "block execution context must be set in block begin handler",
                ),
            ))
        }
    };

    drive.start_current_batch()?;

    drive.fee_pools.borrow().process_block_fees(
        &drive,
        &block_execution_context.block_info,
        request.fees.processing_fees,
        request.fees.storage_fees,
        request.fees.fee_multiplier,
        transaction,
    )?;

    drive.apply_current_batch(false, transaction)?;

    let response = BlockEndResponse {};

    Ok(response)
}
