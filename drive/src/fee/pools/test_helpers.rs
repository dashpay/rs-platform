use crate::drive::Drive;
use crate::fee::pools::fee_pools::FeePools;
use grovedb::Transaction;
use tempfile::TempDir;

pub fn setup_drive() -> Drive {
    let tmp_dir = TempDir::new().unwrap();
    let drive: Drive = Drive::open(tmp_dir).expect("to open Drive successfully");

    drive
}

pub fn setup_fee_pools<'a>(drive: &'a Drive) -> (Transaction<'a>, FeePools) {
    drive
        .create_root_tree(None)
        .expect("to create root tree successfully");

    let transaction = drive.grove.start_transaction();

    let fee_pools = FeePools::new();

    fee_pools
        .init(&drive, Some(&transaction))
        .expect("to init fee pools");

    (transaction, fee_pools)
}
