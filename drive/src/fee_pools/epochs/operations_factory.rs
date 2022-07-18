use crate::drive::batch::GroveDbOpBatch;
use crate::drive::fee_pools::pools_vec_path;
use crate::drive::Drive;
use crate::error::Error;
use crate::fee_pools::epochs::epoch_key_constants::{
    KEY_FEE_MULTIPLIER, KEY_POOL_PROCESSING_FEES, KEY_POOL_STORAGE_FEES, KEY_START_BLOCK_HEIGHT,
    KEY_START_TIME,
};
use crate::fee_pools::epochs::{epoch_key_constants, Epoch};
use grovedb::batch::Op::Insert;
use grovedb::batch::{GroveDbOp, Op};
use grovedb::{Element, TransactionArg};

impl Epoch {
    pub fn increment_proposer_block_count_operation(
        &self,
        drive: &Drive,
        is_epoch_change: bool,
        proposer_pro_tx_hash: &[u8; 32],
        transaction: TransactionArg,
    ) -> Result<GroveDbOp, Error> {
        // update proposer's block count
        let proposed_block_count = if is_epoch_change {
            0
        } else {
            drive
                .get_epochs_proposer_block_count(self, proposer_pro_tx_hash, transaction)
                .or_else(|e| match e {
                    Error::GroveDB(grovedb::Error::PathKeyNotFound(_)) => Ok(0u64),
                    _ => Err(e),
                })?
        };

        Ok(self
            .update_proposer_block_count_operation(proposer_pro_tx_hash, proposed_block_count + 1))
    }

    pub fn shift_to_new_epoch_operations(
        &self,
        start_block_height: u64,
        start_block_time_ms: u64,
        fee_multiplier: f64,
        batch: &mut GroveDbOpBatch,
    ) {
        // create and init next thousandth epochs
        let next_thousandth_epoch = Epoch::new(self.index + 1000);
        next_thousandth_epoch.add_init_empty_operations(batch);

        // init first_proposer_block_height and processing_fee for an epochs
        self.add_init_current_operations(
            fee_multiplier,
            start_block_height,
            start_block_time_ms,
            batch,
        );
    }

    pub fn add_init_empty_operations(&self, batch: &mut GroveDbOpBatch) {
        batch.add_insert_empty_tree(pools_vec_path(), self.key.to_vec());

        // init storage fee item to 0
        batch.push(self.update_storage_credits_for_distribution_operation(0));
    }

    pub fn add_init_current_operations(
        &self,
        multiplier: f64,
        start_block_height: u64,
        start_time_ms: u64,
        batch: &mut GroveDbOpBatch,
    ) {
        batch.push(self.update_start_block_height_operation(start_block_height));

        batch.push(self.init_proposers_tree_operation());

        batch.push(self.update_fee_multiplier_operation(multiplier));

        batch.push(self.update_start_time_operation(start_time_ms));
    }

    pub fn add_mark_as_paid_operations(&self, batch: &mut GroveDbOpBatch) {
        batch.push(self.delete_proposers_tree_operation());

        batch.push(self.delete_storage_credits_for_distribution_operation());

        batch.push(self.delete_processing_credits_for_distribution_operation());
    }

    pub fn update_start_time_operation(&self, time_ms: u64) -> GroveDbOp {
        GroveDbOp {
            path: self.get_vec_path(),
            key: KEY_START_TIME.to_vec(),
            op: Insert {
                element: Element::Item(time_ms.to_be_bytes().to_vec(), None),
            },
        }
    }

    pub fn update_start_block_height_operation(&self, start_block_height: u64) -> GroveDbOp {
        GroveDbOp {
            path: self.get_vec_path(),
            key: KEY_START_BLOCK_HEIGHT.to_vec(),
            op: Insert {
                element: Element::Item(start_block_height.to_be_bytes().to_vec(), None),
            },
        }
    }

    pub fn update_fee_multiplier_operation(&self, multiplier: f64) -> GroveDbOp {
        GroveDbOp {
            path: self.get_vec_path(),
            key: KEY_FEE_MULTIPLIER.to_vec(),
            op: Insert {
                element: Element::Item(multiplier.to_be_bytes().to_vec(), None),
            },
        }
    }

    pub fn update_processing_credits_for_distribution_operation(
        &self,
        processing_fee: u64,
    ) -> GroveDbOp {
        GroveDbOp {
            path: self.get_vec_path(),
            key: KEY_POOL_PROCESSING_FEES.to_vec(),
            op: Insert {
                element: Element::new_item(processing_fee.to_be_bytes().to_vec()),
            },
        }
    }

    pub fn delete_processing_credits_for_distribution_operation(&self) -> GroveDbOp {
        GroveDbOp {
            path: self.get_vec_path(),
            key: KEY_POOL_PROCESSING_FEES.to_vec(),
            op: Op::Delete,
        }
    }

    pub fn update_storage_credits_for_distribution_operation(&self, storage_fee: u64) -> GroveDbOp {
        GroveDbOp {
            path: self.get_vec_path(),
            key: KEY_POOL_STORAGE_FEES.to_vec(),
            op: Insert {
                element: Element::new_item(storage_fee.to_be_bytes().to_vec()),
            },
        }
    }

    pub fn delete_storage_credits_for_distribution_operation(&self) -> GroveDbOp {
        GroveDbOp {
            path: self.get_vec_path(),
            key: KEY_POOL_STORAGE_FEES.to_vec(),
            op: Op::Delete,
        }
    }

    pub(crate) fn update_proposer_block_count_operation(
        &self,
        proposer_pro_tx_hash: &[u8; 32],
        block_count: u64,
    ) -> GroveDbOp {
        GroveDbOp {
            path: self.get_proposers_vec_path(),
            key: proposer_pro_tx_hash.to_vec(),
            op: Insert {
                element: Element::Item(block_count.to_be_bytes().to_vec(), None),
            },
        }
    }

    pub fn init_proposers_tree_operation(&self) -> GroveDbOp {
        GroveDbOp {
            path: self.get_vec_path(),
            key: epoch_key_constants::KEY_PROPOSERS.to_vec(),
            op: Insert {
                element: Element::empty_tree(),
            },
        }
    }

    pub fn delete_proposers_tree_operation(&self) -> GroveDbOp {
        GroveDbOp {
            path: self.get_vec_path(),
            key: epoch_key_constants::KEY_PROPOSERS.to_vec(),
            op: Op::Delete,
        }
    }

    pub fn add_delete_proposers_operations(
        &self,
        pro_tx_hashes: Vec<Vec<u8>>,
        batch: &mut GroveDbOpBatch,
    ) {
        for pro_tx_hash in pro_tx_hashes.into_iter() {
            batch.add_delete(self.get_proposers_vec_path(), pro_tx_hash);
        }
    }
}
