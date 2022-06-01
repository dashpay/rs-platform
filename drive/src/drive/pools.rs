use grovedb::{Element, TransactionArg};
use crate::drive::{Drive, RootTree};
use crate::error::drive::DriveError;
use crate::error::Error;

const FEE_DISTRIBUTION_TABLE: [f64; 50] = [
    0.050000, 0.048000, 0.046000, 0.044000, 0.042000, 0.040000, 0.038500, 0.037000, 0.035500, 0.034000,
    0.032500, 0.031000, 0.029500, 0.028500, 0.027500, 0.026500, 0.025500, 0.024500, 0.023500, 0.022500,
    0.021500, 0.020500, 0.019500, 0.018750, 0.018000, 0.017250, 0.016500, 0.015750, 0.015000, 0.014250,
    0.013500, 0.012750, 0.012000, 0.011250, 0.010500, 0.009750, 0.009000, 0.008250, 0.007500, 0.006750,
    0.006000, 0.005250, 0.004750, 0.004250, 0.003750, 0.003250, 0.002750, 0.002250, 0.001750, 0.001250,
];

pub struct FeePool<'f> {
    genesis_time_key: &'static [u8],
    storage_credit_pool_key: &'static [u8],
    drive: &'f Drive, // TODO: possibly use an RefCell and Arc to be able to reference it through Drive
}

pub struct Epoch<'e> {
    index: u16,
    key: [u8; 2],
    processing_fee_key: &'static [u8],
    storage_fee_key: &'static [u8],
    first_proposer_height_key: &'static [u8],
    proposers_key: &'static [u8],
    drive: &'e Drive,
}

impl<'f> FeePool<'f> {
    pub fn new(drive: &Drive) -> FeePool {
        FeePool {
            genesis_time_key: "g".as_bytes(),
            storage_credit_pool_key: "s".as_bytes(),
            drive,
        }
    }

    pub fn get_path<'a>() -> [&'a [u8]; 1] {
        [
            Into::<&[u8; 1]>::into(RootTree::Pools),
        ]
    }

    pub fn init(&self, genesis_time: f64, transaction: TransactionArg) -> Result<(), Error> {
        // We must encode and store the genesis time
        self.drive.grove
            .insert(FeePool::get_path(), self.genesis_time_key, Element::Item(genesis_time.to_le_bytes().to_vec()), transaction)
            .map_err(Error::GroveDB)?;

        // Update storage credit pool
        self.drive.grove
            .insert(FeePool::get_path(), self.storage_credit_pool_key, Element::Item(0f64.to_le_bytes().to_vec()), transaction)
            .map_err(Error::GroveDB)?;

        // We need to insert 50 years worth of epochs, with 20 epochs per year that's 1000 epochs
        // Except for the 0 one
        for i in 1..1000 {
            let epoch = Epoch::new(i, self.drive);
            epoch.init(transaction)?;
        }

        Ok(())
    }

    pub fn update_storage_pool_credit(&self, storage_fee: f64, transaction: TransactionArg) -> Result<(), Error> {
        match self.drive.grove.get(FeePool::get_path(), self.storage_credit_pool_key, transaction) {
            Ok(element) => {
                if let Element::Item(item) = element {
                    let credit = f64::from_le_bytes(item.as_slice().try_into().expect("expected item to be of length 8"));

                    // in case credit is set update it
                    self.drive.grove
                        .insert(FeePool::get_path(), self.storage_credit_pool_key, Element::Item((credit + storage_fee).to_le_bytes().to_vec()), transaction)
                        .map_err(Error::GroveDB)?;

                    Ok(())
                } else {
                    Err(Error::Drive(DriveError::CorruptedEpochElement("fee pool storage_credit_pool must be an item")))
                }
            }
            Err(err) => {
                match err {
                    grovedb::Error::PathKeyNotFound(e) => Err(Error::Drive(DriveError::CorruptedStorageCreditPoolPathElement(e))),
                    _ => Err(Error::Drive(DriveError::CorruptedCodeExecution("internal grovedb error")))
                }
            }
        }
    }

    pub fn get_storage_pool_credit(&self, transaction: TransactionArg) -> Result<Option<f64>, Error> {
        match self.drive.grove.get(FeePool::get_path(), self.storage_credit_pool_key, transaction) {
            Ok(element) => {
                if let Element::Item(item) = element {
                    let credit = f64::from_le_bytes(item.as_slice().try_into().expect("expected item to be of length 8"));

                    Ok(Some(credit))
                } else {
                    Err(Error::Drive(DriveError::CorruptedEpochElement("fee pool storage_credit_pool must be an item")))
                }
            }
            Err(err) => {
                match err {
                    grovedb::Error::PathKeyNotFound(_) => Ok(None),
                    _ => Err(Error::Drive(DriveError::CorruptedCodeExecution("internal grovedb error")))
                }
            }
        }
    }

    pub fn distribute_storage_distribution_pool(&self, epoch_index: u16, transaction: TransactionArg) -> Result<(), Error> {
        match self.get_storage_pool_credit(transaction)? {
            Some(mut credit) => {
                let mut year = 1;
                let mut epoch_of_the_year = 1;

                for index in epoch_index..epoch_index + 1000 {
                    let epoch = Epoch::new(index, self.drive);

                    let credit_distribution_percent = FEE_DISTRIBUTION_TABLE[year * (epoch_of_the_year - 1)];

                    let credit_share = credit * credit_distribution_percent;

                    epoch.update_storage_fee(credit_share, transaction)?;

                    credit -= credit_share;

                    epoch_of_the_year += 1;

                    if epoch_of_the_year > 20 {
                        year += 1;
                    }
                }

                self.update_storage_pool_credit(credit, transaction)?;
            }
            None => (),
        }

        Ok(())
    }

    pub fn process_epoch_change(&self, epoch_index: u16, first_proposer_block_height: u64, transaction: TransactionArg) -> Result<(), Error> {
        if epoch_index == 0 {
            return Err(Error::Drive(DriveError::CorruptedCodeExecution("epoch should never be 0")))
        }

        // create and init next thousandth epoch
        let next_thousandth_epoch = Epoch::new(epoch_index + 1000, self.drive);
        next_thousandth_epoch.init(transaction)?;

        // init first_proposer_block_height for an epoch `i`
        let epoch = Epoch::new(epoch_index, self.drive);
        epoch.update_first_proposer_block_height(first_proposer_block_height, transaction)?;

        // init processing_fee and proposers for an epoch `i + 1`
        let next_epoch = Epoch::new(epoch_index + 1, self.drive);
        next_epoch.update_processing_fee(0f64, transaction)?;
        next_epoch.update_proposers(vec!(), transaction)?;

        // distribute the storage fees
        self.distribute_storage_distribution_pool(epoch_index, transaction)?;

        Ok(())
    }

    pub fn distribute_st_fees(&self, epoch_index: u16, processing_fees: f64, storage_fees: f64, proposer_pro_tx_hash: [u8;32], transaction: TransactionArg) -> Result<(), Error> {
        if epoch_index == 0 {
            return Err(Error::Drive(DriveError::CorruptedCodeExecution("epoch should never be 0")))
        }

        // update processing_fee of an epoch `i + 1`
        let next_epoch = Epoch::new(epoch_index + 1, self.drive);
        next_epoch.update_processing_fee(processing_fees, transaction)?;

        // update storage credit pool
        self.update_storage_pool_credit(storage_fees, transaction)?;

        // update proposers
        next_epoch.update_proposers(vec!(proposer_pro_tx_hash), transaction)?;
        
        Ok(())
    }
}

impl<'e> Epoch<'e> {
    pub fn new(index: u16, drive: &Drive) -> Epoch {
        Epoch {
            index,
            key: index.to_le_bytes(),
            processing_fee_key: "p".as_bytes(),
            storage_fee_key: "s".as_bytes(),
            first_proposer_height_key: "c".as_bytes(),
            proposers_key: "m".as_bytes(),
            drive
        }
    }

    pub fn init(&self, transaction: TransactionArg) -> Result<(), Error> {
        if self.index == 0 {
            return Err(Error::Drive(DriveError::CorruptedCodeExecution("epoch index should never be set to 0")))
        }

        // init epoch tree
        self.drive.grove
            .insert(FeePool::get_path(), &self.key, Element::empty_tree(), transaction)
            .map_err(Error::GroveDB)?;

        // init storage fee item to 0 
        self.drive.grove
            .insert(self.get_path(), &self.storage_fee_key, Element::Item(0f64.to_le_bytes().to_vec()), transaction)
            .map_err(Error::GroveDB)
    }

    pub fn get_path(&self) -> [&[u8]; 2] {
        [
            FeePool::get_path()[0],
            &self.key,
        ]
    }

    pub fn get_proposers_path(&self) -> [&[u8]; 3] {
        [
            FeePool::get_path()[0],
            &self.key,
            &self.proposers_key,
        ]
    }

    pub fn update_first_proposer_block_height(&self, first_proposer_block_height: u64, transaction: TransactionArg) -> Result<(), Error> {
        self.drive.grove
            .insert(self.get_path(), &self.first_proposer_height_key, Element::Item(first_proposer_block_height.to_le_bytes().to_vec()), transaction)
            .map_err(Error::GroveDB)
    }

    pub fn update_processing_fee(&self, processing_fee: f64, transaction: TransactionArg) -> Result<(), Error> {
        match self.drive.grove.get(self.get_path(), self.processing_fee_key, transaction) {
            Ok(element) => {
                if let Element::Item(item) = element {
                    let fee = f64::from_le_bytes(item.as_slice().try_into().expect("expected item to be of length 8"));

                    // in case fee is set updated it
                    self.drive.grove
                        .insert(self.get_path(), self.processing_fee_key, Element::Item((fee + processing_fee).to_le_bytes().to_vec()), transaction)
                        .map_err(Error::GroveDB)?;

                    Ok(())
                } else {
                    Err(Error::Drive(DriveError::CorruptedEpochElement("epoch processing_fee must be an item")))
                }
            }
            Err(err) => {
                match err {
                    grovedb::Error::PathKeyNotFound(_) => {
                        // if fee path was not found init it with 0
                        self.drive.grove
                            .insert(self.get_path(), self.processing_fee_key, Element::Item(processing_fee.to_le_bytes().to_vec()), transaction)
                            .map_err(Error::GroveDB)
                    },
                    _ => Err(Error::Drive(DriveError::CorruptedCodeExecution("internal grovedb error")))
                }
            }
        }
    }

    pub fn update_storage_fee(&self, storage_fee: f64, transaction: TransactionArg) -> Result<(), Error> {
        match self.drive.grove.get(self.get_path(), self.storage_fee_key, transaction) {
            Ok(element) => {
                if let Element::Item(item) = element {
                    let fee = f64::from_le_bytes(item.as_slice().try_into().expect("expected item to be of length 8"));

                    // in case fee is set updated it
                    self.drive.grove
                        .insert(self.get_path(), self.storage_fee_key, Element::Item((fee + storage_fee).to_le_bytes().to_vec()), transaction)
                        .map_err(Error::GroveDB)?;

                    Ok(())
                } else {
                    Err(Error::Drive(DriveError::CorruptedEpochElement("epoch storage_fee must be an item")))
                }
            }
            Err(err) => {
                match err {
                    grovedb::Error::PathKeyNotFound(_) => {
                        // if fee path was not found init it with 0
                        self.drive.grove
                            .insert(self.get_path(), self.processing_fee_key, Element::Item(storage_fee.to_le_bytes().to_vec()), transaction)
                            .map_err(Error::GroveDB)
                    },
                    _ => Err(Error::Drive(DriveError::CorruptedCodeExecution("internal grovedb error")))
                }
            }
        }
    }

    pub fn get_proposer_block_count(&self, proposer_tx_hash: &[u8; 32], transaction: TransactionArg) -> Result<Option<u64>, Error> {
        match self.drive.grove.get(self.get_proposers_path(), proposer_tx_hash, transaction) {
            Ok(element) => {
                if let Element::Item(item) = element {
                    Ok(Some(u64::from_le_bytes(item.as_slice().try_into().expect("invalid item length"))))
                } else {
                    Err(Error::Drive(DriveError::CorruptedEpochElement("epoch proposer block count must be an item")))
                }
            }
            Err(err) => {
                match err {
                    grovedb::Error::PathKeyNotFound(_) => Ok(None),
                    _ => Err(Error::Drive(DriveError::CorruptedCodeExecution("internal grovedb error")))
                }
            }
        }
    }

    pub fn update_proposers(&self, proposer_tx_hashes: Vec<[u8;32]>, transaction: TransactionArg) -> Result<(), Error> {
        match self.drive.grove.get(self.get_path(), self.proposers_key, transaction) {
            Ok(element) => {
                if let Element::Tree(_) = element {
                    for (_, proposer_tx_hash) in proposer_tx_hashes.iter().enumerate() {
                        match self.get_proposer_block_count(proposer_tx_hash, transaction) {
                            Ok(Some(block_count)) => {
                                // update block count
                                self.drive.grove
                                    .insert(self.get_proposers_path(), proposer_tx_hash, Element::Item((block_count + 1).to_le_bytes().to_vec()), transaction)
                                    .map_err(Error::GroveDB)?;
                            },
                            Ok(None) => {
                                // insert new hash
                                self.drive.grove
                                    .insert(self.get_proposers_path(), proposer_tx_hash, Element::Item(1u64.to_le_bytes().to_vec()), transaction)
                                    .map_err(Error::GroveDB)?;
                            },
                            Err(_) => {
                                return Err(Error::Drive(DriveError::CorruptedCodeExecution("internal grovedb error")));
                            }
                        }
                    }
                    
                    Ok(())
                } else {
                    Err(Error::Drive(DriveError::CorruptedEpochElement("epoch proposer must be a tree")))
                }
            }
            Err(err) => {
                match err {
                    grovedb::Error::PathKeyNotFound(_) => {
                        // if fee path was not found init it
                        self.drive.grove
                            .insert(self.get_path(), self.proposers_key, Element::empty_tree(), transaction)
                            .map_err(Error::GroveDB)
                    },
                    _ => Err(Error::Drive(DriveError::CorruptedCodeExecution("internal grovedb error")))
                }
            }
        }
    }
}

impl Drive {
    pub fn init_fee_pool(&self, genesis_time: f64, transaction: TransactionArg) -> Result<(), Error> {
        let fee_pool = FeePool::new(self);

        // initialize the pool with epochs
        fee_pool.init(genesis_time, transaction)?;

        Ok(())
    }

    pub fn process_block(&self, block_height: u64, block_time: f64, proposer_pro_tx_hash: [u8;32], processing_fees: u64, storage_fees: u64, transaction: TransactionArg)  -> Result<(), Error> {
        // If block time is over the epoch time then we need to change epochs

        // Storage fees should go into the storage distribution pool

        // Processing fees should be added to the next epoch distribution pool
        //self.proposer_proposed_block_in_epoch(epoch_index, proposer_pro_tx_hash, processing_fees, transaction)
        Ok(())
    }

    // fn split_storage_fees_for_distribution(fees: u64) -> Vec<u64> {
    //     let mut distribution_fee : Vec<u64> = vec![];
    //     // todo()!
    //     // We need to split the distribution fee based on the values provided in the DIP
    //     distribution_fee
    // }

    // fn distribute_storage_distribution_pool(&self, current_epoch_index: u16, transaction: TransactionArg) -> Result<(), Error> {
    //     // First we need to get the total value of the storage distribution pool

    //     // Then we need to split the values by epoch years

    //     // Then we need to add the values to the storage credits of each epoch, 20 epochs at a time
    //     Ok(())
    // }

    // fn proposer_proposed_block_in_epoch(&self, epoch_index: u16, proposer_pro_tx_hash: [u8;32], processing_fees: u64, transaction: TransactionArg)  -> Result<(), Error> {
    //     Ok(())
    // }
}