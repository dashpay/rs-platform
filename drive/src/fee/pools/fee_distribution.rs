use grovedb::TransactionArg;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde_json::json;
use std::ops::DerefMut;

use crate::common::value_to_cbor;
use crate::contract::document::Document;
use crate::drive::Drive;
use crate::error::document::DocumentError;
use crate::error::fee::FeeError;
use crate::error::Error;
use crate::fee::pools::fee_pools::FeePools;

use crate::fee::pools::constants;
use crate::fee::pools::epoch::epoch_pool::EpochPool;

impl FeePools {
    pub fn distribute_fees_from_unpaid_pools_to_proposers(
        &self,
        drive: &Drive,
        current_epoch_index: u16,
        transaction: TransactionArg,
    ) -> Result<u16, Error> {
        if current_epoch_index == 0 {
            return Ok(0);
        }

        // For current epoch we pay for previous
        // Find oldest unpaid epoch since previous epoch
        let unpaid_epoch_pool = match self.get_oldest_unpaid_epoch_pool(
            &drive,
            current_epoch_index - 1,
            transaction,
        )? {
            Some(epoch_pool) => epoch_pool,
            None => return Ok(0),
        };

        // Process more proposers at once if we have many unpaid epochs in past
        let proposers_limit: u16 = if unpaid_epoch_pool.index == current_epoch_index {
            50
        } else {
            (current_epoch_index - unpaid_epoch_pool.index) * 50
        };

        let total_fees = unpaid_epoch_pool.get_total_fees(transaction)?;

        let unpaid_epoch_block_count =
            Self::get_epoch_block_count(&drive, &unpaid_epoch_pool, transaction)?;

        let unpaid_epoch_block_count = Decimal::from(unpaid_epoch_block_count);

        let proposers = unpaid_epoch_pool.get_proposers(proposers_limit, transaction)?;

        let proposers_len = proposers.len() as u16;

        let mut fee_leftovers = dec!(0.0);

        for (proposer_tx_hash, proposed_block_count) in proposers.iter() {
            let proposed_block_count = Decimal::from(*proposed_block_count);

            let mut masternode_reward =
                (total_fees * proposed_block_count) / unpaid_epoch_block_count;

            let documents = Self::get_reward_shares(drive, &proposer_tx_hash, transaction)?;

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

                Self::pay_reward_to_identity(drive, pay_to_id, reward_floored, transaction)?;
            }

            // Since balance is an integer, we collect rewards remainder and distribute leftovers afterwards
            let masternode_reward_floored = masternode_reward.floor();

            fee_leftovers += masternode_reward - masternode_reward_floored;

            Self::pay_reward_to_identity(
                drive,
                &proposer_tx_hash,
                masternode_reward_floored,
                transaction,
            )?;
        }

        // remove proposers we've paid out
        let proposer_pro_tx_hashes: Vec<Vec<u8>> =
            proposers.iter().map(|(hash, _)| hash.clone()).collect();

        unpaid_epoch_pool.delete_proposers(proposer_pro_tx_hashes, transaction)?;

        // Move integer part of the leftovers to processing
        // and fractional part to storage fees for the next epoch
        let next_epoch_pool = EpochPool::new(unpaid_epoch_pool.index + 1, drive);

        Self::move_leftovers_to_the_next_epoch_pool(next_epoch_pool, fee_leftovers, transaction)?;

        // if less then a limit processed then mark the epoch pool as paid
        if proposers_len < proposers_limit {
            unpaid_epoch_pool.mark_as_paid(transaction)?;
        }

        Ok(proposers_len)
    }

    fn move_leftovers_to_the_next_epoch_pool(
        next_epoch_pool: EpochPool,
        fee_leftovers: Decimal,
        transaction: TransactionArg,
    ) -> Result<(), Error> {
        let storage_leftovers = fee_leftovers.fract();
        let processing_leftovers: u64 = (fee_leftovers.floor()).try_into().map_err(|_| {
            Error::Fee(FeeError::DecimalConversion(
                "can't convert fee_leftovers to u64 from Decimal",
            ))
        })?;

        let processing_fee = next_epoch_pool.get_processing_fee(transaction)?;

        next_epoch_pool.update_processing_fee(processing_fee + processing_leftovers)?;

        let storage_fee = next_epoch_pool.get_storage_fee(transaction)?;

        next_epoch_pool.update_storage_fee(storage_fee + storage_leftovers)?;

        Ok(())
    }

    fn pay_reward_to_identity(
        drive: &Drive,
        id: &Vec<u8>,
        reward: Decimal,
        transaction: TransactionArg,
    ) -> Result<(), Error> {
        // Convert to integer, since identity balance is u64
        let reward: i64 = reward.try_into().map_err(|_| {
            Error::Fee(FeeError::DecimalConversion(
                "can't convert reward to i64 from Decimal",
            ))
        })?;

        // We don't need additional verification, since we ensure an identity
        // existence in the data contract triggers in DPP
        let mut identity = drive.fetch_identity(id, transaction)?;

        identity.balance += reward;

        drive.insert_identity_operations(identity, drive.current_batch.borrow_mut().deref_mut())?;

        Ok(())
    }

    fn get_reward_shares(
        drive: &Drive,
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
            .map(|cbor| Ok(Document::from_cbor(cbor, None, None)?))
            .collect::<Result<Vec<Document>, Error>>()
    }

    fn get_oldest_unpaid_epoch_pool<'d>(
        &'d self,
        drive: &'d Drive,
        from_epoch_index: u16,
        transaction: TransactionArg,
    ) -> Result<Option<EpochPool>, Error> {
        self.get_oldest_unpaid_epoch_pool_recursive(
            &drive,
            from_epoch_index,
            from_epoch_index,
            transaction,
        )
    }

    fn get_oldest_unpaid_epoch_pool_recursive<'d>(
        &'d self,
        drive: &'d Drive,
        from_epoch_index: u16,
        epoch_index: u16,
        transaction: TransactionArg,
    ) -> Result<Option<EpochPool>, Error> {
        let epoch_pool = EpochPool::new(epoch_index, drive);

        if epoch_pool.is_proposers_tree_empty(transaction)? {
            return if epoch_index == from_epoch_index {
                Ok(None)
            } else {
                let unpaid_epoch_pool = EpochPool::new(epoch_index + 1, drive);

                Ok(Some(unpaid_epoch_pool))
            };
        }

        if epoch_index == 0 {
            return Ok(Some(epoch_pool));
        }

        self.get_oldest_unpaid_epoch_pool_recursive(
            &drive,
            from_epoch_index,
            epoch_index - 1,
            transaction,
        )
    }

    fn get_epoch_block_count(
        drive: &Drive,
        epoch_pool: &EpochPool,
        transaction: TransactionArg,
    ) -> Result<u64, Error> {
        let next_epoch_pool = EpochPool::new(epoch_pool.index + 1, drive);

        let block_count = next_epoch_pool.get_start_block_height(transaction)?
            - epoch_pool.get_start_block_height(transaction)?;

        Ok(block_count)
    }

    pub fn distribute_fees_into_pools(
        &self,
        drive: &Drive,
        current_epoch_pool: &EpochPool,
        processing_fees: u64,
        storage_fees: i64,
        transaction: TransactionArg,
    ) -> Result<(), Error> {
        // update epoch pool processing fees
        let epoch_processing_fees = current_epoch_pool.get_processing_fee(transaction)?;

        current_epoch_pool.update_processing_fee(epoch_processing_fees + processing_fees)?;

        // update storage fee pool
        let storage_fee_pool = self
            .storage_fee_distribution_pool
            .value(&drive, transaction)?;

        self.storage_fee_distribution_pool
            .update(&drive, storage_fee_pool + storage_fees)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::fee::pools::tests::helpers::fee_pools::create_mn_shares_contract;
    use crate::fee::pools::tests::helpers::fee_pools::populate_proposers;
    use crate::fee::pools::tests::helpers::setup::setup_drive;
    use crate::fee::pools::tests::helpers::setup::setup_fee_pools;

    use crate::fee::pools::epoch::epoch_pool::EpochPool;

    mod get_oldest_unpaid_epoch_pool {

        #[test]
        fn test_all_epochs_paid() {
            let drive = super::setup_drive();
            let (transaction, fee_pools) = super::setup_fee_pools(&drive, None);

            match fee_pools
                .get_oldest_unpaid_epoch_pool(&drive, 999, Some(&transaction))
                .expect("should get oldest epoch pool")
            {
                Some(_) => assert!(false, "shouldn't return any unpaid epoch"),
                None => assert!(true),
            }
        }

        #[test]
        fn test_two_unpaid_epochs() {
            let drive = super::setup_drive();
            let (transaction, fee_pools) = super::setup_fee_pools(&drive, None);

            let unpaid_epoch_pool_0 = super::EpochPool::new(0, &drive);

            drive
                .start_current_batch()
                .expect("should start current batch");

            unpaid_epoch_pool_0
                .init_proposers()
                .expect("should create proposers tree");

            // Apply proposers tree
            drive
                .apply_current_batch(true, Some(&transaction))
                .expect("should apply batch");

            drive
                .start_current_batch()
                .expect("should start current batch");

            super::populate_proposers(&unpaid_epoch_pool_0, 2, Some(&transaction));

            let unpaid_epoch_pool_1 = super::EpochPool::new(1, &drive);

            unpaid_epoch_pool_1
                .init_proposers()
                .expect("should create proposers tree");

            // Apply proposers tree
            drive
                .apply_current_batch(true, Some(&transaction))
                .expect("should apply batch");

            drive
                .start_current_batch()
                .expect("should start current batch");

            super::populate_proposers(&unpaid_epoch_pool_1, 2, Some(&transaction));

            drive
                .apply_current_batch(true, Some(&transaction))
                .expect("should apply batch");

            match fee_pools
                .get_oldest_unpaid_epoch_pool(&drive, 1, Some(&transaction))
                .expect("should get oldest epoch pool")
            {
                Some(epoch_pool) => assert_eq!(epoch_pool.index, 0),
                None => assert!(false, "should have unpaid epochs"),
            }
        }
    }

    mod distribute_fees_from_unpaid_pools_to_proposers {
        use crate::{
            error::Error,
            fee::pools::{
                epoch::constants,
                tests::helpers::fee_pools::{
                    create_mn_shares_contract, fetch_identities_by_pro_tx_hashes,
                    refetch_identities, setup_identities_with_share_documents,
                },
            },
        };

        #[test]
        fn test_no_distribution_on_epoch_0() {
            let drive = super::setup_drive();
            let (transaction, fee_pools) = super::setup_fee_pools(&drive, None);

            let current_epoch_index = 0;

            let proposers_paid = fee_pools
                .distribute_fees_from_unpaid_pools_to_proposers(
                    &drive,
                    current_epoch_index,
                    Some(&transaction),
                )
                .expect("should distribute fees");

            assert_eq!(proposers_paid, 0);
        }

        #[test]
        fn test_no_distribution_when_all_epochs_paid() {
            let drive = super::setup_drive();
            let (transaction, fee_pools) = super::setup_fee_pools(&drive, None);

            let current_epoch_index = 1;

            let proposers_paid = fee_pools
                .distribute_fees_from_unpaid_pools_to_proposers(
                    &drive,
                    current_epoch_index,
                    Some(&transaction),
                )
                .expect("should distribute fees");

            assert_eq!(proposers_paid, 0);
        }

        #[test]
        fn test_increased_proposers_limit_for_two_unpaid_epochs() {
            let drive = super::setup_drive();
            let (transaction, fee_pools) = super::setup_fee_pools(&drive, None);

            // Create epochs

            let unpaid_epoch_pool_0 = super::EpochPool::new(0, &drive);
            let unpaid_epoch_pool_1 = super::EpochPool::new(1, &drive);

            drive
                .start_current_batch()
                .expect("should start current batch");

            unpaid_epoch_pool_0
                .init_current(1, 1, 1)
                .expect("should create proposers tree");

            let unpaid_epoch_pool_0_proposers_count = 200;

            unpaid_epoch_pool_1
                .init_current(1, unpaid_epoch_pool_0_proposers_count as u64 + 1, 2)
                .expect("should create proposers tree");

            drive
                .apply_current_batch(true, Some(&transaction))
                .expect("should apply batch");

            drive
                .start_current_batch()
                .expect("should start current batch");

            fee_pools
                .distribute_fees_into_pools(
                    &drive,
                    &unpaid_epoch_pool_0,
                    10000,
                    10000,
                    Some(&transaction),
                )
                .expect("distribute fees into epoch pool 0");

            super::populate_proposers(
                &unpaid_epoch_pool_0,
                unpaid_epoch_pool_0_proposers_count,
                Some(&transaction),
            );

            super::populate_proposers(&unpaid_epoch_pool_1, 200, Some(&transaction));

            // Create masternode reward shares contract
            super::create_mn_shares_contract(&drive);

            drive
                .apply_current_batch(true, Some(&transaction))
                .expect("should apply batch");

            let proposers_paid = fee_pools
                .distribute_fees_from_unpaid_pools_to_proposers(&drive, 2, Some(&transaction))
                .expect("should distribute fees");

            assert_eq!(proposers_paid, 100);
        }

        #[test]
        fn test_partial_distribution() {
            todo!()
        }

        #[test]
        fn test_complete_distribution() {
            let drive = super::setup_drive();
            let (transaction, fee_pools) = super::setup_fee_pools(&drive, None);

            let unpaid_epoch_pool = super::EpochPool::new(0, &drive);
            let next_epoch_pool = super::EpochPool::new(1, &drive);

            drive
                .start_current_batch()
                .expect("should start current batch");

            unpaid_epoch_pool
                .init_current(1, 1, 1)
                .expect("should create proposers tree");

            // emulating epoch change
            next_epoch_pool
                .init_current(1, 11, 10)
                .expect("to init current for next epoch");

            drive
                .apply_current_batch(true, Some(&transaction))
                .expect("should apply batch");

            drive
                .start_current_batch()
                .expect("should start current batch");

            fee_pools
                .distribute_fees_into_pools(
                    &drive,
                    &unpaid_epoch_pool,
                    10000,
                    10000,
                    Some(&transaction),
                )
                .expect("distribute fees into epoch pool");

            let pro_tx_hashes =
                super::populate_proposers(&unpaid_epoch_pool, 10, Some(&transaction));

            let contract = create_mn_shares_contract(&drive);

            let share_identities_and_documents = setup_identities_with_share_documents(
                &drive,
                &contract,
                &pro_tx_hashes,
                Some(&transaction),
            );

            // Create masternode reward shares contract
            super::create_mn_shares_contract(&drive);

            drive
                .apply_current_batch(true, Some(&transaction))
                .expect("should apply batch");

            drive
                .start_current_batch()
                .expect("should start current batch");

            let proposers_paid = fee_pools
                .distribute_fees_from_unpaid_pools_to_proposers(&drive, 1, Some(&transaction))
                .expect("should distribute fees");

            drive
                .apply_current_batch(true, Some(&transaction))
                .expect("should apply batch");

            assert_eq!(proposers_paid, 10);

            // check we paid 500 to every mn identity
            let paid_mn_identities =
                fetch_identities_by_pro_tx_hashes(&drive, &pro_tx_hashes, Some(&transaction));

            for paid_mn_identity in paid_mn_identities {
                assert_eq!(paid_mn_identity.balance, 500);
            }

            let share_identities = share_identities_and_documents
                .iter()
                .map(|(identity, _)| identity)
                .collect();

            let refetched_share_identities =
                refetch_identities(&drive, share_identities, Some(&transaction));

            for identity in refetched_share_identities {
                assert_eq!(identity.balance, 500);
            }

            // check we've removed proposers tree
            match drive
                .grove
                .get(
                    unpaid_epoch_pool.get_path(),
                    constants::KEY_PROPOSERS.as_bytes(),
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

        let current_epoch_pool = EpochPool::new(0, &drive);

        drive
            .start_current_batch()
            .expect("should start current batch");

        current_epoch_pool
            .init_current(1, 1, 1)
            .expect("should init the epoch pool as current");

        // Apply new pool structure
        drive
            .apply_current_batch(true, Some(&transaction))
            .expect("should apply batch");

        drive
            .start_current_batch()
            .expect("should start current batch");

        let processing_fees = 1000000;
        let storage_fees = 2000000;

        fee_pools
            .distribute_fees_into_pools(
                &drive,
                &current_epoch_pool,
                processing_fees,
                storage_fees,
                Some(&transaction),
            )
            .expect("should distribute fees into pools");

        drive
            .apply_current_batch(true, Some(&transaction))
            .expect("should apply batch");

        let stored_processing_fees = current_epoch_pool
            .get_processing_fee(Some(&transaction))
            .expect("should get processing fees");

        let stored_storage_fee_pool = fee_pools
            .storage_fee_distribution_pool
            .value(&drive, Some(&transaction))
            .expect("should get storage fee pool");

        assert_eq!(stored_processing_fees, processing_fees);
        assert_eq!(stored_storage_fee_pool, storage_fees);
    }
}
