use crate::platform::Platform;
use rs_drive::grovedb::TransactionArg;
use rs_drive::drive::batch::GroveDbOpBatch;
use rs_drive::fee_pools::epochs::Epoch;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use crate::error::Error;
use crate::error::execution::ExecutionError;

pub struct DistributionInfo {
    pub masternodes_paid_count: u16,
    pub paid_epoch_index: Option<u16>,
    pub fee_leftovers: Decimal,
}

impl DistributionInfo {
    pub fn empty() -> Self {
        DistributionInfo {
            masternodes_paid_count: 0,
            paid_epoch_index: None,
            fee_leftovers: dec!(0.0),
        }
    }
}

impl Platform {
    pub fn add_distribute_fees_from_unpaid_pools_to_proposers_operations(
        &self,
        current_epoch_index: u16,
        transaction: TransactionArg,
        batch: &mut GroveDbOpBatch,
    ) -> Result<DistributionInfo, Error> {
        if current_epoch_index == 0 {
            return Ok(DistributionInfo::empty());
        }

        // For current epochs we pay for previous
        // Find oldest unpaid epochs since previous epochs
        let unpaid_epoch_pool =
            match self.drive.get_oldest_unpaid_epoch_pool(current_epoch_index - 1, transaction).map_err(Error::Drive)? {
                Some(epoch_pool) => epoch_pool,
                None => return Ok(DistributionInfo::empty()),
            };

        // Process more proposers at once if we have many unpaid epochs in past
        let proposers_limit: u16 = if unpaid_epoch_pool.index == current_epoch_index {
            50
        } else {
            (current_epoch_index - unpaid_epoch_pool.index) * 50
        };

        let total_fees =
            self.drive.get_epoch_total_credits_for_distribution(&unpaid_epoch_pool, transaction).map_err(Error::Drive)?;

        let unpaid_epoch_block_count =
            self.drive.get_epoch_block_count(&unpaid_epoch_pool, transaction).map_err(Error::Drive)?;

        let unpaid_epoch_block_count = Decimal::from(unpaid_epoch_block_count);

        let proposers =
            self.drive.get_epoch_proposers(&unpaid_epoch_pool, proposers_limit, transaction).map_err(Error::Drive)?;

        let proposers_len = proposers.len() as u16;

        let mut fee_leftovers = dec!(0.0);

        for (proposer_tx_hash, proposed_block_count) in proposers.iter() {
            let proposed_block_count = Decimal::from(*proposed_block_count);

            let mut masternode_reward =
                (Decimal::from(total_fees) * proposed_block_count) / unpaid_epoch_block_count;

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

                //todo this shouldn't be a percentage
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

                let share_percentage = Decimal::new(share_percentage_integer, 0) / dec!(10000.0);

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

            // Since balance is an integer, we collect rewards remainder and distribute leftovers afterwards
            let masternode_reward_floored = masternode_reward.floor();

            fee_leftovers += masternode_reward - masternode_reward_floored;

            self.add_pay_reward_to_identity_operations(
                proposer_tx_hash,
                masternode_reward_floored,
                transaction,
                batch,
            )?;
        }

        // remove proposers we've paid out
        let proposer_pro_tx_hashes: Vec<Vec<u8>> =
            proposers.iter().map(|(hash, _)| hash.clone()).collect();

        unpaid_epoch_pool.add_delete_proposers_operations(
            proposer_pro_tx_hashes,
            batch,
        );

        // if less then a limit processed then mark the epochs pool as paid
        if proposers_len < proposers_limit {
            unpaid_epoch_pool.add_mark_as_paid_operations(batch);
        }

        Ok(DistributionInfo {
            masternodes_paid_count: proposers_len,
            paid_epoch_index: Some(unpaid_epoch_pool.index),
            fee_leftovers,
        })
    }

    fn add_pay_reward_to_identity_operations(
        &self,
        id: &[u8],
        reward: Decimal,
        transaction: TransactionArg,
        batch: &mut GroveDbOpBatch,
    ) -> Result<(), Error> {
        // Convert to integer, since identity balance is u64
        let reward: u64 = reward.try_into().map_err(|_| {
            Error::Execution(ExecutionError::Overflow(
                "can't convert reward to i64 from Decimal",
            ))
        })?;

        // We don't need additional verification, since we ensure an identity
        // existence in the data contract triggers in DPP
        let (mut identity, storage_flags) = self.drive.fetch_identity(id, transaction)?;

        //todo balance should be a u64
        identity.balance += reward as i64;

        self.drive.add_insert_identity_operations(identity, storage_flags, batch).map_err(Error::Drive)
    }

    pub fn add_distribute_fees_into_pools_operations(
        &self,
        current_epoch_pool: &Epoch,
        processing_fees: u64,
        storage_fees: u64,
        transaction: TransactionArg,
        batch: &mut GroveDbOpBatch,
    ) -> Result<(), Error> {
        // update epochs pool processing fees
        let epoch_processing_fees = self
            .drive
            .get_epoch_processing_credits_for_distribution(current_epoch_pool, transaction)?;

        batch.push(
            current_epoch_pool.update_processing_credits_for_distribution_operation(
                epoch_processing_fees + processing_fees,
            ),
        );

        // update storage fee pool
        let storage_fee_pool =
            self.drive.get_aggregate_storage_fees_in_current_distribution_pool(transaction)?;

        batch.push(
            current_epoch_pool
                .update_storage_credits_for_distribution_operation(storage_fee_pool + storage_fees),
        );

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use rs_drive::drive::batch::GroveDbOpBatch;
    use rs_drive::fee_pools::epochs::Epoch;
    use crate::common::helpers::setup::setup_platform_with_initial_state_structure;


    mod distribute_fees_from_unpaid_pools_to_proposers {
        use rs_drive::common::helpers::identities::create_test_masternode_identities_and_add_them_as_epoch_block_proposers;
        use crate::common::helpers::setup;
        use rs_drive::drive::batch::GroveDbOpBatch;
        use rs_drive::fee_pools::epochs::epoch_key_constants::KEY_PROPOSERS;
        use rs_drive::fee_pools::epochs::Epoch;
        use rs_drive::grovedb;
        use crate::common::helpers::fee_pools::{create_test_masternode_share_identities_and_documents, refetch_identities};
        use crate::common::helpers::setup::setup_platform_with_initial_state_structure;

        #[test]
        fn test_no_distribution_on_epoch_0() {
            let platform = setup::setup_platform_with_initial_state_structure();
            let transaction = platform.drive.grove.start_transaction();

            let current_epoch_index = 0;

            let mut batch = GroveDbOpBatch::new();

            let distribution_info = platform
                .add_distribute_fees_from_unpaid_pools_to_proposers_operations(
                    current_epoch_index,
                    Some(&transaction),
                    &mut batch,
                )
                .expect("should distribute fees");

            assert_eq!(distribution_info.masternodes_paid_count, 0);
            assert!(distribution_info.paid_epoch_index.is_none());
        }

        #[test]
        fn test_no_distribution_when_all_epochs_paid() {
            let platform = setup::setup_platform_with_initial_state_structure();
            let transaction = platform.drive.grove.start_transaction();

            let current_epoch_index = 1;

            let mut batch = GroveDbOpBatch::new();

            let distribution_info = platform
                .add_distribute_fees_from_unpaid_pools_to_proposers_operations(
                    current_epoch_index,
                    Some(&transaction),
                    &mut batch,
                )
                .expect("should distribute fees");

            assert_eq!(distribution_info.masternodes_paid_count, 0);
            assert!(distribution_info.paid_epoch_index.is_none());
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

            let mut batch = GroveDbOpBatch::new();

            unpaid_epoch_pool_0.add_init_current_operations(1.0, 1, 1, &mut batch);

            let unpaid_epoch_pool_0_proposers_count = 200;

            unpaid_epoch_pool_1.add_init_current_operations(
                1.0,
                unpaid_epoch_pool_0_proposers_count as u64 + 1,
                2,
                &mut batch,
            );

            platform.drive
                .grove_apply_batch(batch, false, Some(&transaction))
                .expect("should apply batch");

            create_test_masternode_identities_and_add_them_as_epoch_block_proposers(
                &platform.drive,
                &unpaid_epoch_pool_0,
                unpaid_epoch_pool_0_proposers_count,
                Some(&transaction),
            );

            create_test_masternode_identities_and_add_them_as_epoch_block_proposers(
                &platform.drive,
                &unpaid_epoch_pool_1,
                200,
                Some(&transaction),
            );

            let mut batch = GroveDbOpBatch::new();

            platform
                .add_distribute_fees_into_pools_operations(
                    &unpaid_epoch_pool_0,
                    10000,
                    10000,
                    Some(&transaction),
                    &mut batch,
                )
                .expect("distribute fees into epochs pool 0");

            platform.drive
                .grove_apply_batch(batch, false, Some(&transaction))
                .expect("should apply batch");

            let mut batch = GroveDbOpBatch::new();

            let distribution_info = platform
                .add_distribute_fees_from_unpaid_pools_to_proposers_operations(
                    2,
                    Some(&transaction),
                    &mut batch,
                )
                .expect("should distribute fees");

            platform.drive
                .grove_apply_batch(batch, false, Some(&transaction))
                .expect("should apply batch");

            assert_eq!(distribution_info.masternodes_paid_count, 100);
            assert_eq!(distribution_info.paid_epoch_index.unwrap(), 0);
        }

        #[test]
        fn test_partial_distribution() {
            let platform = setup_platform_with_initial_state_structure();
            let transaction = platform.drive.grove.start_transaction();

            // Create masternode reward shares contract
            let contract = platform.create_mn_shares_contract(Some(&transaction));

            let unpaid_epoch_pool = Epoch::new(0);
            let next_epoch_pool = Epoch::new(1);

            let mut batch = GroveDbOpBatch::new();

            unpaid_epoch_pool.add_init_current_operations(1.0, 1, 1, &mut batch);

            // emulating epochs change
            next_epoch_pool.add_init_current_operations(1.0, 11, 10, &mut batch);

            platform.drive
                .grove_apply_batch(batch, false, Some(&transaction))
                .expect("should apply batch");

            let pro_tx_hashes = create_test_masternode_identities_and_add_them_as_epoch_block_proposers(
                &platform.drive,
                &unpaid_epoch_pool,
                60,
                Some(&transaction),
            );

            create_test_masternode_share_identities_and_documents(
                &platform.drive,
                &contract,
                &pro_tx_hashes,
                Some(&transaction),
            );

            let mut batch = GroveDbOpBatch::new();

            platform
                .add_distribute_fees_into_pools_operations(
                    &unpaid_epoch_pool,
                    10000,
                    10000,
                    Some(&transaction),
                    &mut batch,
                )
                .expect("distribute fees into epochs pool");

            platform.drive
                .grove_apply_batch(batch, false, Some(&transaction))
                .expect("should apply batch");

            let mut batch = GroveDbOpBatch::new();

            let distribution_info = platform
                .add_distribute_fees_from_unpaid_pools_to_proposers_operations(
                    1,
                    Some(&transaction),
                    &mut batch,
                )
                .expect("should distribute fees");

            platform.drive
                .grove_apply_batch(batch, false, Some(&transaction))
                .expect("should apply batch");

            assert_eq!(distribution_info.masternodes_paid_count, 50);
            assert_eq!(distribution_info.paid_epoch_index.unwrap(), 0);

            // expect unpaid proposers exist
            match platform.drive.is_epochs_proposers_tree_empty(&unpaid_epoch_pool, Some(&transaction)) {
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

            let unpaid_epoch_pool = Epoch::new(0);
            let next_epoch_pool = Epoch::new(1);

            let mut batch = GroveDbOpBatch::new();

            unpaid_epoch_pool.add_init_current_operations(1.0, 1, 1, &mut batch);

            // emulating epochs change
            next_epoch_pool.add_init_current_operations(1.0, 11, 10, &mut batch);

            platform.drive
                .grove_apply_batch(batch, false, Some(&transaction))
                .expect("should apply batch");

            let pro_tx_hashes = create_test_masternode_identities_and_add_them_as_epoch_block_proposers(
                &platform.drive,
                &unpaid_epoch_pool,
                10,
                Some(&transaction),
            );

            let share_identities_and_documents = create_test_masternode_share_identities_and_documents(
                &platform.drive,
                &contract,
                &pro_tx_hashes,
                Some(&transaction),
            );

            let mut batch = GroveDbOpBatch::new();

            platform
                .add_distribute_fees_into_pools_operations(
                    &unpaid_epoch_pool,
                    10000,
                    10000,
                    Some(&transaction),
                    &mut batch,
                )
                .expect("distribute fees into epochs pool");

            platform.drive
                .grove_apply_batch(batch, false, Some(&transaction))
                .expect("should apply batch");

            let mut batch = GroveDbOpBatch::new();

            let distribution_info = platform
                .add_distribute_fees_from_unpaid_pools_to_proposers_operations(
                    1,
                    Some(&transaction),
                    &mut batch,
                )
                .expect("should distribute fees");

            platform.drive
                .grove_apply_batch(batch, false, Some(&transaction))
                .expect("should apply batch");

            assert_eq!(distribution_info.masternodes_paid_count, 10);
            assert_eq!(distribution_info.paid_epoch_index.unwrap(), 0);

            // check we paid 500 to every mn identity
            let paid_mn_identities =
                platform.drive.fetch_identities(&pro_tx_hashes, Some(&transaction)).expect("expected to get identities");

            for paid_mn_identity in paid_mn_identities {
                assert_eq!(paid_mn_identity.balance, 500);
            }

            let share_identities = share_identities_and_documents
                .iter()
                .map(|(identity, _)| identity)
                .collect();

            let refetched_share_identities =
                refetch_identities(&platform.drive, share_identities, Some(&transaction)).expect("expected to refresh identities");

            for identity in refetched_share_identities {
                assert_eq!(identity.balance, 500);
            }

            // check we've removed proposers tree
            match platform.drive
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

    #[test]
    fn test_distribute_fees_into_pools() {
        let platform = setup_platform_with_initial_state_structure();
        let transaction = platform.drive.grove.start_transaction();

        let current_epoch_pool = Epoch::new(0);

        let mut batch = GroveDbOpBatch::new();

        current_epoch_pool.add_init_current_operations(1.0, 1, 1, &mut batch);

        // Apply new pool structure
        platform.drive
            .grove_apply_batch(batch, false, Some(&transaction))
            .expect("should apply batch");

        let mut batch = GroveDbOpBatch::new();

        let processing_fees = 1000000;
        let storage_fees = 2000000;

        platform
            .add_distribute_fees_into_pools_operations(
                &current_epoch_pool,
                processing_fees,
                storage_fees,
                Some(&transaction),
                &mut batch,
            )
            .expect("should distribute fees into pools");

        platform.drive
            .grove_apply_batch(batch, false, Some(&transaction))
            .expect("should apply batch");

        let stored_processing_fee_credits = platform.drive
            .get_epoch_processing_credits_for_distribution(&current_epoch_pool, Some(&transaction))
            .expect("should get processing fees");

        let stored_storage_fee_credits = platform.drive
            .get_epoch_storage_credits_for_distribution(&current_epoch_pool, Some(&transaction))
            .expect("should get storage fee pool");

        assert_eq!(stored_processing_fee_credits, processing_fees);
        assert_eq!(stored_storage_fee_credits, storage_fees);
    }
}