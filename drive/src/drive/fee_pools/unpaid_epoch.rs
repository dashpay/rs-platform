use crate::drive::Drive;
use crate::error::Error;
use crate::fee_pools::epochs::Epoch;
use grovedb::TransactionArg;

impl Drive {
    // TODO: We should cache last paid epoch in execution logic so we don't need to do two reads from db every block

    // TODO: Move to execution, it's not a storage logic
    pub fn get_oldest_unpaid_epoch_pool(
        &self,
        from_epoch_index: u16,
        transaction: TransactionArg,
    ) -> Result<Option<Epoch>, Error> {
        self.get_oldest_unpaid_epoch_pool_recursive(from_epoch_index, from_epoch_index, transaction)
    }

    pub fn get_oldest_unpaid_epoch_pool_recursive(
        &self,
        from_epoch_index: u16,
        epoch_index: u16,
        transaction: TransactionArg,
    ) -> Result<Option<Epoch>, Error> {
        let epoch_pool = Epoch::new(epoch_index);

        // TODO: It's wrong, we should use get_epoch_start_block_height to check is it a gap or not.
        if self.is_epoch_tree_exists(&epoch_pool, transaction)? {
            if self.is_epochs_proposers_tree_empty(&epoch_pool, transaction)? {
                return if epoch_index == from_epoch_index {
                    Ok(None)
                } else {
                    let unpaid_epoch_pool = Epoch::new(epoch_index + 1);

                    Ok(Some(unpaid_epoch_pool))
                };
            }

            if epoch_index == 0 {
                return Ok(Some(epoch_pool));
            }
        }

        self.get_oldest_unpaid_epoch_pool_recursive(from_epoch_index, epoch_index - 1, transaction)
    }
}

#[cfg(test)]
mod tests {
    mod get_oldest_unpaid_epoch_pool {
        use crate::common::helpers::identities::create_test_masternode_identities_and_add_them_as_epoch_block_proposers;
        use crate::common::helpers::setup::setup_drive_with_initial_state_structure;
        use crate::drive::batch::GroveDbOpBatch;
        use crate::fee_pools::epochs::Epoch;

        #[test]
        fn test_all_epochs_paid() {
            let drive = setup_drive_with_initial_state_structure();
            let transaction = drive.grove.start_transaction();

            match drive
                .get_oldest_unpaid_epoch_pool(999, Some(&transaction))
                .expect("should get oldest epochs pool")
            {
                Some(_) => assert!(false, "shouldn't return any unpaid epochs"),
                None => assert!(true),
            }
        }

        #[test]
        fn test_two_unpaid_epochs() {
            let drive = setup_drive_with_initial_state_structure();
            let transaction = drive.grove.start_transaction();

            let unpaid_epoch_pool_0 = Epoch::new(0);

            let mut batch = GroveDbOpBatch::new();

            batch.push(unpaid_epoch_pool_0.init_proposers_tree_operation());

            // Apply proposers tree
            drive
                .grove_apply_batch(batch, false, Some(&transaction))
                .expect("should apply batch");

            create_test_masternode_identities_and_add_them_as_epoch_block_proposers(
                &drive,
                &unpaid_epoch_pool_0,
                2,
                Some(&transaction),
            );

            let unpaid_epoch_pool_1 = Epoch::new(1);

            let mut batch = GroveDbOpBatch::new();

            batch.push(unpaid_epoch_pool_1.init_proposers_tree_operation());

            // Apply proposers tree
            drive
                .grove_apply_batch(batch, false, Some(&transaction))
                .expect("should apply batch");

            create_test_masternode_identities_and_add_them_as_epoch_block_proposers(
                &drive,
                &unpaid_epoch_pool_1,
                2,
                Some(&transaction),
            );

            match drive
                .get_oldest_unpaid_epoch_pool(1, Some(&transaction))
                .expect("should get oldest epochs pool")
            {
                Some(epoch_pool) => assert_eq!(epoch_pool.index, 0),
                None => assert!(false, "should have unpaid epochs"),
            }
        }
    }
}
