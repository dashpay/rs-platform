use crate::abci::messages::FeesAggregate;
use crate::error::execution::ExecutionError;
use crate::error::Error;

use crate::platform::Platform;
use rs_drive::drive::batch::GroveDbOpBatch;
use rs_drive::error::fee::FeeError;
use rs_drive::fee_pools::epochs::Epoch;
use rs_drive::fee_pools::update_storage_fee_distribution_pool_operation;
use rs_drive::grovedb::TransactionArg;
use rs_drive::{error, grovedb};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;

pub struct ProposerPayouts {
    pub proposers_paid_count: u16,
    pub paid_epoch_index: u16,
}

pub struct FeesInPools {
    pub processing_fees: u64,
    pub storage_fees: u64,
}

pub struct UnpaidEpoch {
    epoch_index: u16,
    start_block_height: u64,
    end_block_height: u64,
    proposers_limit: u16,
}

impl UnpaidEpoch {
    fn block_count(&self) -> Result<u64, error::Error> {
        self.end_block_height
            .checked_sub(self.start_block_height)
            .ok_or(error::Error::Fee(FeeError::Overflow(
                "overflow for get_epoch_block_count",
            )))
    }
}

impl Platform {
    pub fn get_unpaid_epoch(
        &self,
        current_epoch_index: u16,
        cached_current_epoch_start_block_height: Option<u64>,
        transaction: TransactionArg,
    ) -> Result<Option<UnpaidEpoch>, Error> {
        let unpaid_epoch_index = self.drive.get_epoch_index_to_pay(transaction)?;

        if unpaid_epoch_index < current_epoch_index {
            return Ok(None);
        }

        // Process more proposers at once if we have many unpaid epochs in past
        let proposers_limit: u16 = if unpaid_epoch_index == current_epoch_index {
            // TODO We never visit this branch?
            50
        } else {
            (current_epoch_index - unpaid_epoch_index + 1) * 50
        };

        let current_start_block_height =
            self.get_epoch_start_block_height(unpaid_epoch_pool, transaction)?;

        // Pass cached current epoch start block height only if we pay for the previous epoch
        let unpaid_epoch_start_block_height = if cached_current_epoch_start_block_height.is_some()
            && unpaid_epoch_pool.index == current_epoch_index - 1
        {
            cached_current_epoch_start_block_height.unwrap();
        } else {
            let (_, start_block_height) = self.find_next_epoch_stat_block_height(
                unpaid_epoch_pool.index,
                current_epoch_index,
                transaction,
            )?.ok_or((FeeError::CorruptedCodeExecution("start_block_height must be present for current epoch or cached_next_epoch_start_block_height must be passed")))?;

            start_block_height
        };

        let next_start_block_height =
            if let Some(next_start_block_height) = cached_next_epoch_start_block_height {
                next_start_block_height
            } else {
            };

        let block_count = next_start_block_height
            .checked_sub(unpaid_epoch_start_block_height)
            .ok_or(error::Error::Fee(FeeError::Overflow(
                "overflow for get_epoch_block_count",
            )))?;

        UnpaidEpoch {
            epoch_index: unpaid_epoch_index,
            proposers_limit,
        }
    }

    pub fn add_distribute_fees_from_unpaid_pools_to_proposers_operations(
        &self,
        unpaid_epoch_info: &UnpaidEpoch,
        transaction: TransactionArg,
        batch: &mut GroveDbOpBatch,
    ) -> Result<Option<ProposerPayouts>, Error> {
        let unpaid_epoch_pool = Epoch::new(epoch_index_to_pay);

        let total_fees = self
            .drive
            .get_epoch_total_credits_for_distribution(&unpaid_epoch_pool, transaction)
            .map_err(Error::Drive)?;

        let total_fees = Decimal::from(total_fees);

        // Calculate block count

        let unpaid_epoch_block_count = self
            .drive
            .get_epoch_block_count(
                &unpaid_epoch_pool,
                current_epoch_index,
                cached_current_epoch_start_block_height,
                transaction,
            )
            .map_err(Error::Drive)?;

        let unpaid_epoch_block_count = Decimal::from(unpaid_epoch_block_count);

        let proposers = self
            .drive
            .get_epoch_proposers(&unpaid_epoch_pool, proposers_limit, transaction)
            .map_err(Error::Drive)?;

        let proposers_len = proposers.len() as u16;

        let mut fee_leftovers = dec!(0.0);

        for (i, (proposer_tx_hash, proposed_block_count)) in proposers.iter().enumerate() {
            let i = i as u16;
            let proposed_block_count = Decimal::from(*proposed_block_count);

            let mut masternode_reward =
                (total_fees * proposed_block_count) / unpaid_epoch_block_count;

            let documents =
                self.get_reward_shares_list_for_masternode(proposer_tx_hash, transaction)?;

            for document in documents {
                let pay_to_id = document
                    .properties
                    .get("payToId")
                    .ok_or(Error::Execution(ExecutionError::DriveMissingData(
                        "payToId property is missing",
                    )))?
                    .as_bytes()
                    .ok_or(Error::Execution(ExecutionError::DriveIncoherence(
                        "payToId property type is not bytes",
                    )))?;

                // TODO this shouldn't be a percentage
                //  Answer: it's already percentage but converted to integer with bigger precision like in ProRegTx
                let share_percentage_integer: i64 = document
                    .properties
                    .get("percentage")
                    .ok_or(Error::Execution(ExecutionError::DriveMissingData(
                        "percentage property is missing",
                    )))?
                    .as_integer()
                    .ok_or(Error::Execution(ExecutionError::DriveIncoherence(
                        "percentage property type is not integer",
                    )))?
                    .try_into()
                    .map_err(|_| {
                        Error::Execution(ExecutionError::Overflow(
                            "percentage property cannot be converted to i64",
                        ))
                    })?;

                let share_percentage = Decimal::from(share_percentage_integer) / dec!(10000);

                let reward = masternode_reward * share_percentage;

                let reward_floored = reward.floor();

                // update masternode reward that would be paid later
                masternode_reward -= reward_floored;

                self.add_pay_reward_to_identity_operations(
                    pay_to_id,
                    reward_floored,
                    transaction,
                    batch,
                )?;
            }

            // Since balance is an integer, we collect rewards remainder
            // and add leftovers to the latest proposer of the chunk
            let masternode_reward_floored = masternode_reward.floor();

            fee_leftovers += masternode_reward - masternode_reward_floored;

            let masternode_reward_given = if i == proposers_len - 1 {
                masternode_reward_floored + fee_leftovers
            } else {
                masternode_reward_floored
            };

            self.add_pay_reward_to_identity_operations(
                proposer_tx_hash,
                masternode_reward_given,
                transaction,
                batch,
            )?;
        }

        // remove proposers we've paid out
        let proposer_pro_tx_hashes: Vec<Vec<u8>> =
            proposers.iter().map(|(hash, _)| hash.clone()).collect();

        unpaid_epoch_pool.add_delete_proposers_operations(proposer_pro_tx_hashes, batch);

        // if less then a limit processed then mark the epochs pool as paid
        if proposers_len < proposers_limit {
            // TODO It must be called in upper function. It's not this function
            //   responsibility to remove some keys from epoch pools, it deals only with proposers tree
            unpaid_epoch_pool.add_mark_as_paid_operations(batch);

            // Update
            self.drive.find_next_epoch_stat_block_height(
                unpaid_epoch_pool.index,
                current_epoch_index,
                transaction,
            )
        }

        Ok(Some(ProposerPayouts {
            proposers_paid_count: proposers_len,
            paid_epoch_index: unpaid_epoch_pool.index,
        }))
    }

    fn add_pay_reward_to_identity_operations(
        &self,
        id: &[u8],
        reward: Decimal,
        transaction: TransactionArg,
        batch: &mut GroveDbOpBatch,
    ) -> Result<(), Error> {
        // Convert to integer, since identity balance is u64
        let reward: u64 = reward.floor().try_into().map_err(|_| {
            Error::Execution(ExecutionError::Overflow(
                "can't convert reward to i64 from Decimal",
            ))
        })?;

        // We don't need additional verification, since we ensure an identity
        // existence in the data contract triggers in DPP
        let (mut identity, storage_flags) = self.drive.fetch_identity(id, transaction)?;

        //todo balance should be a u64
        identity.balance += reward as i64;

        self.drive
            .add_insert_identity_operations(identity, storage_flags, batch)
            .map_err(Error::Drive)
    }

    pub fn add_distribute_block_fees_into_pools_operations(
        &self,
        current_epoch: &Epoch,
        block_fees: &FeesAggregate,
        cached_aggregated_storage_fees: Option<u64>,
        transaction: TransactionArg,
        batch: &mut GroveDbOpBatch,
    ) -> Result<FeesInPools, Error> {
        // update epochs pool processing fees
        let epoch_processing_fees = self
            .drive
            .get_epoch_processing_credits_for_distribution(current_epoch, transaction)
            .or_else(|e| match e {
                // Handle epoch change when storage fees are not set yet
                error::Error::GroveDB(grovedb::Error::PathKeyNotFound(_)) => Ok(0u64),
                _ => Err(e),
            })?;

        let total_processing_fees = epoch_processing_fees + block_fees.processing_fees;

        batch.push(
            current_epoch
                // TODO Why update processing fees in Epoch but get function in Drive?
                .update_processing_credits_for_distribution_operation(total_processing_fees),
        );

        // update storage fee pool
        let storage_distribution_credits_in_fee_pool = match cached_aggregated_storage_fees {
            None => self
                .drive
                .get_aggregate_storage_fees_in_current_distribution_pool(transaction)?,
            Some(storage_fees) => storage_fees,
        };

        let total_storage_fees = storage_distribution_credits_in_fee_pool + block_fees.storage_fees;

        batch.push(update_storage_fee_distribution_pool_operation(
            storage_distribution_credits_in_fee_pool + block_fees.storage_fees,
        ));

        Ok(FeesInPools {
            processing_fees: total_processing_fees,
            storage_fees: total_storage_fees,
        })
    }
}

#[cfg(test)]
mod tests {
    mod add_distribute_fees_from_unpaid_pools_to_proposers {
        use crate::abci::messages::FeesAggregate;
        use crate::common::helpers::fee_pools::{
            create_test_masternode_share_identities_and_documents, refetch_identities,
        };
        use crate::common::helpers::setup::setup_platform_with_initial_state_structure;
        use rs_drive::common::helpers::identities::create_test_masternode_identities_and_add_them_as_epoch_block_proposers;
        use rs_drive::drive::batch::GroveDbOpBatch;
        use rs_drive::fee_pools::epochs::epoch_key_constants::KEY_PROPOSERS;
        use rs_drive::fee_pools::epochs::Epoch;
        use rs_drive::grovedb;
        use rust_decimal::Decimal;
        use rust_decimal_macros::dec;

        #[test]
        fn test_no_distribution_when_all_epochs_paid() {
            let platform = setup_platform_with_initial_state_structure();
            let transaction = platform.drive.grove.start_transaction();

            let pay_starting_with_epoch_index = 0;

            let mut batch = GroveDbOpBatch::new();

            let proposers_payouts = platform
                .add_distribute_fees_from_unpaid_pools_to_proposers_operations(
                    pay_starting_with_epoch_index,
                    None,
                    Some(&transaction),
                    &mut batch,
                )
                .expect("should distribute fees");

            assert!(proposers_payouts.is_none());
        }

        #[test]
        fn test_increased_proposers_limit_for_two_unpaid_epochs() {
            let platform = setup_platform_with_initial_state_structure();
            let transaction = platform.drive.grove.start_transaction();

            // Create masternode reward shares contract
            platform.create_mn_shares_contract(Some(&transaction));

            // Create epochs

            let unpaid_epoch_pool_0 = Epoch::new(0);
            let unpaid_epoch_pool_1 = Epoch::new(1);
            let epoch_pool_2 = Epoch::new(2);

            let mut batch = GroveDbOpBatch::new();

            unpaid_epoch_pool_0.add_init_current_operations(1.0, 1, 1, &mut batch);

            batch.push(
                unpaid_epoch_pool_0.update_processing_credits_for_distribution_operation(10000),
            );

            let proposers_count = 100u16;

            unpaid_epoch_pool_1.add_init_current_operations(
                1.0,
                proposers_count as u64 + 1,
                2,
                &mut batch,
            );

            unpaid_epoch_pool_1.add_init_current_operations(
                1.0,
                proposers_count as u64 * 2 + 1,
                3,
                &mut batch,
            );

            epoch_pool_2.add_init_current_operations(
                1.0,
                proposers_count as u64 * 3 + 1,
                3,
                &mut batch,
            );

            platform
                .drive
                .grove_apply_batch(batch, false, Some(&transaction))
                .expect("should apply batch");

            create_test_masternode_identities_and_add_them_as_epoch_block_proposers(
                &platform.drive,
                &unpaid_epoch_pool_0,
                proposers_count,
                Some(&transaction),
            );

            create_test_masternode_identities_and_add_them_as_epoch_block_proposers(
                &platform.drive,
                &unpaid_epoch_pool_1,
                proposers_count,
                Some(&transaction),
            );

            let mut batch = GroveDbOpBatch::new();

            let pay_starting_with_epoch_index = 1;
            let cached_current_epoch_start_block_height = None;

            let proposer_payouts = platform
                .add_distribute_fees_from_unpaid_pools_to_proposers_operations(
                    pay_starting_with_epoch_index,
                    cached_current_epoch_start_block_height,
                    Some(&transaction),
                    &mut batch,
                )
                .expect("should distribute fees");

            platform
                .drive
                .grove_apply_batch(batch, false, Some(&transaction))
                .expect("should apply batch");

            match proposer_payouts {
                None => assert!(false, "proposers should be paid"),
                Some(payouts) => {
                    assert_eq!(payouts.proposers_paid_count, 100);
                    assert_eq!(payouts.paid_epoch_index, 0);
                }
            }
        }

        #[test]
        fn test_payouts_for_previous_epoch_without_start_block_height_committed() {
            let platform = setup_platform_with_initial_state_structure();
            let transaction = platform.drive.grove.start_transaction();

            // Create masternode reward shares contract
            let contract = platform.create_mn_shares_contract(Some(&transaction));

            let unpaid_epoch_pool = Epoch::new(0);
            let current_epoch_pool = Epoch::new(1);

            let proposers_count = 50;

            let mut batch = GroveDbOpBatch::new();

            unpaid_epoch_pool.add_init_current_operations(1.0, 1, 1, &mut batch);

            // Add some fees to unpaid epoch pool
            platform
                .add_distribute_block_fees_into_pools_operations(
                    &unpaid_epoch_pool,
                    &FeesAggregate {
                        processing_fees: 10000,
                        storage_fees: 0,
                        refunds_by_epoch: vec![],
                    },
                    None,
                    Some(&transaction),
                    &mut batch,
                )
                .expect("distribute fees into epochs pool");

            platform
                .drive
                .grove_apply_batch(batch, false, Some(&transaction))
                .expect("should apply batch");

            // Populate proposers into unpaid epoch pool

            let pro_tx_hashes =
                create_test_masternode_identities_and_add_them_as_epoch_block_proposers(
                    &platform.drive,
                    &unpaid_epoch_pool,
                    proposers_count,
                    Some(&transaction),
                );

            create_test_masternode_share_identities_and_documents(
                &platform.drive,
                &contract,
                &pro_tx_hashes,
                Some(&transaction),
            );

            // emulating epochs change
            let mut batch = GroveDbOpBatch::new();

            let start_block_height = proposers_count as u64 + 1;

            current_epoch_pool.add_init_current_operations(1.0, start_block_height, 2, &mut batch);

            let pay_starting_with_epoch_index = 0;
            let cached_current_epoch_start_block_height = Some(start_block_height);

            let proposer_payouts = platform
                .add_distribute_fees_from_unpaid_pools_to_proposers_operations(
                    pay_starting_with_epoch_index,
                    cached_current_epoch_start_block_height,
                    Some(&transaction),
                    &mut batch,
                )
                .expect("should distribute fees");

            platform
                .drive
                .grove_apply_batch(batch, false, Some(&transaction))
                .expect("should apply batch");

            match proposer_payouts {
                None => assert!(false, "proposers should be paid"),
                Some(payouts) => {
                    assert_eq!(payouts.proposers_paid_count, 50);
                    assert_eq!(payouts.paid_epoch_index, 0);
                }
            }
        }

        #[test]
        fn test_payouts_for_two_epochs_ago_without_start_block_height_committed() {
            let platform = setup_platform_with_initial_state_structure();
            let transaction = platform.drive.grove.start_transaction();

            // Create masternode reward shares contract
            let contract = platform.create_mn_shares_contract(Some(&transaction));

            let unpaid_epoch_pool_0 = Epoch::new(0);
            let unpaid_epoch_pool_1 = Epoch::new(1);
            let epoch_pool_2 = Epoch::new(2);

            let proposers_count = 50;

            let mut batch = GroveDbOpBatch::new();

            unpaid_epoch_pool_0.add_init_current_operations(1.0, 1, 1, &mut batch);

            batch.push(
                unpaid_epoch_pool_0.update_processing_credits_for_distribution_operation(10000),
            );

            unpaid_epoch_pool_1.add_init_current_operations(
                1.0,
                proposers_count as u64 + 1,
                2,
                &mut batch,
            );

            platform
                .drive
                .grove_apply_batch(batch, false, Some(&transaction))
                .expect("should apply batch");

            // Populate proposers into unpaid epoch pools

            let pro_tx_hashes =
                create_test_masternode_identities_and_add_them_as_epoch_block_proposers(
                    &platform.drive,
                    &unpaid_epoch_pool_0,
                    proposers_count,
                    Some(&transaction),
                );

            create_test_masternode_share_identities_and_documents(
                &platform.drive,
                &contract,
                &pro_tx_hashes,
                Some(&transaction),
            );

            let pro_tx_hashes =
                create_test_masternode_identities_and_add_them_as_epoch_block_proposers(
                    &platform.drive,
                    &unpaid_epoch_pool_1,
                    proposers_count,
                    Some(&transaction),
                );

            create_test_masternode_share_identities_and_documents(
                &platform.drive,
                &contract,
                &pro_tx_hashes,
                Some(&transaction),
            );

            // emulating epochs change
            let mut batch = GroveDbOpBatch::new();

            let epoch_pool_2_start_block_height = proposers_count as u64 * 2 + 1;

            epoch_pool_2.add_init_current_operations(
                1.0,
                epoch_pool_2_start_block_height,
                2,
                &mut batch,
            );

            let pay_starting_with_epoch_index = 1;
            let cached_current_epoch_start_block_height = Some(epoch_pool_2_start_block_height);

            let proposer_payouts = platform
                .add_distribute_fees_from_unpaid_pools_to_proposers_operations(
                    pay_starting_with_epoch_index,
                    cached_current_epoch_start_block_height,
                    Some(&transaction),
                    &mut batch,
                )
                .expect("should distribute fees");

            platform
                .drive
                .grove_apply_batch(batch, false, Some(&transaction))
                .expect("should apply batch");

            match proposer_payouts {
                None => assert!(false, "proposers should be paid"),
                Some(payouts) => {
                    assert_eq!(payouts.paid_epoch_index, 0);
                    assert_eq!(payouts.proposers_paid_count, 50);
                }
            }
        }

        #[test]
        fn test_partial_distribution() {
            let platform = setup_platform_with_initial_state_structure();
            let transaction = platform.drive.grove.start_transaction();

            // Create masternode reward shares contract
            let contract = platform.create_mn_shares_contract(Some(&transaction));

            let unpaid_epoch_pool = Epoch::new(0);
            let current_epoch_pool = Epoch::new(1);

            let proposers_count = 60;

            let mut batch = GroveDbOpBatch::new();

            unpaid_epoch_pool.add_init_current_operations(1.0, 1, 1, &mut batch);

            batch.push(
                unpaid_epoch_pool.update_processing_credits_for_distribution_operation(10000),
            );

            // emulating epochs change
            current_epoch_pool.add_init_current_operations(
                1.0,
                proposers_count as u64 + 1,
                2,
                &mut batch,
            );

            platform
                .drive
                .grove_apply_batch(batch, false, Some(&transaction))
                .expect("should apply batch");

            let pro_tx_hashes =
                create_test_masternode_identities_and_add_them_as_epoch_block_proposers(
                    &platform.drive,
                    &unpaid_epoch_pool,
                    proposers_count,
                    Some(&transaction),
                );

            create_test_masternode_share_identities_and_documents(
                &platform.drive,
                &contract,
                &pro_tx_hashes,
                Some(&transaction),
            );

            let mut batch = GroveDbOpBatch::new();

            let pay_starting_with_epoch_index = 0;
            let cached_current_epoch_start_block_height = None;

            let proposer_payouts = platform
                .add_distribute_fees_from_unpaid_pools_to_proposers_operations(
                    pay_starting_with_epoch_index,
                    cached_current_epoch_start_block_height,
                    Some(&transaction),
                    &mut batch,
                )
                .expect("should distribute fees");

            platform
                .drive
                .grove_apply_batch(batch, false, Some(&transaction))
                .expect("should apply batch");

            match proposer_payouts {
                None => assert!(false, "proposers should be paid"),
                Some(payouts) => {
                    assert_eq!(payouts.proposers_paid_count, 50);
                    assert_eq!(payouts.paid_epoch_index, 0);
                }
            }

            // expect unpaid proposers exist
            match platform
                .drive
                .is_epochs_proposers_tree_empty(&unpaid_epoch_pool, Some(&transaction))
            {
                Ok(is_empty) => assert!(!is_empty),
                Err(e) => match e {
                    _ => assert!(false, "should be able to get proposers tree"),
                },
            }
        }

        #[test]
        fn test_complete_distribution() {
            let platform = setup_platform_with_initial_state_structure();
            let transaction = platform.drive.grove.start_transaction();

            // Create masternode reward shares contract
            let contract = platform.create_mn_shares_contract(Some(&transaction));

            let proposers_count = 10;
            let processing_fees = 10000;
            let storage_fees = 10000;

            let unpaid_epoch_pool = Epoch::new(0);
            let next_epoch_pool = Epoch::new(1);

            let mut batch = GroveDbOpBatch::new();

            unpaid_epoch_pool.add_init_current_operations(1.0, 1, 1, &mut batch);

            batch.push(
                unpaid_epoch_pool
                    .update_processing_credits_for_distribution_operation(processing_fees),
            );

            batch.push(
                unpaid_epoch_pool.update_storage_credits_for_distribution_operation(storage_fees),
            );

            next_epoch_pool.add_init_current_operations(1.0, 11, 10, &mut batch);

            platform
                .drive
                .grove_apply_batch(batch, false, Some(&transaction))
                .expect("should apply batch");

            let pro_tx_hashes =
                create_test_masternode_identities_and_add_them_as_epoch_block_proposers(
                    &platform.drive,
                    &unpaid_epoch_pool,
                    proposers_count,
                    Some(&transaction),
                );

            let share_identities_and_documents =
                create_test_masternode_share_identities_and_documents(
                    &platform.drive,
                    &contract,
                    &pro_tx_hashes,
                    Some(&transaction),
                );

            let mut batch = GroveDbOpBatch::new();

            let pay_starting_with_epoch_index = 0;
            let cached_current_epoch_start_block_height = None;

            let proposer_payouts = platform
                .add_distribute_fees_from_unpaid_pools_to_proposers_operations(
                    pay_starting_with_epoch_index,
                    cached_current_epoch_start_block_height,
                    Some(&transaction),
                    &mut batch,
                )
                .expect("should distribute fees");

            platform
                .drive
                .grove_apply_batch(batch, false, Some(&transaction))
                .expect("should apply batch");

            match proposer_payouts {
                None => assert!(false, "proposers should be paid"),
                Some(payouts) => {
                    assert_eq!(payouts.proposers_paid_count, 10);
                    assert_eq!(payouts.paid_epoch_index, 0);
                }
            }

            // check we paid 500 to every mn identity
            let paid_mn_identities = platform
                .drive
                .fetch_identities(&pro_tx_hashes, Some(&transaction))
                .expect("expected to get identities");

            let total_fees = Decimal::from(storage_fees + processing_fees);

            let masternode_reward = total_fees / Decimal::from(proposers_count);

            let shares_percentage_with_precision: u64 = share_identities_and_documents[0]
                .1
                .properties
                .get("percentage")
                .expect("should have percentage field")
                .as_integer()
                .expect("percentage should an integer")
                .try_into()
                .expect("percentage should be u64");

            let shares_percentage = Decimal::from(shares_percentage_with_precision) / dec!(10000);

            let payout_credits = masternode_reward * shares_percentage;

            let payout_credits: i64 = payout_credits.try_into().expect("should convert to i64");

            for paid_mn_identity in paid_mn_identities {
                assert_eq!(paid_mn_identity.balance, payout_credits);
            }

            let share_identities = share_identities_and_documents
                .iter()
                .map(|(identity, _)| identity)
                .collect();

            let refetched_share_identities =
                refetch_identities(&platform.drive, share_identities, Some(&transaction))
                    .expect("expected to refresh identities");

            for identity in refetched_share_identities {
                assert_eq!(identity.balance, payout_credits as i64);
            }

            // check we've removed proposers tree
            match platform
                .drive
                .grove
                .get(
                    unpaid_epoch_pool.get_path(),
                    KEY_PROPOSERS.as_slice(),
                    Some(&transaction),
                )
                .unwrap()
            {
                Ok(_) => assert!(false, "expect tree not exists"),
                Err(e) => match e {
                    grovedb::Error::PathKeyNotFound(_) => assert!(true),
                    _ => assert!(false, "invalid error type"),
                },
            }
        }
    }

    mod add_distribute_block_fees_into_pools_operations {
        use crate::abci::messages::FeesAggregate;
        use crate::common::helpers::setup::setup_platform_with_initial_state_structure;
        use rs_drive::drive::batch::GroveDbOpBatch;
        use rs_drive::fee_pools::epochs::Epoch;

        #[test]
        fn test_distribute_block_fees_into_uncommitted_epoch_on_epoch_change() {
            let platform = setup_platform_with_initial_state_structure();
            let transaction = platform.drive.grove.start_transaction();

            let current_epoch_pool = Epoch::new(1);

            let mut batch = GroveDbOpBatch::new();

            current_epoch_pool.add_init_current_operations(1.0, 1, 1, &mut batch);

            let processing_fees = 1000000;
            let storage_fees = 2000000;

            platform
                .add_distribute_block_fees_into_pools_operations(
                    &current_epoch_pool,
                    &FeesAggregate {
                        processing_fees,
                        storage_fees,
                        refunds_by_epoch: vec![],
                    },
                    None,
                    Some(&transaction),
                    &mut batch,
                )
                .expect("should distribute fees into pools");

            platform
                .drive
                .grove_apply_batch(batch, false, Some(&transaction))
                .expect("should apply batch");

            let stored_processing_fee_credits = platform
                .drive
                .get_epoch_processing_credits_for_distribution(
                    &current_epoch_pool,
                    Some(&transaction),
                )
                .expect("should get processing fees");

            let stored_storage_fee_credits = platform
                .drive
                .get_aggregate_storage_fees_in_current_distribution_pool(Some(&transaction))
                .expect("should get storage fee pool");

            assert_eq!(stored_processing_fee_credits, processing_fees);
            assert_eq!(stored_storage_fee_credits, storage_fees);
        }

        #[test]
        fn test_distribute_block_fees_into_pools() {
            let platform = setup_platform_with_initial_state_structure();
            let transaction = platform.drive.grove.start_transaction();

            let current_epoch_pool = Epoch::new(1);

            let mut batch = GroveDbOpBatch::new();

            current_epoch_pool.add_init_current_operations(1.0, 1, 1, &mut batch);

            // Apply new pool structure
            platform
                .drive
                .grove_apply_batch(batch, false, Some(&transaction))
                .expect("should apply batch");

            let mut batch = GroveDbOpBatch::new();

            let processing_fees = 1000000;
            let storage_fees = 2000000;

            platform
                .add_distribute_block_fees_into_pools_operations(
                    &current_epoch_pool,
                    &FeesAggregate {
                        processing_fees,
                        storage_fees,
                        refunds_by_epoch: vec![],
                    },
                    None,
                    Some(&transaction),
                    &mut batch,
                )
                .expect("should distribute fees into pools");

            platform
                .drive
                .grove_apply_batch(batch, false, Some(&transaction))
                .expect("should apply batch");

            let stored_processing_fee_credits = platform
                .drive
                .get_epoch_processing_credits_for_distribution(
                    &current_epoch_pool,
                    Some(&transaction),
                )
                .expect("should get processing fees");

            let stored_storage_fee_credits = platform
                .drive
                .get_aggregate_storage_fees_in_current_distribution_pool(Some(&transaction))
                .expect("should get storage fee pool");

            assert_eq!(stored_processing_fee_credits, processing_fees);
            assert_eq!(stored_storage_fee_credits, storage_fees);
        }
    }
}
