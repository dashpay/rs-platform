use grovedb::TransactionArg;
use serde_json::json;

use crate::common::value_to_cbor;
use crate::contract::Document;
use crate::error::document::DocumentError;
use crate::error::{self, Error};
use crate::fee::pools::fee_pools::FeePools;

use super::constants;
use super::epoch::epoch_pool::EpochPool;

fn get_fee_distribution_percent(epoch_index: u16, start_index: u16) -> f64 {
    let reset_epoch_index = epoch_index - start_index;

    let epoch_year = (reset_epoch_index as f64 / 20.0).trunc() as usize;

    constants::FEE_DISTRIBUTION_TABLE[epoch_year]
}

impl<'f> FeePools<'f> {
    pub fn distribute_storage_distribution_pool(
        &self,
        epoch_index: u16,
        transaction: TransactionArg,
    ) -> Result<(), Error> {
        let mut fee_pool_value = self.get_storage_fee_pool(transaction)?;

        for index in epoch_index..epoch_index + 1000 {
            let epoch_pool = EpochPool::new(index, self.drive);

            let distribution_percent = get_fee_distribution_percent(index, epoch_index);

            let fee_share = fee_pool_value * distribution_percent;

            let storage_fee = epoch_pool.get_storage_fee(transaction)?;

            epoch_pool.update_storage_fee(storage_fee + fee_share, transaction)?;

            fee_pool_value -= fee_share;
        }

        self.update_storage_fee_pool(fee_pool_value, transaction)
    }

    pub fn distribute_fees_to_proposers(
        &self,
        epoch_index: u16,
        transaction: TransactionArg,
    ) -> Result<(), Error> {
        let epoch_pool = self.get_oldest_epoch_pool(epoch_index, transaction)?;

        let proposers_limit: u16 = if epoch_pool.index == epoch_index {
            50
        } else {
            (epoch_index - epoch_pool.index) * 50
        };

        let accumulated_fees = epoch_pool.get_combined_fee(transaction)?;

        let next_epoch_pool = EpochPool::new(epoch_pool.index + 1, self.drive);
        let epoch_block_count = next_epoch_pool.get_first_proposed_block_height(transaction)?
            - epoch_pool.get_first_proposed_block_height(transaction)?;

        let proposers = epoch_pool.get_proposers(proposers_limit, transaction)?;

        let proposers_len = proposers.len();

        for (proposer_tx_hash, proposed_block_count) in proposers {
            let query_json = json!({
                "where": [
                    ["$ownerId", "==", bs58::encode(proposer_tx_hash).into_string()]
                ],
            });

            let query_cbor = value_to_cbor(query_json, None);

            let (document_cbors, _, _) = self.drive.query_documents(
                &query_cbor,
                constants::MN_REWARD_SHARES_CONTRACT_ID,
                constants::MN_REWARD_SHARES_DOCUMENT_TYPE,
                transaction,
            )?;

            let documents: Vec<Document> = document_cbors
                .iter()
                .map(|cbor| Ok(Document::from_cbor(cbor, None, None)?))
                .collect::<Result<Vec<Document>, Error>>()?;

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

                let mut identity = self.drive.fetch_identity(pay_to_id, transaction)?;

                let share_percentage_integer: u64 = document
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
                            "percentage property cannot be converted to u64",
                        ))
                    })?;

                let share_percentage: f64 = share_percentage_integer as f64 / 100.0;

                let reward: f64 =
                    ((accumulated_fees * proposed_block_count as f64 * share_percentage)
                        / epoch_block_count as f64)
                        .floor();

                identity.balance += reward as u64;

                self.drive.insert_identity_cbor(
                    Some(pay_to_id),
                    identity.to_cbor(),
                    true,
                    transaction,
                )?;
            }
        }

        // if less then a limit processed - drop the pool
        if proposers_len < proposers_limit.into() {
            epoch_pool.delete(transaction)?;
        }

        Ok(())
    }

    pub fn distribute_st_fees(
        &self,
        epoch_index: u16,
        processing_fees: f64,
        storage_fees: f64,
        proposer_pro_tx_hash: [u8; 32],
        transaction: TransactionArg,
    ) -> Result<(), Error> {
        let epoch_pool = EpochPool::new(epoch_index, self.drive);

        // update epoch pool processing fees
        let epoch_processing_fees = epoch_pool.get_processing_fee(transaction)?;
        epoch_pool.update_processing_fee(epoch_processing_fees + processing_fees, transaction)?;

        // update storage fee pool
        let storage_fee_pool = self.get_storage_fee_pool(transaction)?;
        self.update_storage_fee_pool(storage_fee_pool + storage_fees, transaction)?;

        // update proposer's block count
        let proposed_block_count = epoch_pool
            .get_proposer_block_count(&proposer_pro_tx_hash, transaction)
            .or_else(|e| match e {
                error::Error::GroveDB(grovedb::Error::PathKeyNotFound(_)) => Ok(0u64),
                _ => Err(e),
            })?;

        epoch_pool.update_proposer_block_count(
            &proposer_pro_tx_hash,
            proposed_block_count + 1,
            transaction,
        )
    }
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use crate::{
        drive::Drive,
        fee::pools::{epoch::epoch_pool::EpochPool, fee_pools::FeePools},
    };

    #[test]
    fn test_fee_pools_distribute_storage_distribution_pool() {
        let tmp_dir = TempDir::new().unwrap();
        let drive: Drive = Drive::open(tmp_dir).expect("expected to open Drive successfully");

        drive
            .create_root_tree(None)
            .expect("expected to create root tree successfully");

        let transaction = drive.grove.start_transaction();

        let fee_pools = FeePools::new(&drive);

        fee_pools
            .init(Some(&transaction))
            .expect("fee pools to init");

        let storage_pool = 1000.0;
        let epoch_index = 42;

        // init additional epoch pools as it will be done in epoch_change
        for i in 1000..=1000 + epoch_index {
            let epoch = EpochPool::new(i, &drive);
            epoch
                .init(Some(&transaction))
                .expect("to init additional epoch pool");
        }

        fee_pools
            .update_storage_fee_pool(storage_pool, Some(&transaction))
            .expect("to update storage fee pool");

        fee_pools
            .distribute_storage_distribution_pool(epoch_index, Some(&transaction))
            .expect("to distribute storage fee pool");

        let leftover_storage_fee_pool = fee_pools
            .get_storage_fee_pool(Some(&transaction))
            .expect("to get storage fee pool");

        assert_eq!(leftover_storage_fee_pool, 1.5260017107721069e-6);

        // selectively check 1st and last item
        let first_epoch = EpochPool::new(epoch_index, &drive);

        let first_epoch_storage_fee = first_epoch
            .get_storage_fee(Some(&transaction))
            .expect("to get storage fee");

        assert_eq!(first_epoch_storage_fee, 50.0);

        let last_epoch = EpochPool::new(epoch_index + 999, &drive);

        let last_epoch_storage_fee = last_epoch
            .get_storage_fee(Some(&transaction))
            .expect("to get storage fee");

        assert_eq!(last_epoch_storage_fee, 1.909889563258572e-9);
    }

    #[test]
    fn test_fee_pools_distribute_fees_to_proposers() {
        todo!()
    }

    #[test]
    fn test_fee_pools_distribute_st_fees() {
        let tmp_dir = TempDir::new().unwrap();
        let drive: Drive = Drive::open(tmp_dir).expect("expected to open Drive successfully");

        drive
            .create_root_tree(None)
            .expect("expected to create root tree successfully");

        let transaction = drive.grove.start_transaction();

        let fee_pools = FeePools::new(&drive);

        fee_pools
            .init(Some(&transaction))
            .expect("fee pools to init");

        let epoch_index = 0;

        let epoch_pool = EpochPool::new(epoch_index, &drive);

        // emulating epoch_change
        epoch_pool
            .update_processing_fee(0f64, Some(&transaction))
            .expect("to update processing fee");

        epoch_pool
            .update_storage_fee(0f64, Some(&transaction))
            .expect("to update storage fee");

        epoch_pool
            .init_proposers_tree(Some(&transaction))
            .expect("to init proposers tree");

        let processing_fees = 0.42;
        let storage_fees = 0.16;

        let proposer_pro_tx_hash: [u8; 32] =
            hex::decode("0101010101010101010101010101010101010101010101010101010101010101")
                .expect("to decode pro tx hash")
                .try_into()
                .expect("to convert vector to array of 32 bytes");

        fee_pools
            .distribute_st_fees(
                epoch_index,
                processing_fees,
                storage_fees,
                proposer_pro_tx_hash,
                Some(&transaction),
            )
            .expect("to distribute st fees");

        let stored_processing_fees = epoch_pool
            .get_processing_fee(Some(&transaction))
            .expect("to get processing fees");

        let stored_storage_fee_pool = fee_pools
            .get_storage_fee_pool(Some(&transaction))
            .expect("to get storage fee pool");

        let stored_block_count = epoch_pool
            .get_proposer_block_count(&proposer_pro_tx_hash, Some(&transaction))
            .expect("to get proposer block count");

        assert_eq!(stored_processing_fees, processing_fees);
        assert_eq!(stored_storage_fee_pool, storage_fees);
        assert_eq!(stored_block_count, 1);
    }
}
