use grovedb::{Element, TransactionArg};
use crate::drive::{Drive, RootTree};
use crate::error::drive::DriveError;
use crate::error::Error;

fn root_pools_path<'a>() -> [&'a [u8]; 1] {
    [
        Into::<&[u8; 1]>::into(RootTree::Pools)
    ]
}

fn epoch_path<'a>(epoch: u16) -> [&'a [u8]; 2] {
    [
        Into::<&[u8; 1]>::into(RootTree::Pools),
        epoch.to_le_bytes().to_vec().as_slice()
    ]
}

pub struct PoolInfo {
    pub genesis_time: f64,
}

impl Drive {
    pub fn init_pools(&self, genesis_time: f64, transaction: TransactionArg) -> Result<(), Error> {
        let root_pools_path = root_pools_path();
        // This can never happen after the first init, hence we should make sure nothing exists at the pools path already

        let encoded_genesis_time = genesis_time.to_le_bytes().to_vec();
        // We must encode and store the genesis time
        self.grove
            .insert(root_pools_path, "g".as_bytes(), Element::Item(encoded_genesis_time), transaction)
            .map_err(Error::GroveDB)?;

        // We need to insert 50 years worth of epochs, with 20 epochs per year that's 1000 epochs
        for i in 0..1000 {
            self.init_epoch_tree(i, transaction)?;
        }

        // We place the genesis time into the pool info
        self.pool_info.replace(Some(PoolInfo{
            genesis_time
        }));

        Ok(())
    }

    pub fn load_pool_info(&self, transaction: TransactionArg) -> Result<(), Error> {
        let genesis_time = self.get_genesis_time(transaction)?;
        if let Some(genesis_time) = genesis_time {
            self.pool_info.replace(Some(PoolInfo{
                genesis_time
            }));
            Ok(())
        } else {
            Err(Error::Drive(DriveError::CorruptedCodeExecution("")))
        }
    }

    pub fn get_genesis_time(&self, transaction: TransactionArg) -> Result<Option<f64>, Error> {
        let root_pools_path = root_pools_path();
        match self.grove.get(root_pools_path, "g".as_bytes(), transaction: TransactionArg) {
            Ok(element) => {
                if let Element::Item(item) = element {
                    Ok(Some(f64::from_le_bytes(item)))
                } else {
                    Err(Error::Drive(DriveError::CorruptedGenesisElement("genesis time must be an item"))
                }
            }
            Err(err) => {
                match err {
                    Error::PathKeyNotFound(e) => {}
                    _ => { Error::GroveDB(err)}
                }
            }
        }
    }

    pub fn init_epoch_tree(&self, epoch_index: u16, transaction: TransactionArg)  -> Result<(), Error> {
        if epoch_index == 0 {
            return Err(Error::Drive(DriveError::CorruptedCodeExecution("epoch should never be 0")))
        }
        let key = epoch_index.to_le_bytes().as_slice();
        self.grove
            .insert(root_pools_path(), key, Element::empty_tree(), transaction)
            .map_err(Error::GroveDB)?;

        let epoch_path = epoch_path(epoch_index);
        // For each epoch we must insert the storage credits for that epoch
        self.grove
            .insert(epoch_path, "s".as_bytes(), Element::Item(0u64.to_le_bytes().to_vec()), transaction)
            .map_err(Error::GroveDB)
    }

    pub fn update_epoch_tree_for_current_epoch(&self, epoch_index: u16, first_proposer_block_height: u64, transaction: TransactionArg)  -> Result<(), Error> {
        if epoch_index == 0 {
            return Err(Error::Drive(DriveError::CorruptedCodeExecution("epoch should never be 0")))
        }

        // We store the processing credits as p
        let epoch_path = epoch_path(epoch_index);

        // For each epoch we must insert the processing credits for that epoch as p
        self.grove
            .insert(epoch_path, "p".as_bytes(), Element::Item(0u64.to_le_bytes().to_vec()), transaction)
            .map_err(Error::GroveDB)?;

        // For each epoch we must insert the first proposer block height as c
        self.grove
            .insert(epoch_path, "c".as_bytes(), Element::Item(first_proposer_block_height.to_le_bytes().to_vec()), transaction)
            .map_err(Error::GroveDB)?;

        // For each epoch we must insert the proposer tree under m
        self.grove
            .insert(epoch_path, "m".as_bytes(), Element::empty_tree(), transaction)
            .map_err(Error::GroveDB)
    }

    fn split_storage_fees_for_distribution(fees: u64) -> Vec<u64> {
        let mut distribution_fee : Vec<u64> = vec![];
        // todo()!
        // We need to split the distribution fee based on the values provided in the DIP
        distribution_fee
    }

    fn distribute_storage_distribution_pool(&self, current_epoch_index: u16, transaction: TransactionArg) -> Result<(), Error> {
        // First we need to get the total value of the storage distribution pool

        // Then we need to split the values by epoch years

        // Then we need to add the values to the storage credits of each epoch, 20 epochs at a time
    }

    pub fn change_to_epoch(&self, epoch_index: u16, first_proposer_block_height: u64, transaction: TransactionArg)  -> Result<(), Error> {
        self.update_epoch_tree_for_current_epoch(epoch_index, first_proposer_block_height, transaction)?;
        // We also need to create a new epoch pool 1000 epochs later
        self.init_epoch_tree(epoch_index + 1000, transaction);

        // We need to distribute the storage fees
        self.distribute_storage_distribution_pool(epoch_index, transaction)
    }

    fn proposer_proposed_block_in_epoch(&self, epoch_index: u16, proposer_pro_tx_hash: [u8;32], processing_fees: u64, transaction: TransactionArg)  -> Result<(), Error> {

    }

    pub fn process_block(&self, block_height: u64, block_time: f64, proposer_pro_tx_hash: [u8;32], processing_fees: u64, storage_fees: u64, transaction: TransactionArg)  -> Result<(), Error> {
        // If block time is over the epoch time then we need to change epochs

        // Storage fees should go into the storage distribution pool

        // Processing fees should be added to the next epoch distribution pool
        self.proposer_proposed_block_in_epoch(epoch_index, proposer_pro_tx_hash, processing_fees)
    }
}