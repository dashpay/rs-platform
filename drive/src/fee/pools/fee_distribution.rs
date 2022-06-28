use std::str::FromStr;

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
    ) -> Result<(), Error> {
        if current_epoch_index == 0 {
            return Ok(());
        }

        // For current epoch we pay for previous
        // Find oldest unpaid epoch since previous epoch
        let unpaid_epoch_pool = match self.get_oldest_unpaid_epoch_pool(
            &drive,
            current_epoch_index - 1,
            transaction,
        )? {
            Some(epoch_pool) => epoch_pool,
            None => return Ok(()),
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

        Ok(())
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

        self.get_oldest_unpaid_epoch_pool_recursive(
            &drive,
            from_epoch_index,
            from_epoch_index - 1,
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
    use tempfile::TempDir;

    use crate::{
        contract::{Contract, Document},
        drive::{
            flags::StorageFlags,
            object_size_info::{DocumentAndContractInfo, DocumentInfo::DocumentAndSerialization},
            Drive,
        },
        fee::pools::{constants, epoch::epoch_pool::EpochPool, fee_pools::FeePools},
    };

    fn setup_mn_share_contract_and_docs(drive: &Drive) {
        let contract_hex = "01000000a56324696458200cace205246693a7c8156523620daa937d2f2247934463eeb01ff7219590958c6724736368656d61783468747470733a2f2f736368656d612e646173682e6f72672f6470702d302d342d302f6d6574612f646174612d636f6e7472616374676f776e65724964582024da2bb09da5b1429f717ac1ce6537126cc65215f1d017e67b65eb252ef964b76776657273696f6e0169646f63756d656e7473a16b7265776172645368617265a66474797065666f626a65637467696e646963657382a3646e616d65716f776e65724964416e64506179546f496466756e69717565f56a70726f7065727469657382a168246f776e6572496463617363a167706179546f496463617363a2646e616d65676f776e657249646a70726f7065727469657381a168246f776e65724964636173636872657175697265648267706179546f49646a70657263656e746167656a70726f70657274696573a267706179546f4964a66474797065656172726179686d61784974656d731820686d696e4974656d73182069627974654172726179f56b6465736372697074696f6e781f4964656e74696669657220746f20736861726520726577617264207769746870636f6e74656e744d656469615479706578216170706c69636174696f6e2f782e646173682e6470702e6964656e7469666965726a70657263656e74616765a4647479706567696e7465676572676d6178696d756d192710676d696e696d756d016b6465736372697074696f6e781a5265776172642070657263656e7461676520746f2073686172656b6465736372697074696f6e78405368617265207370656369666965642070657263656e74616765206f66206d61737465726e6f646520726577617264732077697468206964656e746974696573746164646974696f6e616c50726f70657274696573f4";

        let contract_cbor = hex::decode(contract_hex).expect("Decoding failed");

        let contract = Contract::from_cbor(&contract_cbor, None)
            .expect("expected to deserialize the contract");

        drive
            .apply_contract(
                &contract,
                contract_cbor.clone(),
                0f64,
                true,
                StorageFlags { epoch: 0 },
                None,
            )
            .expect("expected to apply contract successfully");

        // ProTxHash identity
        let mn_identity_id =
            hex::decode("0101010101010101010101010101010101010101010101010101010101010101")
                .expect("to decode identity id");
        let mn_identity_bytes = hex::decode("01000000a4626964582001010101010101010101010101010101010101010101010101010101010101016762616c616e63650a687265766973696f6e006a7075626c69634b65797382a6626964006464617461582102eaf222e32d46b97f56f890bb22c3d65e279b18bda203f30bd2d3eed769a3476264747970650067707572706f73650068726561644f6e6c79f46d73656375726974794c6576656c00a6626964016464617461582103c00af793d83155f95502b33a17154110946dcf69ca0dd188bee3b6d10c0d4f8b64747970650067707572706f73650168726561644f6e6c79f46d73656375726974794c6576656c03").expect("to decode identity bytes");

        drive
            .insert_identity_cbor(Some(&mn_identity_id), mn_identity_bytes, true, None)
            .expect("to insert the identity");

        // PayToId identity
        let identity_id =
            hex::decode("43af4034d3844bafd091d11b0bd0c11618717e62ef950ce12657b4baf6a81fd2")
                .expect("to decode identity id");
        let identity_bytes = hex::decode("01000000a4626964582043af4034d3844bafd091d11b0bd0c11618717e62ef950ce12657b4baf6a81fd26762616c616e63650a687265766973696f6e006a7075626c69634b65797382a6626964006464617461582102eaf222e32d46b97f56f890bb22c3d65e279b18bda203f30bd2d3eed769a3476264747970650067707572706f73650068726561644f6e6c79f46d73656375726974794c6576656c00a6626964016464617461582103c00af793d83155f95502b33a17154110946dcf69ca0dd188bee3b6d10c0d4f8b64747970650067707572706f73650168726561644f6e6c79f46d73656375726974794c6576656c03").expect("to decode identity bytes");

        drive
            .insert_identity_cbor(Some(&identity_id), identity_bytes, true, None)
            .expect("to insert the identity");

        let document_hex = "01000000a7632469645820e7a9504ffc0c037c79bfc11417fc5e5eded9d1d52939e7c0990f31b1f50362f56524747970656b726577617264536861726567706179546f4964582043af4034d3844bafd091d11b0bd0c11618717e62ef950ce12657b4baf6a81fd268246f776e657249645820010101010101010101010101010101010101010101010101010101010101010169247265766973696f6e016a70657263656e746167650a6f2464617461436f6e7472616374496458200cace205246693a7c8156523620daa937d2f2247934463eeb01ff7219590958c";

        let document_cbor = hex::decode(document_hex).expect("Decoding failed");

        let document = Document::from_cbor(&document_cbor, None, Some(&mn_identity_id))
            .expect("expected to deserialize the document");

        let document_type = contract
            .document_type_for_name(constants::MN_REWARD_SHARES_DOCUMENT_TYPE)
            .expect("expected to get a document type");

        let storage_flags = StorageFlags { epoch: 0 };

        drive
            .add_document_for_contract(
                DocumentAndContractInfo {
                    document_info: DocumentAndSerialization((
                        &document,
                        &document_cbor,
                        &storage_flags,
                    )),
                    contract: &contract,
                    document_type,
                    owner_id: None,
                },
                false,
                0f64,
                true,
                None,
            )
            .expect("expected to insert a document successfully");
    }

    mod get_oldest_unpaid_epoch_pool {
        use crate::drive::Drive;
        use crate::fee::pools::fee_pools::FeePools;
        use tempfile::TempDir;

        #[test]
        fn test_all_epochs_paid() {
            todo!();

            let tmp_dir = TempDir::new().unwrap();
            let drive: Drive = Drive::open(tmp_dir).expect("expected to open Drive successfully");

            drive
                .create_root_tree(None)
                .expect("expected to create root tree successfully");

            let transaction = drive.grove.start_transaction();

            let fee_pools = FeePools::new();

            fee_pools
                .init(&drive, Some(&transaction))
                .expect("fee pools to init");

            let oldest_epoch = fee_pools
                .get_oldest_unpaid_epoch_pool(&drive, 999, Some(&transaction))
                .expect("to get oldest epoch pool")
                .unwrap();

            assert_eq!(oldest_epoch.index, 999);

            let proposer_pro_tx_hash: [u8; 32] =
                hex::decode("0101010101010101010101010101010101010101010101010101010101010101")
                    .expect("to decode pro tx hash")
                    .try_into()
                    .expect("to convert vector to array of 32 bytes");

            oldest_epoch
                .init_proposers(Some(&transaction))
                .expect("to init proposers tree");

            oldest_epoch
                .increment_proposer_block_count(&proposer_pro_tx_hash, Some(&transaction))
                .expect("to update proposer block count");

            let oldest_epoch = fee_pools
                .get_oldest_unpaid_epoch_pool(&drive, 999, Some(&transaction))
                .expect("to get oldest epoch pool")
                .unwrap();

            assert_eq!(oldest_epoch.index, 998);
        }

        #[test]
        fn test_two_unpaid_epochs() {
            todo!()
        }
    }

    mod distribute_fees_from_unpaid_pools_to_proposers {
        use crate::drive::Drive;
        use crate::fee::pools::epoch::epoch_pool::EpochPool;
        use crate::fee::pools::fee_pools::FeePools;
        use tempfile::TempDir;

        #[test]
        fn test_no_previous_epochs() {
            todo!()
        }

        #[test]
        fn test_all_epochs_paid() {
            todo!()
        }

        #[test]
        fn test_distribution_for_2_epochs_ago() {
            todo!()
        }

        #[test]
        fn test_partial_distribution() {
            todo!()
        }

        #[test]
        fn test_complete_distribution() {
            let tmp_dir = TempDir::new().unwrap();
            let drive: Drive = Drive::open(tmp_dir).expect("expected to open Drive successfully");

            drive
                .create_root_tree(None)
                .expect("expected to create root tree successfully");

            super::setup_mn_share_contract_and_docs(&drive);

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
        let tmp_dir = TempDir::new().unwrap();
        let drive: Drive = Drive::open(tmp_dir).expect("expected to open Drive successfully");

        drive
            .create_root_tree(None)
            .expect("expected to create root tree successfully");

        let transaction = drive.grove.start_transaction();

        let fee_pools = FeePools::new();

        fee_pools
            .init(&drive, Some(&transaction))
            .expect("fee pools to init");

        let epoch_index = 0;

        let start_block_height = 1;

        let epoch_pool = EpochPool::new(epoch_index, &drive);

        epoch_pool
            .init_current(1, start_block_height, 1, Some(&transaction))
            .expect("should init current pool");

        // Distribute fees

        let processing_fees = 42;
        let storage_fees = 1600;

        let proposer_pro_tx_hash: [u8; 32] =
            hex::decode("0101010101010101010101010101010101010101010101010101010101010101")
                .expect("to decode pro tx hash")
                .try_into()
                .expect("to convert vector to array of 32 bytes");

        fee_pools
            .distribute_fees_into_pools(
                &drive,
                &epoch_pool,
                processing_fees,
                storage_fees,
                Some(&transaction),
            )
            .expect("to distribute st fees");

        let stored_processing_fees = epoch_pool
            .get_processing_fee(Some(&transaction))
            .expect("to get processing fees");

        let stored_storage_fee_pool = fee_pools
            .storage_fee_distribution_pool
            .value(&drive, Some(&transaction))
            .expect("to get storage fee pool");

        let stored_block_count = epoch_pool
            .get_proposer_block_count(&proposer_pro_tx_hash, Some(&transaction))
            .expect("to get proposer block count");

        assert_eq!(stored_processing_fees, processing_fees);
        assert_eq!(stored_storage_fee_pool, storage_fees);
        assert_eq!(stored_block_count, 1);
    }
}
