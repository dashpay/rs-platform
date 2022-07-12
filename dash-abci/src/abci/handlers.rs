use std::ops::Deref;

use crate::abci::messages::{
    BlockBeginRequest, BlockBeginResponse, BlockEndRequest, BlockEndResponse, InitChainRequest,
    InitChainResponse,
};
use crate::block::{BlockExecutionContext, BlockInfo};
use grovedb::TransactionArg;
use rs_drive::fee::epoch::EpochInfo;
use rs_drive::query::GroveError::StorageError;
use rs_drive::query::TransactionArg;

use crate::drive::storage::batch::Batch;
use crate::drive::Drive;
use crate::error;
use crate::error::Error;
use crate::fee::epoch::EpochInfo;
use crate::platform::Platform;

pub trait TenderdashAbci {
    fn init_chain(
        &self,
        request: InitChainRequest,
        transaction: TransactionArg,
    ) -> Result<InitChainResponse, Error>;

    fn block_begin(
        &self,
        request: BlockBeginRequest,
        transaction: TransactionArg,
    ) -> Result<BlockBeginResponse, Error>;

    fn block_end(
        &self,
        request: BlockEndRequest,
        transaction: TransactionArg,
    ) -> Result<BlockEndResponse, Error>;
}

impl TenderdashAbci for Platform {
    fn init_chain(
        &self,
        _request: InitChainRequest,
        transaction: TransactionArg,
    ) -> Result<InitChainResponse, Error> {
        self.drive
            .create_initial_state_structure(transaction)
            .map_err(StorageError)?;

        let response = InitChainResponse {};

        Ok(response)
    }

    fn block_begin(
        &self,
        request: BlockBeginRequest,
        transaction: TransactionArg,
    ) -> Result<BlockBeginResponse, Error> {
        // Set genesis time
        let genesis_time = if request.block_height == 1 {
            self.init_genesis(request.block_time_ms)
        } else {
            drive.get_genesis_time(transaction)?
        };

        // Init block execution context
        let epoch_info = EpochInfo::calculate(
            genesis_time,
            request.block_time_ms,
            request.previous_block_time_ms,
        )?;

        let block_execution_context = BlockExecutionContext {
            block_info: BlockInfo::from_block_begin_request(&request),
            epoch_info,
        };

        self.block_execution_context
            .replace(Some(block_execution_context));

        let response = BlockBeginResponse {};

        Ok(response)
    }

    fn block_end(
        &self,
        request: BlockEndRequest,
        transaction: TransactionArg,
    ) -> Result<BlockEndResponse, Error> {
        // Retrieve block execution context
        let block_execution_context = self.block_execution_context.borrow();
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
        let distribution_info = self.drive.process_block_fees(
            &block_execution_context.block_info,
            &block_execution_context.epoch_info,
            &request.fees,
            transaction,
        )?;

        let response = BlockEndResponse {
            current_epoch_index: block_execution_context.epoch_info.current_epoch_index,
            is_epoch_change: block_execution_context.epoch_info.is_epoch_change,
            masternodes_paid_count: distribution_info.masternodes_paid_count,
            paid_epoch_index: distribution_info.paid_epoch_index,
        };

        Ok(response)
    }
}

#[cfg(test)]
mod tests {
    mod handlers {
        use chrono::{Duration, Utc};
        use rs_drive::common::tests::helpers::fee_pools::{
            create_masternode_identities, create_masternode_share_identities_and_documents,
            create_mn_shares_contract,
        };
        use rs_drive::common::tests::helpers::setup::{setup_drive, setup_fee_pools};
        use std::time::Duration;

        use crate::abci::handlers::{block_begin, block_end, init_chain};
        use crate::abci::messages::{BlockBeginRequest, BlockEndRequest, InitChainRequest};
        use crate::fee::pools::tests::helpers::fee_pools::create_masternode_identities;
        use crate::{
            drive::abci::{
                handlers::{block_begin, block_end, init_chain},
                messages::{BlockBeginRequest, BlockEndRequest, Fees, InitChainRequest},
            },
            fee::pools::tests::helpers::{
                fee_pools::{
                    create_masternode_share_identities_and_documents, create_mn_shares_contract,
                },
                setup::{setup_drive, setup_fee_pools},
            },
        };

        #[test]
        fn test_abci_flow() {
            let drive = setup_drive();
            let (transaction, fee_pools) = setup_fee_pools(&drive, None);

            // init chain
            let init_chain_request = InitChainRequest {};

            init_chain(&drive, init_chain_request, Some(&transaction)).expect("should init chain");

            // setup the contract
            let contract = create_mn_shares_contract(&drive, Some(&transaction));

            let genesis_time = Utc::now();

            let total_days = 22;

            let epoch_1_start_day = 20;

            let proposers_count = total_days;

            let storage_fees_per_block = 42000;

            // and create masternode identities
            let proposers =
                create_masternode_identities(&drive, proposers_count, Some(&transaction));

            create_masternode_share_identities_and_documents(
                &drive,
                &contract,
                &proposers,
                Some(&transaction),
            );

            // process blocks
            for day in 1..=total_days {
                let block_time = if day == 1 {
                    genesis_time
                } else {
                    genesis_time + Duration::days(day as i64 - 1)
                };

                let previous_block_time = if day == 1 {
                    None
                } else {
                    Some((genesis_time + Duration::days(day as i64 - 2)).timestamp_millis())
                };

                let block_height = day as u64;

                // Processing block
                let block_begin_request = BlockBeginRequest {
                    block_height,
                    block_time_ms: block_time.timestamp_millis(),
                    previous_block_time_ms: previous_block_time,
                    proposer_pro_tx_hash: proposers[day as usize - 1],
                };

                block_begin(&drive, block_begin_request, Some(&transaction))
                    .expect(format!("should begin process block #{}", day).as_str());

                let block_end_request = BlockEndRequest {
                    fees: Fees {
                        processing_fees: 1600,
                        storage_fees: storage_fees_per_block,
                        fee_multiplier: 2,
                    },
                };

                let block_end_response = block_end(&drive, block_end_request, Some(&transaction))
                    .expect(format!("should end process block #{}", day).as_str());

                // Should calculate correct current epochs
                let epoch_index = if day >= epoch_1_start_day { 1 } else { 0 };

                assert_eq!(block_end_response.current_epoch_index, epoch_index);

                assert_eq!(
                    block_end_response.is_epoch_change,
                    previous_block_time.is_none() || day == epoch_1_start_day
                );

                // Should pay to 19 masternodes, when epochs 1 started
                let masternodes_paid_count = if day == epoch_1_start_day {
                    day as u16 - 1
                } else {
                    0
                };

                assert_eq!(
                    block_end_response.masternodes_paid_count,
                    masternodes_paid_count
                );

                // Should pay for the epochs 0, when epochs 1 started
                match block_end_response.paid_epoch_index {
                    Some(index) => assert_eq!(
                        index, 0,
                        "should pay to masternodes only when epochs 1 started"
                    ),
                    None => assert_ne!(
                        day, epoch_1_start_day,
                        "should pay to masternodes only when epochs 1 started"
                    ),
                }
            }

            let storage_fee_pool_value = fee_pools
                .get_storage_fee_distribution_pool_fees(&drive, Some(&transaction))
                .expect("should get storage fee pool");

            assert_eq!(
                storage_fee_pool_value,
                storage_fees_per_block * (total_days - epoch_1_start_day + 1) as i64,
                "should contain only storage fees from the last block"
            );
        }
    }
}
