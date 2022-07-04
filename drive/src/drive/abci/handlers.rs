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

    drive.apply_current_batch(false, transaction)?;

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

#[cfg(test)]
mod tests {
    use crate::drive::abci::{
        handlers::{block_begin, block_end, init_chain},
        messages::{BlockBeginRequest, BlockEndRequest, InitChainRequest},
    };
    use crate::fee::pools::tests::helpers::fee_pools::create_mn_shares_contract;
    use crate::fee::pools::tests::helpers::setup::{setup_drive, setup_fee_pools};

    mod handlers {
        use chrono::{Duration, TimeZone, Utc};

        use crate::{
            drive::abci::messages::Fees,
            fee::pools::{
                epoch::epoch_pool::EpochPool,
                tests::helpers::fee_pools::{
                    populate_proposers, setup_identities_with_share_documents,
                },
            },
        };

        #[test]
        fn test_abci_flow() {
            let drive = super::setup_drive();
            let (transaction, fee_pools) = super::setup_fee_pools(&drive, None);

            // setup the contract
            let contract = super::create_mn_shares_contract(&drive);

            // init chain
            let init_chain_request = super::InitChainRequest {};

            super::init_chain(&drive, init_chain_request, Some(&transaction))
                .expect("to init chain");

            // setup proposers and mn share documents
            let epoch_pool = EpochPool::new(0, &drive);

            epoch_pool.init_proposers().expect("to init proposers");

            drive
                .apply_current_batch(false, Some(&transaction))
                .expect("to apply a batch");

            let proposer_tx_hashes = populate_proposers(&epoch_pool, 10, Some(&transaction));

            let identity_and_document_pairs = setup_identities_with_share_documents(
                &drive,
                &contract,
                &proposer_tx_hashes,
                Some(&transaction),
            );

            drive
                .apply_current_batch(false, Some(&transaction))
                .expect("to apply a batch");

            let genesis_time = Utc::now();

            // process blocks
            for day in 1..=20 {
                let block_time = if day == 1 {
                    genesis_time
                } else {
                    genesis_time + Duration::days(day - 1)
                };

                let previous_block_time = if day == 1 {
                    None
                } else {
                    Some((genesis_time + Duration::days(day - 2)).timestamp_millis())
                };

                let block_height = day as u64;

                // Processing block #1
                let block_begin_request = super::BlockBeginRequest {
                    block_height,
                    block_time: block_time.timestamp_millis(),
                    previous_block_time,
                    proposer_pro_tx_hash: proposer_tx_hashes[0],
                };

                super::block_begin(&drive, block_begin_request, Some(&transaction))
                    .expect(format!("to begin process block #{}", day).as_str());

                let block_end_request = super::BlockEndRequest {
                    fees: Fees {
                        processing_fees: 1600,
                        storage_fees: 42000,
                        fee_multiplier: 1,
                    },
                };

                super::block_end(&drive, block_end_request, Some(&transaction))
                    .expect(format!("to begin process block #{}", day).as_str());
            }

            let storage_fee_pool_value = fee_pools
                .storage_fee_distribution_pool
                .value(&drive, Some(&transaction))
                .expect("to get storage fee pool");

            assert_eq!(storage_fee_pool_value, 0);
        }
    }
}
