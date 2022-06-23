use grovedb::TransactionArg;

use crate::drive::Drive;
use crate::error::Error;

use crate::fee::pools::epoch::epoch_pool::EpochPool;
use chrono::Utc;

impl Drive {
    pub fn init_fee_pools(&self, transaction: TransactionArg) -> Result<(), Error> {
        // initialize the pools with epochs
        self.fee_pools.borrow().init(self, transaction)
    }

    pub fn process_block(
        &self,
        block_height: u64,
        block_time: i64,
        previous_block_time: i64,
        proposer_pro_tx_hash: [u8; 32],
        processing_fees: u64,
        storage_fees: i64,
        fee_multiplier: u64,
        transaction: TransactionArg,
    ) -> Result<(), Error> {
        if block_height == 1 {
            let genesis_time = Utc::now().timestamp();

            self.fee_pools
                .borrow_mut()
                .update_genesis_time(&self, genesis_time, transaction)?;
        }

        let fee_pools = self.fee_pools.borrow();

        let (current_epoch_index, is_epoch_change) = fee_pools.calculate_current_epoch_index(
            &self,
            block_time,
            previous_block_time,
            transaction,
        )?;

        let current_epoch_pool = EpochPool::new(current_epoch_index, self);

        if is_epoch_change {
            // make next epoch pool as a current
            // and create one more in future
            fee_pools.shift_current_epoch_pool(
                &self,
                &current_epoch_pool,
                block_height,
                fee_multiplier,
                transaction,
            )?;

            // distribute accumulated previous epoch storage fees
            fee_pools.storage_fee_distribution_pool.distribute(
                &self,
                current_epoch_pool.index,
                transaction,
            )?;
        }

        fee_pools.distribute_fees_into_pools(
            &self,
            &current_epoch_pool,
            processing_fees,
            storage_fees,
            transaction,
        )?;

        current_epoch_pool.increment_proposer_block_count(&proposer_pro_tx_hash, transaction)?;

        fee_pools.distribute_fees_from_unpaid_pools_to_proposers(
            &self,
            current_epoch_index,
            transaction,
        )?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        contract::{Contract, Document},
        drive::{
            flags::StorageFlags,
            object_size_info::{DocumentAndContractInfo, DocumentInfo::DocumentAndSerialization},
            Drive,
        },
        fee::pools::constants,
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

    mod process_block {
        use crate::drive::Drive;
        use chrono::Utc;
        use tempfile::TempDir;

        #[test]
        fn test_process_first_block() {
            let tmp_dir = TempDir::new().unwrap();
            let mut drive: Drive =
                Drive::open(tmp_dir).expect("expected to open Drive successfully");

            drive
                .create_root_tree(None)
                .expect("expected to create root tree successfully");

            super::setup_mn_share_contract_and_docs(&drive);

            let transaction = drive.grove.start_transaction();

            drive
                .init_fee_pools(Some(&transaction))
                .expect("to init fee pools");

            let block_time = Utc::now().timestamp();
            let previous_block_time = Utc::now().timestamp();

            let proposer_pro_tx_hash = [
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0,
            ];

            let processing_fees = 100;
            let storage_fees = 2000;

            drive
                .process_block(
                    1,
                    block_time,
                    previous_block_time,
                    proposer_pro_tx_hash,
                    processing_fees,
                    storage_fees,
                    1,
                    Some(&transaction),
                )
                .expect("to process block 1");
        }

        #[test]
        fn test_process_second_block() {
            todo!()
        }

        #[test]
        fn test_process_epoch_change() {
            todo!()
        }
    }
}
