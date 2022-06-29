use grovedb::TransactionArg;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde_json::json;

use crate::common::value_to_cbor;
use crate::contract::Document;
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

        for (proposer_tx_hash, proposed_block_count) in proposers {
            let proposed_block_count = Decimal::from(proposed_block_count);

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

        next_epoch_pool
            .update_processing_fee(processing_fee + processing_leftovers, transaction)?;

        let storage_fee = next_epoch_pool.get_storage_fee(transaction)?;

        next_epoch_pool.update_storage_fee(storage_fee + storage_leftovers, transaction)?;

        Ok(())
    }

    fn pay_reward_to_identity(
        drive: &Drive,
        id: &Vec<u8>,
        reward: Decimal,
        transaction: TransactionArg,
    ) -> Result<(), Error> {
        // Convert to integer, since identity balance is u64
        let reward: u64 = reward.try_into().map_err(|_| {
            Error::Fee(FeeError::DecimalConversion(
                "can't convert reward to u64 from Decimal",
            ))
        })?;

        // We don't need additional verification, since we ensure an identity
        // existence in the data contract triggers in DPP
        let mut identity = drive.fetch_identity(id, transaction)?;

        identity.balance += reward;

        drive.insert_identity_cbor(Some(id), identity.to_cbor(), true, transaction)?;

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

        current_epoch_pool
            .update_processing_fee(epoch_processing_fees + processing_fees, transaction)?;

        // update storage fee pool
        let storage_fee_pool = self
            .storage_fee_distribution_pool
            .value(&drive, transaction)?;

        self.storage_fee_distribution_pool.update(
            &drive,
            storage_fee_pool + storage_fees,
            transaction,
        )?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::fee::pools::tests::helpers::fee_pools::create_mn_shares_contract;
    use crate::fee::pools::tests::helpers::fee_pools::populate_proposers;
    use crate::fee::pools::tests::helpers::setup::setup_drive;
    use crate::fee::pools::tests::helpers::setup::setup_fee_pools;

    use crate::{
        contract::{Contract, Document},
        drive::{
            flags::StorageFlags,
            object_size_info::{DocumentAndContractInfo, DocumentInfo::DocumentAndSerialization},
            Drive,
        },
        fee::pools::{constants, epoch::epoch_pool::EpochPool, fee_pools::FeePools},
    };

    mod get_oldest_unpaid_epoch_pool {

        #[test]
        fn test_all_epochs_paid() {
            let drive = super::setup_drive();
            let (transaction, fee_pools) = super::setup_fee_pools(&drive, None);

            match fee_pools
                .get_oldest_unpaid_epoch_pool(&drive, 999, Some(&transaction))
                .expect("to get oldest epoch pool")
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

            unpaid_epoch_pool_0
                .init_proposers(Some(&transaction))
                .expect("should create proposers tree");

            super::populate_proposers(&unpaid_epoch_pool_0, 2, Some(&transaction));

            let unpaid_epoch_pool_1 = super::EpochPool::new(1, &drive);

            unpaid_epoch_pool_1
                .init_proposers(Some(&transaction))
                .expect("should create proposers tree");

            super::populate_proposers(&unpaid_epoch_pool_1, 2, Some(&transaction));

            match fee_pools
                .get_oldest_unpaid_epoch_pool(&drive, 1, Some(&transaction))
                .expect("to get oldest epoch pool")
            {
                Some(epoch_pool) => assert_eq!(epoch_pool.index, 0),
                None => assert!(false, "should have unpaid epochs"),
            }
        }
    }

    mod distribute_fees_from_unpaid_pools_to_proposers {
        use crate::drive::Drive;
        use crate::fee::pools::epoch::epoch_pool::EpochPool;
        use crate::fee::pools::fee_pools::FeePools;
        use tempfile::TempDir;

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

            // Create epoch 0

            let unpaid_epoch_pool_0 = super::EpochPool::new(0, &drive);

            let unpaid_epoch_pool_0_proposers_count = 200;

            unpaid_epoch_pool_0
                .init_current(1, 1, 1, Some(&transaction))
                .expect("should create proposers tree");

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

            // Create epoch 1

            let unpaid_epoch_pool_1 = super::EpochPool::new(1, &drive);

            unpaid_epoch_pool_1
                .init_current(
                    1,
                    unpaid_epoch_pool_0_proposers_count as u64 + 1,
                    2,
                    Some(&transaction),
                )
                .expect("should create proposers tree");

            super::populate_proposers(&unpaid_epoch_pool_1, 200, Some(&transaction));

            // Create masternode reward shares contract
            super::create_mn_shares_contract(&drive);

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
            todo!();

            let tmp_dir = TempDir::new().unwrap();
            let drive: Drive = Drive::open(tmp_dir).expect("expected to open Drive successfully");

            drive
                .create_root_tree(None)
                .expect("expected to create root tree successfully");

            // super::setup_mn_share_contract_and_docs(&drive);

            let proposer_pro_tx_hash: [u8; 32] =
                hex::decode("0101010101010101010101010101010101010101010101010101010101010101")
                    .expect("to decode pro tx hash")
                    .try_into()
                    .expect("to convert vector to array of 32 bytes");

            let transaction = drive.grove.start_transaction();

            let fee_pools = FeePools::new();

            fee_pools
                .init(&drive, Some(&transaction))
                .expect("fee pools to init");

            let epoch = EpochPool::new(0, &drive);

            // set initial data for test
            fee_pools
                .shift_current_epoch_pool(&drive, &epoch, 1, 1, 1, Some(&transaction))
                .expect("to process epoch change");

            let block_count = 42;

            epoch
                .update_proposer_block_count(&proposer_pro_tx_hash, block_count, Some(&transaction))
                .expect("to update proposer block count");

            fee_pools
                .distribute_fees_from_unpaid_pools_to_proposers(&drive, 0, Some(&transaction))
                .expect("to distribute fees to proporsers");

            match drive
                .grove
                .get(FeePools::get_path(), &epoch.key, Some(&transaction))
            {
                Ok(_) => assert!(false, "should not be able to get deleted epoch pool"),
                Err(e) => match e {
                    grovedb::Error::PathKeyNotFound(_) => assert!(true),
                    _ => assert!(false, "invalid error type"),
                },
            }

            todo!("Check updated balances");
        }
    }

    #[test]
    fn test_distribute_fees_into_pools() {
        let drive = setup_drive();
        let (transaction, fee_pools) = setup_fee_pools(&drive, None);

        let current_epoch_pool = EpochPool::new(0, &drive);
        current_epoch_pool
            .init_current(1, 1, 1, Some(&transaction))
            .expect("should init the epoch pool as current");

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

        let stored_processing_fees = current_epoch_pool
            .get_processing_fee(Some(&transaction))
            .expect("to get processing fees");

        let stored_storage_fee_pool = fee_pools
            .storage_fee_distribution_pool
            .value(&drive, Some(&transaction))
            .expect("to get storage fee pool");

        assert_eq!(stored_processing_fees, processing_fees);
        assert_eq!(stored_storage_fee_pool, storage_fees);
    }
}
