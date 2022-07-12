use grovedb::TransactionArg;
use crate::drive::batch::GroveDbOpBatch;
use crate::drive::{Drive, RootTree};
use crate::error::Error;
use crate::fee_pools::add_create_fee_pool_trees_operations;

impl Drive {
    pub fn create_initial_state_structure(&self, transaction: TransactionArg) -> Result<(), Error> {
        let mut batch = GroveDbOpBatch::new();

        batch.add_insert_empty_tree(vec![], vec![RootTree::Identities as u8]);

        batch.add_insert_empty_tree(vec![], vec![RootTree::ContractDocuments as u8]);

        batch.add_insert_empty_tree(vec![], vec![RootTree::PublicKeyHashesToIdentities as u8]);

        batch.add_insert_empty_tree(vec![], vec![RootTree::SpentAssetLockTransactions as u8]);

        batch.add_insert_empty_tree(vec![], vec![RootTree::Pools as u8]);

        // initialize the pools with epochs
        add_create_fee_pool_trees_operations(&mut batch);

        self.grove_apply_batch(batch, false, transaction)?;

        Ok(())
    }
}