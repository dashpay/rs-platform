use grovedb::{Element, TransactionArg, Query, PathQuery, SizedQuery};

use crate::drive::{Drive, RootTree, Document};
use crate::identity::{Identity};
use crate::error::drive::DriveError;
use crate::error::Error;
use crate::common::value_to_cbor;

use serde_json::json;
use chrono::{Utc};

const FEE_DISTRIBUTION_TABLE: [f64; 50] = [
    0.050000, 0.048000, 0.046000, 0.044000, 0.042000, 0.040000, 0.038500, 0.037000, 0.035500, 0.034000,
    0.032500, 0.031000, 0.029500, 0.028500, 0.027500, 0.026500, 0.025500, 0.024500, 0.023500, 0.022500,
    0.021500, 0.020500, 0.019500, 0.018750, 0.018000, 0.017250, 0.016500, 0.015750, 0.015000, 0.014250,
    0.013500, 0.012750, 0.012000, 0.011250, 0.010500, 0.009750, 0.009000, 0.008250, 0.007500, 0.006750,
    0.006000, 0.005250, 0.004750, 0.004250, 0.003750, 0.003250, 0.002750, 0.002250, 0.001750, 0.001250,
];

const MN_REWARD_SHARES_CONTRACT_ID: [u8; 32] = [ 
    0x0c, 0xac, 0xe2, 0x05, 0x24, 0x66, 0x93, 0xa7,
    0xc8, 0x15, 0x65, 0x23, 0x62, 0x0d, 0xaa, 0x93,
    0x7d, 0x2f, 0x22, 0x47, 0x93, 0x44, 0x63, 0xee,
    0xb0, 0x1f, 0xf7, 0x21, 0x95, 0x90, 0x95, 0x8c,
];

const MN_REWARD_SHARES_DOCUMENT_TYPE: &'static str = "rewardShare";

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

    pub fn init(&self, genesis_time: i64, transaction: TransactionArg) -> Result<(), Error> {
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

    pub fn get_genesis_time(&self, transaction: TransactionArg) -> Result<i64, Error> {
        match self.drive.grove.get(FeePool::get_path(), self.genesis_time_key, transaction) {
            Ok(element) => {
                if let Element::Item(item) = element {
                    let genesis_time = i64::from_le_bytes(item.as_slice().try_into().expect("invalid item length in bytes"));

                    Ok(genesis_time)
                } else {
                    Err(Error::Drive(DriveError::CorruptedEpochElement("fee pool genesis_time must be an item")))
                }
            }
            Err(err) => {
                match err {
                    grovedb::Error::PathKeyNotFound(e) => Err(Error::Drive(DriveError::CorruptedGenesisElementPath(e))),
                    _ => Err(Error::Drive(DriveError::CorruptedCodeExecution("internal grovedb error")))
                }
            }
        }
    }

    pub fn get_current_epoch_index(&self, block_time: i64, transaction: TransactionArg) -> Result<(u16, bool), Error> {
        let genesis_time = self.get_genesis_time(transaction)?;

        let epoch_index = (block_time - genesis_time) as f64 / 1576800000.0;
        let epoch_index_floored = epoch_index.floor();

        let is_epoch_change = false; // TODO: find a proper way of knowing epoch change

        Ok((epoch_index_floored as u16, is_epoch_change))
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
                    if epoch_index == 0 {
                        return Err(Error::Drive(DriveError::CorruptedCodeExecution("epoch should never be 0")))
                    }

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

    pub fn get_oldest_epoch(&self, epoch_index: u16, transaction: TransactionArg) -> Result<Epoch, Error> {
        if epoch_index == 1 {
            return Ok(Epoch::new(epoch_index, self.drive));
        }

        let epoch = Epoch::new(epoch_index, self.drive);

        if epoch.is_proposers_tree_empty(transaction)? {
            return Ok(epoch)
        }

        self.get_oldest_epoch(epoch_index - 1, transaction)
    }  

    pub fn distribute_fees_to_proposers(&self, epoch_index: u16, block_height: u64, transaction: TransactionArg) -> Result<(), Error> {
        if epoch_index == 0 {
            return Err(Error::Drive(DriveError::CorruptedCodeExecution("epoch should never be 0")))
        }

        let epoch = self.get_oldest_epoch(epoch_index, transaction)?;

        let proposers_limit: u16 = (epoch_index - epoch.index) * 50;
        
        let credit_value = epoch.get_credit_value(transaction)?;

        let epoch_block_count = block_height - epoch.get_first_proposed_block_height(transaction)?;

        for (proposer_tx_hash, proposed_block_count) in epoch.get_proposers(proposers_limit, transaction)? {
            let query_json = json!({
                "where": [
                    ["$ownerId", "==", bs58::encode(proposer_tx_hash).into_string()]
                ],
            });
    
            let query_cbor = value_to_cbor(query_json, None);

            let (document_cbors, _, _) = self.drive.query_documents(
                &query_cbor, 
                MN_REWARD_SHARES_CONTRACT_ID, 
                MN_REWARD_SHARES_DOCUMENT_TYPE, 
                transaction,
            )?;

            let documents: Vec<Document> = document_cbors
                .iter()
                .map(|cbor| Document::from_cbor(cbor, None, None).expect("should be able to deserialize cbor"))
                .collect();

            for document in documents {
                let pay_to_id = document.properties.get("payToId")
                    .expect("should be able to get payToId")
                    .as_bytes()
                    .expect("shoul be able to get as bytes");

                let mut identity = self.drive.get_identity(pay_to_id, transaction)?.expect("identity to be found");

                let share_percentage_integer: u64 = document.properties.get("percentage")
                    .expect("should be able to get percentage")
                    .as_integer()
                    .expect("should be an integer")
                    .try_into()
                    .expect("should be able to convert to u64");

                let share_percentage: f64 = share_percentage_integer as f64 / 100.0;

                let reward: f64 = ((credit_value * proposed_block_count as f64 * share_percentage) / epoch_block_count as f64).floor();

                identity.balance += reward as u64;

                self.drive.insert_identity_cbor(
                    Some(pay_to_id), 
                    identity.to_cbor(),
                    transaction,
                )?;
            }
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

    pub fn get_storage_credit(&self, transaction: TransactionArg) -> Result<f64, Error> {
        match self.drive.grove.get(self.get_path(), self.storage_fee_key, transaction) {
            Ok(element) => {
                if let Element::Item(item) = element {
                    Ok(f64::from_le_bytes(item.as_slice().try_into().expect("invalid item length")))
                } else {
                    Err(Error::Drive(DriveError::CorruptedEpochElement("epoch storage fee must be an item")))
                }
            }
            Err(err) => {
                match err {
                    grovedb::Error::PathKeyNotFound(e) => Err(Error::Drive(DriveError::CorruptedStorageCreditPathElement(e))),
                    _ => Err(Error::Drive(DriveError::CorruptedCodeExecution("internal grovedb error")))
                }
            }
        }
    }

    pub fn get_processing_credit(&self, transaction: TransactionArg) -> Result<f64, Error> {
        match self.drive.grove.get(self.get_path(), self.processing_fee_key, transaction) {
            Ok(element) => {
                if let Element::Item(item) = element {
                    Ok(f64::from_le_bytes(item.as_slice().try_into().expect("invalid item length")))
                } else {
                    Err(Error::Drive(DriveError::CorruptedEpochElement("epoch processing fee must be an item")))
                }
            }
            Err(err) => {
                match err {
                    grovedb::Error::PathKeyNotFound(e) => Err(Error::Drive(DriveError::CorruptedProcessingCreditPathElement(e))),
                    _ => Err(Error::Drive(DriveError::CorruptedCodeExecution("internal grovedb error")))
                }
            }
        }
    }

    pub fn get_credit_value(&self, transaction: TransactionArg) -> Result<f64, Error> {
        let storage_credit = self.get_storage_credit(transaction)?;

        let processing_credit = self.get_processing_credit(transaction)?;

        Ok(storage_credit + processing_credit)
    }

    pub fn get_first_proposed_block_height(&self, transaction: TransactionArg) -> Result<u64, Error> {
        match self.drive.grove.get(self.get_path(), self.first_proposer_height_key, transaction) {
            Ok(element) => {
                if let Element::Item(item) = element {
                    Ok(u64::from_le_bytes(item.as_slice().try_into().expect("invalid item length")))
                } else {
                    Err(Error::Drive(DriveError::CorruptedEpochElement("epoch first proposed block height must be an item")))
                }
            }
            Err(err) => {
                match err {
                    grovedb::Error::PathKeyNotFound(e) => Err(Error::Drive(DriveError::CorruptedProposersCountPathElement(e))),
                    _ => Err(Error::Drive(DriveError::CorruptedCodeExecution("internal grovedb error")))
                }
            }
        }
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

    pub fn is_proposers_tree_empty(&self, transaction: TransactionArg) -> Result<bool, Error> {
        match self.drive.grove.is_empty_tree(self.get_proposers_path(), transaction) {
            Ok(result) => Ok(result),
            Err(err) => {
                match err {
                    grovedb::Error::PathKeyNotFound(_) => Ok(true),
                    _ => Err(Error::Drive(DriveError::CorruptedCodeExecution("internal grovedb error")))
                }
            }
        }
    }

    pub fn get_proposers(&self, limit: u16, transaction: TransactionArg) -> Result<Vec<(Vec<u8>, u64)>, Error> {
        let path_as_vec: Vec<Vec<u8>> = self.get_proposers_path()
            .iter()
            .map(|slice| slice.to_vec())
            .collect();

        let path_query = PathQuery::new(
            path_as_vec,
            SizedQuery::new(
                Query::new(),
                Some(limit),
                None,
            ), 
        );

        let path_queries = [&path_query];

        match self.drive.grove.get_path_queries(&path_queries, transaction) {
            //TODO: find a way on how to get pro_tx_hash (which is a key) here
            Ok(elements) => {
                let result: Vec<(Vec<u8>, u64)> = elements.into_iter().map(|e| (vec!(), u64::from_le_bytes(e.try_into().expect("to have length of 8")))).collect();
                Ok(result)
            },
            Err(err) => {
                match err {
                    grovedb::Error::InvalidQuery(e) => Err(Error::Drive(DriveError::CorruptedProposersQuery(String::from(e)))),
                    _ => Err(Error::Drive(DriveError::CorruptedCodeExecution("internal grovedb error")))
                }
            }
        }
    }
}

impl Drive {
    pub fn init_fee_pool(&self, genesis_time: i64, transaction: TransactionArg) -> Result<(), Error> {
        let fee_pool = FeePool::new(self);

        // initialize the pool with epochs
        fee_pool.init(genesis_time, transaction)?;

        Ok(())
    }

    pub fn process_block(&self, block_height: u64, block_time: i64, proposer_pro_tx_hash: [u8;32], processing_fees: f64, storage_fees: f64, transaction: TransactionArg)  -> Result<(), Error> {
        if block_height == 1 {
            let genesis_time = Utc::now().timestamp();
            self.init_fee_pool(genesis_time, transaction)?;
        }

        let fee_pool = FeePool::new(self);

        let (epoch_index, is_epoch_change) = fee_pool.get_current_epoch_index(block_time, transaction)?;

        if is_epoch_change {
            fee_pool.distribute_storage_distribution_pool(epoch_index, transaction)?;
        }

        fee_pool.distribute_st_fees(
            epoch_index, 
            processing_fees, 
            storage_fees, 
            proposer_pro_tx_hash, 
            transaction,
        )?;

        fee_pool.distribute_fees_to_proposers(epoch_index, block_height, transaction)?;

        Ok(())
    }

    pub fn get_identity(&self, id: &[u8], transaction: TransactionArg) -> Result<Option<Identity>, Error> {
        match self.grove.get([Into::<&[u8; 1]>::into(RootTree::Identities).as_slice()], id, transaction) {
            Ok(element) => {
                if let Element::Item(identity_cbor) = element {
                    let identity = Identity::from_cbor(identity_cbor.as_slice())?;

                    Ok(Some(identity))
                } else {
                    Err(Error::Drive(DriveError::CorruptedEpochElement("identity must be an item")))
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
}

#[cfg(test)]
mod tests {
    use crate::drive::Drive;
    use crate::drive::pools::{FeePool, Epoch};
    use crate::error::Error;
    use crate::error::drive::DriveError;

    use tempfile::TempDir;

    #[test]
    fn test_fee_pool_new() {
        let tmp_dir = TempDir::new().unwrap();
        let drive: Drive = Drive::open(tmp_dir).expect("expected to open Drive successfully");

        let fee_pool = FeePool::new(&drive);

        assert_eq!(fee_pool.genesis_time_key, "g".as_bytes());
        assert_eq!(fee_pool.storage_credit_pool_key, "s".as_bytes());
    }

    #[test]
    fn test_fee_pool_init() {
        let tmp_dir = TempDir::new().unwrap();
        let drive: Drive = Drive::open(tmp_dir).expect("expected to open Drive successfully");

        let db_transaction = drive.grove.start_transaction();

        let fee_pool = FeePool::new(&drive);

        fee_pool.init(1654622858842, Some(&db_transaction)).expect("should init fee pool");
    }

    #[test]
    fn test_epoch_new() {
        let tmp_dir = TempDir::new().unwrap();
        let drive: Drive = Drive::open(tmp_dir).expect("expected to open Drive successfully");

        let epoch = Epoch::new(1u16, &drive);

        assert_eq!(epoch.index, 1u16);
        assert_eq!(epoch.key, 1u16.to_le_bytes());
        assert_eq!(epoch.processing_fee_key, "p".as_bytes());
        assert_eq!(epoch.storage_fee_key, "s".as_bytes());
        assert_eq!(epoch.first_proposer_height_key, "c".as_bytes());
        assert_eq!(epoch.proposers_key, "m".as_bytes());
    }

    #[test]
    fn test_epoch_init() {
        let tmp_dir = TempDir::new().unwrap();
        let drive: Drive = Drive::open(tmp_dir).expect("expected to open Drive successfully");

        let db_transaction = drive.grove.start_transaction();

        let epoch = Epoch::new(0u16, &drive);

        match epoch.init(Some(&db_transaction)) {
            Ok(_) => assert!(false, "should return an error for 0 based epoch"),
            Err(e) => {
                // TODO: validate error
                assert!(true);
            },
        };

        let epoch = Epoch::new(1u16, &drive);

        epoch.init(Some(&db_transaction)).expect("should init epoch with index 1");
    }
}