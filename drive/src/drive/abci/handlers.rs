use crate::drive::abci::messages::{
    BlockBeginRequest, BlockBeginResponse, BlockEndRequest, BlockEndResponse, InitChainRequest,
    InitChainResponse,
};
use std::ops::Deref;

use crate::drive::block::BlockExecutionContext;
use crate::drive::block::BlockInfo;
use grovedb::TransactionArg;

use crate::drive::storage::batch::Batch;
use crate::drive::Drive;
use crate::error;
use crate::error::Error;
use crate::fee::epoch::EpochInfo;

pub fn init_chain(
    drive: &Drive,
    _request: InitChainRequest,
    transaction: TransactionArg,
) -> Result<InitChainResponse, Error> {
    drive.apply_initial_state_structure(transaction)?;

    let response = InitChainResponse {};

    Ok(response)
}

pub fn block_begin(
    drive: &Drive,
    request: BlockBeginRequest,
    transaction: TransactionArg,
) -> Result<BlockBeginResponse, Error> {
    // Set genesis time
    let genesis_time = if request.block_height == 1 {
        let mut batch = Batch::new(drive);

        drive.update_genesis_time(&mut batch, request.block_time)?;

        batch.apply(false, transaction)?;

        request.block_time
    } else {
        drive.get_genesis_time(transaction)?
    };

    // Init block execution context
    let epoch_info = EpochInfo::calculate(
        genesis_time,
        request.block_time,
        request.previous_block_time,
    )?;

    let block_execution_context = BlockExecutionContext {
        block_info: BlockInfo::from_block_begin_request(&request),
        epoch_info,
        genesis_time,
    };

    drive
        .block_execution_context
        .replace(Some(block_execution_context));

    let response = BlockBeginResponse {};

    Ok(response)
}

pub fn block_end(
    drive: &Drive,
    request: BlockEndRequest,
    transaction: TransactionArg,
) -> Result<BlockEndResponse, Error> {
    // Retrieve block execution context
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

    // Process fees
    let (masternodes_paid_count, paid_epoch_index) = drive.fee_pools.borrow().process_block_fees(
        drive,
        &block_execution_context.block_info,
        &block_execution_context.epoch_info,
        &request.fees,
        transaction,
    )?;

    let response = BlockEndResponse {
        epoch_info: block_execution_context.epoch_info.clone(),
        masternodes_paid_count,
        paid_epoch_index,
    };

    Ok(response)
}

#[cfg(test)]
mod tests {
    mod handlers {
        use chrono::{Duration, Utc};
        use rand::prelude::SliceRandom;

        use crate::drive::storage::batch::Batch;
        use crate::{
            drive::abci::{
                handlers::{block_begin, block_end, init_chain},
                messages::{BlockBeginRequest, BlockEndRequest, Fees, InitChainRequest},
            },
            fee::pools::{
                epoch::epoch_pool::EpochPool,
                tests::helpers::{
                    fee_pools::{
                        create_mn_shares_contract, populate_proposers,
                        setup_identities_with_share_documents,
                    },
                    setup::{setup_drive, setup_fee_pools},
                },
            },
        };

        #[test]
        fn test_abci_flow() {
            let drive = setup_drive();
            let (transaction, fee_pools) = setup_fee_pools(&drive, None);

            // init chain
            let init_chain_request = InitChainRequest {};

            init_chain(&drive, init_chain_request, Some(&transaction)).expect("to init chain");

            // setup the contract
            let contract = create_mn_shares_contract(&drive, Some(&transaction));

            // setup proposers and mn share documents
            let epoch_pool = EpochPool::new(0, &drive);

            let mut batch = Batch::new(&drive);

            epoch_pool
                .init_proposers(&mut batch)
                .expect("to init proposers");

            batch
                .apply(false, Some(&transaction))
                .expect("should apply a batch");

            let proposer_tx_hashes = populate_proposers(&epoch_pool, 2, Some(&transaction));

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

                // Processing block
                let block_begin_request = BlockBeginRequest {
                    block_height,
                    block_time: block_time.timestamp_millis(),
                    previous_block_time,
                    proposer_pro_tx_hash: *proposer_tx_hashes
                        .choose(&mut rand::thread_rng())
                        .unwrap(),
                };

                block_begin(&drive, block_begin_request, Some(&transaction))
                    .expect(format!("to begin process block #{}", day).as_str());

                let block_end_request = BlockEndRequest {
                    fees: Fees {
                        processing_fees: 1600,
                        storage_fees: 42000,
                        fee_multiplier: 1,
                    },
                };

                let block_end_response = block_end(&drive, block_end_request, Some(&transaction))
                    .expect(format!("to begin process block #{}", day).as_str());

                let epoch_index = if day >= 20 { 1 } else { 0 };

                assert_eq!(
                    block_end_response.epoch_info.current_epoch_index,
                    epoch_index
                );

                assert_eq!(
                    block_end_response.epoch_info.is_epoch_change,
                    previous_block_time.is_none() || day == 20
                );

                let masternodes_paid_count = if day == 20 { 2 } else { 0 };

                assert_eq!(
                    block_end_response.masternodes_paid_count,
                    masternodes_paid_count
                );
            }

            let storage_fee_pool_value = fee_pools
                .storage_fee_distribution_pool
                .value(&drive, Some(&transaction))
                .expect("to get storage fee pool");

            assert_eq!(
                storage_fee_pool_value, 42000,
                "should contain only storage fees from the last block"
            );
        }
    }
}
