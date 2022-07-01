use crate::drive::abci::messages::{
    BlockBeginRequest, BlockBeginResponse, BlockEndRequest, BlockEndResponse, InitChainRequest,
    InitChainResponse,
};
use crate::drive::block::BlockInfo;
use grovedb::TransactionArg;

use crate::drive::Drive;
use crate::error::Error;
use crate::fee::epoch::EpochInfo;

pub fn init_chain(
    drive: &Drive,
    _request: InitChainRequest,
    transaction: TransactionArg,
) -> Result<InitChainResponse, Error> {
    drive.start_current_batch()?;

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

    // TODO: Make sure we set epoch storage flag everywhere we need
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
    let block_info = BlockInfo::from_block_end_request(&request);

    drive.start_current_batch()?;

    drive.fee_pools.borrow().process_block_fees(
        &drive,
        &block_info,
        request.processing_fees,
        request.storage_fees,
        transaction,
    )?;

    drive.apply_current_batch(false, transaction)?;

    let response = BlockEndResponse {};

    Ok(response)
}
