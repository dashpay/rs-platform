use grovedb::TransactionArg;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde_json::json;

use crate::common::value_to_cbor;
use crate::contract::document::Document;
use crate::drive::batch::GroveDbOpBatch;
use crate::drive::fee_pools::constants;
use crate::drive::Drive;
use crate::error::document::DocumentError;
use crate::error::fee::FeeError;
use crate::error::Error;
use crate::fee_pools::epochs::EpochPool;

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

impl Drive {
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
            match self.get_oldest_unpaid_epoch_pool(current_epoch_index - 1, transaction)? {
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
            self.get_epoch_total_credits_for_distribution(&unpaid_epoch_pool, transaction)?;

        let unpaid_epoch_block_count =
            self.get_epoch_block_count(&unpaid_epoch_pool, transaction)?;

        let unpaid_epoch_block_count = Decimal::from(unpaid_epoch_block_count);

        let proposers =
            self.get_epochs_proposers(&unpaid_epoch_pool, proposers_limit, transaction)?;

        let proposers_len = proposers.len() as u16;

        let mut fee_leftovers = dec!(0.0);

        for (proposer_tx_hash, proposed_block_count) in proposers.iter() {
            let proposed_block_count = Decimal::from(*proposed_block_count);

            let mut masternode_reward =
                (total_fees * proposed_block_count) / unpaid_epoch_block_count;

            let documents = self.get_reward_shares(proposer_tx_hash, transaction)?;

            for document in documents {
                let pay_to_id = document
                    .properties
                    .get("payToId")
                    .ok_or(Error::Document(DocumentError::MissingDocumentProperty(
                        "payToId property is missing",
                    )))?
                    .as_bytes()
                    .ok_or(Error::Document(DocumentError::InvalidDocumentPropertyType(
                        "payToId property type is not bytes",
                    )))?;

                let share_percentage_integer: i64 = document
                    .properties
                    .get("percentage")
                    .ok_or(Error::Document(DocumentError::MissingDocumentProperty(
                        "percentage property is missing",
                    )))?
                    .as_integer()
                    .ok_or(Error::Document(DocumentError::InvalidDocumentPropertyType(
                        "percentage property type is not integer",
                    )))?
                    .try_into()
                    .map_err(|_| {
                        Error::Document(DocumentError::InvalidDocumentPropertyType(
                            "percentage property cannot be converted to i64",
                        ))
                    })?;

                let share_percentage = Decimal::new(share_percentage_integer, 0) / dec!(10000.0);

                let reward = masternode_reward * share_percentage;

                let reward_floored = reward.floor();

                // update masternode reward that would be paid later
                masternode_reward -= reward_floored;

                Self::add_pay_reward_to_identity_operations(
                    drive,
                    pay_to_id,
                    reward_floored,
                    transaction,
                    batch,
                )?;
            }

            // Since balance is an integer, we collect rewards remainder and distribute leftovers afterwards
            let masternode_reward_floored = masternode_reward.floor();

            fee_leftovers += masternode_reward - masternode_reward_floored;

            Self::add_pay_reward_to_identity_operations(
                drive,
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
            batch,
            proposer_pro_tx_hashes,
            transaction,
        )?;

        // if less then a limit processed then mark the epochs pool as paid
        if proposers_len < proposers_limit {
            unpaid_epoch_pool.add_mark_as_paid_operations(batch, transaction)?;
        }

        Ok(DistributionInfo {
            masternodes_paid_count: proposers_len,
            paid_epoch_index: Some(unpaid_epoch_pool.index),
            fee_leftovers,
        })
    }

    fn add_pay_reward_to_identity_operations(
        drive: &Drive,
        id: &[u8],
        reward: Decimal,
        transaction: TransactionArg,
        batch: &mut GroveDbOpBatch,
    ) -> Result<(), Error> {
        // Convert to integer, since identity balance is u64
        let reward: u64 = reward.try_into().map_err(|_| {
            Error::Fee(FeeError::DecimalConversion(
                "can't convert reward to i64 from Decimal",
            ))
        })?;

        // We don't need additional verification, since we ensure an identity
        // existence in the data contract triggers in DPP
        let (mut identity, storage_flags) = drive.fetch_identity(id, transaction)?;

        //todo balance should be a u64
        identity.balance += reward as i64;

        drive.add_insert_identity_operations(identity, storage_flags, batch)
    }

    fn get_reward_shares(
        masternode_owner_id: &Vec<u8>,
        transaction: TransactionArg,
    ) -> Result<Vec<Document>, Error> {
        let query_json = json!({
            "where": [
                ["$ownerId", "==", bs58::encode(masternode_owner_id).into_string()]
            ],
        });

        let query_cbor = value_to_cbor(query_json, None);

        let (document_cbors, _, _) = drive.query_documents(
            &query_cbor,
            constants::MN_REWARD_SHARES_CONTRACT_ID,
            constants::MN_REWARD_SHARES_DOCUMENT_TYPE,
            transaction,
        )?;

        document_cbors
            .iter()
            .map(|cbor| Document::from_cbor(cbor, None, None))
            .collect::<Result<Vec<Document>, Error>>()
    }

    fn get_oldest_unpaid_epoch_pool<'d>(
        &'d self,
        from_epoch_index: u16,
        transaction: TransactionArg,
    ) -> Result<Option<EpochPool>, Error> {
        self.get_oldest_unpaid_epoch_pool_recursive(from_epoch_index, from_epoch_index, transaction)
    }

    fn get_oldest_unpaid_epoch_pool_recursive<'d>(
        &'d self,
        from_epoch_index: u16,
        epoch_index: u16,
        transaction: TransactionArg,
    ) -> Result<Option<EpochPool>, Error> {
        let epoch_pool = EpochPool::new(epoch_index);

        if self.is_epochs_proposers_tree_empty(&epoch_pool, transaction)? {
            return if epoch_index == from_epoch_index {
                Ok(None)
            } else {
                let unpaid_epoch_pool = EpochPool::new(epoch_index + 1);

                Ok(Some(unpaid_epoch_pool))
            };
        }

        if epoch_index == 0 {
            return Ok(Some(epoch_pool));
        }

        self.get_oldest_unpaid_epoch_pool_recursive(from_epoch_index, epoch_index - 1, transaction)
    }

    pub fn add_distribute_fees_into_pools_operations(
        &self,
        current_epoch_pool: &EpochPool,
        processing_fees: u64,
        storage_fees: u64,
        transaction: TransactionArg,
        batch: &mut GroveDbOpBatch,
    ) -> Result<(), Error> {
        // update epochs pool processing fees
        let epoch_processing_fees =
            self.get_epoch_processing_credits_for_distribution(current_epoch_pool, transaction)?;

        batch.push(
            current_epoch_pool.update_processing_credits_for_distribution_operation(
                epoch_processing_fees + processing_fees,
            ),
        );

        // update storage fee pool
        let storage_fee_pool =
            self.get_aggregate_storage_fees_in_current_distribution_pool(transaction)?;

        batch.push(
            current_epoch_pool
                .update_storage_credits_for_distribution_operation(storage_fee_pool + storage_fees),
        );

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::common::tests::helpers::setup::{setup_drive, setup_fee_pools};
    use crate::fee::pools::{
        epoch::constants,
        epoch::epoch_pool::EpochPool,
        tests::helpers::{
            fee_pools::{
                create_masternode_identities_and_increment_proposers,
                create_masternode_share_identities_and_documents, create_mn_shares_contract,
                fetch_identities_by_pro_tx_hashes, refetch_identities,
            },
            setup::{setup_drive, setup_fee_pools},
        },
    };

    use crate::drive::storage::batch::GroveDbOpBatch;
    use crate::fee_pools::epochs::EpochPool;

    mod get_oldest_unpaid_epoch_pool {
        #[test]
        fn test_all_epochs_paid() {
            let drive = super::setup_drive();
            let (transaction, fee_pools) = super::setup_fee_pools(&drive, None);

            match fee_pools
                .get_oldest_unpaid_epoch_pool(&drive, 999, Some(&transaction))
                .expect("should get oldest epochs pool")
            {
                Some(_) => assert!(false, "shouldn't return any unpaid epochs"),
                None => assert!(true),
            }
        }

        #[test]
        fn test_two_unpaid_epochs() {
            let drive = super::setup_drive();
            let (transaction, fee_pools) = super::setup_fee_pools(&drive, None);

            let unpaid_epoch_pool_0 = super::EpochPool::new(0);

            let mut batch = super::GroveDbOpBatch::new(&drive);

            batch.push(unpaid_epoch_pool_0.init_proposers_tree_operation());

            // Apply proposers tree
            drive
                .grove_apply_batch(batch, false, Some(&transaction))
                .expect("should apply batch");

            super::create_masternode_identities_and_increment_proposers(
                &drive,
                &unpaid_epoch_pool_0,
                2,
                Some(&transaction),
            );

            let unpaid_epoch_pool_1 = super::EpochPool::new(1);

            let mut batch = super::GroveDbOpBatch::new();

            batch.push(unpaid_epoch_pool_1.init_proposers_tree_operation());

            // Apply proposers tree
            drive
                .grove_apply_batch(batch, false, Some(&transaction))
                .expect("should apply batch");

            super::create_masternode_identities_and_increment_proposers(
                &drive,
                &unpaid_epoch_pool_1,
                2,
                Some(&transaction),
            );

            match fee_pools
                .get_oldest_unpaid_epoch_pool(&drive, 1, Some(&transaction))
                .expect("should get oldest epochs pool")
            {
                Some(epoch_pool) => assert_eq!(epoch_pool.index, 0),
                None => assert!(false, "should have unpaid epochs"),
            }
        }
    }

    mod distribute_fees_from_unpaid_pools_to_proposers {
        #[test]
        fn test_no_distribution_on_epoch_0() {
            let drive = super::setup_drive();
            let (transaction, fee_pools) = super::setup_fee_pools(&drive, None);

            let current_epoch_index = 0;

            let mut batch = super::GroveDbOpBatch::new(&drive);

            let distribution_info = fee_pools
                .add_distribute_fees_from_unpaid_pools_to_proposers_operations(
                    &drive,
                    &mut batch,
                    current_epoch_index,
                    Some(&transaction),
                )
                .expect("should distribute fees");

            assert_eq!(distribution_info.masternodes_paid_count, 0);
            assert!(distribution_info.paid_epoch_index.is_none());
        }

        #[test]
        fn test_no_distribution_when_all_epochs_paid() {
            let drive = super::setup_drive();
            let (transaction, fee_pools) = super::setup_fee_pools(&drive, None);

            let current_epoch_index = 1;

            let mut batch = super::GroveDbOpBatch::new(&drive);

            let distribution_info = fee_pools
                .add_distribute_fees_from_unpaid_pools_to_proposers_operations(
                    &drive,
                    &mut batch,
                    current_epoch_index,
                    Some(&transaction),
                )
                .expect("should distribute fees");

            assert_eq!(distribution_info.masternodes_paid_count, 0);
            assert!(distribution_info.paid_epoch_index.is_none());
        }

        #[test]
        fn test_increased_proposers_limit_for_two_unpaid_epochs() {
            let drive = super::setup_drive();
            let (transaction, fee_pools) = super::setup_fee_pools(&drive, None);

            // Create masternode reward shares contract
            super::create_mn_shares_contract(&drive, Some(&transaction));

            // Create epochs

            let unpaid_epoch_pool_0 = super::EpochPool::new(0);
            let unpaid_epoch_pool_1 = super::EpochPool::new(1);

            let mut batch = super::GroveDbOpBatch::new(&drive);

            unpaid_epoch_pool_0.add_init_current_operations(1, 1, 1, &mut batch);

            let unpaid_epoch_pool_0_proposers_count = 200;

            unpaid_epoch_pool_1.add_init_current_operations(
                1,
                unpaid_epoch_pool_0_proposers_count as u64 + 1,
                2,
                &mut batch,
            );

            drive
                .grove_apply_batch(batch, false, Some(&transaction))
                .expect("should apply batch");

            super::create_masternode_identities_and_increment_proposers(
                &drive,
                &unpaid_epoch_pool_0,
                unpaid_epoch_pool_0_proposers_count,
                Some(&transaction),
            );

            super::create_masternode_identities_and_increment_proposers(
                &drive,
                &unpaid_epoch_pool_1,
                200,
                Some(&transaction),
            );

            let mut batch = super::GroveDbOpBatch::new(&drive);

            fee_pools
                .add_distribute_fees_into_pools_operations(
                    &drive,
                    &mut batch,
                    &unpaid_epoch_pool_0,
                    10000,
                    10000,
                    Some(&transaction),
                )
                .expect("distribute fees into epochs pool 0");

            drive
                .grove_apply_batch(batch, false, Some(&transaction))
                .expect("should apply batch");

            let mut batch = super::GroveDbOpBatch::new(&drive);

            let distribution_info = fee_pools
                .add_distribute_fees_from_unpaid_pools_to_proposers_operations(
                    &drive,
                    &mut batch,
                    2,
                    Some(&transaction),
                )
                .expect("should distribute fees");

            drive
                .grove_apply_batch(batch, false, Some(&transaction))
                .expect("should apply batch");

            assert_eq!(distribution_info.masternodes_paid_count, 100);
            assert_eq!(distribution_info.paid_epoch_index.unwrap(), 0);
        }

        #[test]
        fn test_partial_distribution() {
            let drive = super::setup_drive();
            let (transaction, fee_pools) = super::setup_fee_pools(&drive, None);

            // Create masternode reward shares contract
            let contract = super::create_mn_shares_contract(&drive, Some(&transaction));

            let unpaid_epoch_pool = super::EpochPool::new(0);
            let next_epoch_pool = super::EpochPool::new(1);

            let mut batch = super::GroveDbOpBatch::new(&drive);

            unpaid_epoch_pool.add_init_current_operations(1, 1, 1, &mut batch);

            // emulating epochs change
            next_epoch_pool.add_init_current_operations(1, 11, 10, &mut batch);

            drive
                .grove_apply_batch(batch, false, Some(&transaction))
                .expect("should apply batch");

            let pro_tx_hashes = super::create_masternode_identities_and_increment_proposers(
                &drive,
                &unpaid_epoch_pool,
                60,
                Some(&transaction),
            );

            super::create_masternode_share_identities_and_documents(
                &drive,
                &contract,
                &pro_tx_hashes,
                Some(&transaction),
            );

            let mut batch = super::GroveDbOpBatch::new(&drive);

            fee_pools
                .add_distribute_fees_into_pools_operations(
                    &drive,
                    &mut batch,
                    &unpaid_epoch_pool,
                    10000,
                    10000,
                    Some(&transaction),
                )
                .expect("distribute fees into epochs pool");

            drive
                .grove_apply_batch(batch, false, Some(&transaction))
                .expect("should apply batch");

            let mut batch = super::GroveDbOpBatch::new(&drive);

            let distribution_info = fee_pools
                .add_distribute_fees_from_unpaid_pools_to_proposers_operations(
                    &drive,
                    &mut batch,
                    1,
                    Some(&transaction),
                )
                .expect("should distribute fees");

            drive
                .grove_apply_batch(batch, false, Some(&transaction))
                .expect("should apply batch");

            assert_eq!(distribution_info.masternodes_paid_count, 50);
            assert_eq!(distribution_info.paid_epoch_index.unwrap(), 0);

            // expect unpaid proposers exist
            match drive.is_epochs_proposers_tree_empty(&unpaid_epoch_pool, Some(&transaction)) {
                Ok(is_empty) => assert!(!is_empty),
                Err(e) => match e {
                    _ => assert!(false, "should be able to get proposers tree"),
                },
            }
        }

        #[test]
        fn test_complete_distribution() {
            let drive = super::setup_drive();
            let (transaction, fee_pools) = super::setup_fee_pools(&drive, None);

            // Create masternode reward shares contract
            let contract = super::create_mn_shares_contract(&drive, Some(&transaction));

            let unpaid_epoch_pool = super::EpochPool::new(0);
            let next_epoch_pool = super::EpochPool::new(1);

            let mut batch = super::GroveDbOpBatch::new(&drive);

            unpaid_epoch_pool.add_init_current_operations(1, 1, 1, &mut batch);

            // emulating epochs change
            next_epoch_pool.add_init_current_operations(1, 11, 10, &mut batch);

            drive
                .grove_apply_batch(batch, false, Some(&transaction))
                .expect("should apply batch");

            let pro_tx_hashes = super::create_masternode_identities_and_increment_proposers(
                &drive,
                &unpaid_epoch_pool,
                10,
                Some(&transaction),
            );

            let share_identities_and_documents =
                super::create_masternode_share_identities_and_documents(
                    &drive,
                    &contract,
                    &pro_tx_hashes,
                    Some(&transaction),
                );

            let mut batch = super::GroveDbOpBatch::new(&drive);

            fee_pools
                .add_distribute_fees_into_pools_operations(
                    &drive,
                    &mut batch,
                    &unpaid_epoch_pool,
                    10000,
                    10000,
                    Some(&transaction),
                )
                .expect("distribute fees into epochs pool");

            drive
                .grove_apply_batch(batch, false, Some(&transaction))
                .expect("should apply batch");

            let mut batch = super::GroveDbOpBatch::new(&drive);

            let distribution_info = fee_pools
                .add_distribute_fees_from_unpaid_pools_to_proposers_operations(
                    &drive,
                    &mut batch,
                    1,
                    Some(&transaction),
                )
                .expect("should distribute fees");

            drive
                .grove_apply_batch(batch, false, Some(&transaction))
                .expect("should apply batch");

            assert_eq!(distribution_info.masternodes_paid_count, 10);
            assert_eq!(distribution_info.paid_epoch_index.unwrap(), 0);

            // check we paid 500 to every mn identity
            let paid_mn_identities = super::fetch_identities_by_pro_tx_hashes(
                &drive,
                &pro_tx_hashes,
                Some(&transaction),
            );

            for paid_mn_identity in paid_mn_identities {
                assert_eq!(paid_mn_identity.balance, 500);
            }

            let share_identities = share_identities_and_documents
                .iter()
                .map(|(identity, _)| identity)
                .collect();

            let refetched_share_identities =
                super::refetch_identities(&drive, share_identities, Some(&transaction));

            for identity in refetched_share_identities {
                assert_eq!(identity.balance, 500);
            }

            // check we've removed proposers tree
            match drive
                .grove
                .get(
                    unpaid_epoch_pool.get_path(),
                    super::constants::KEY_PROPOSERS.as_slice(),
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
        let drive = setup_drive();
        let (transaction, fee_pools) = setup_fee_pools(&drive, None);

        let current_epoch_pool = EpochPool::new(0);

        let mut batch = super::GroveDbOpBatch::new();

        current_epoch_pool.add_init_current_operations(1, 1, 1, &mut batch);

        // Apply new pool structure
        drive
            .grove_apply_batch(batch, false, Some(&transaction))
            .expect("should apply batch");

        let mut batch = super::GroveDbOpBatch::new();

        let processing_fees = 1000000;
        let storage_fees = 2000000;

        fee_pools
            .add_distribute_fees_into_pools_operations(
                &drive,
                &mut batch,
                &current_epoch_pool,
                processing_fees,
                storage_fees,
                Some(&transaction),
            )
            .expect("should distribute fees into pools");

        drive
            .grove_apply_batch(batch, false, Some(&transaction))
            .expect("should apply batch");

        let stored_processing_fee_credits = drive
            .get_epoch_processing_credits_for_distribution(&current_epoch_pool, Some(&transaction))
            .expect("should get processing fees");

        let stored_storage_fee_credits = drive
            .get_epoch_storage_credits_for_distribution(&current_epoch_pool, Some(&transaction))
            .expect("should get storage fee pool");

        assert_eq!(stored_processing_fee_credits, processing_fees);
        assert_eq!(stored_storage_fee_credits, storage_fees);
    }
}
