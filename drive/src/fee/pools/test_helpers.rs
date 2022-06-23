use crate::drive::Drive;
use crate::fee::pools::fee_pools::FeePools;
use grovedb::Transaction;
use tempfile::TempDir;

pub struct TestHelperOptions {
    pub init_fee_pools: bool,
}

impl Default for TestHelperOptions {
    fn default() -> TestHelperOptions {
        TestHelperOptions {
            init_fee_pools: true,
        }
    }
}

pub fn setup_drive() -> Drive {
    let tmp_dir = TempDir::new().unwrap();
    let drive: Drive = Drive::open(tmp_dir).expect("to open Drive successfully");

    drive
}

pub fn setup_fee_pools<'a>(
    drive: &'a Drive,
    options: Option<TestHelperOptions>,
) -> (Transaction<'a>, FeePools) {
    let options = options.unwrap_or(TestHelperOptions::default());

    drive
        .create_root_tree(None)
        .expect("to create root tree successfully");

    let transaction = drive.grove.start_transaction();

    let fee_pools = FeePools::new();

    if options.init_fee_pools {
        fee_pools
            .init(&drive, Some(&transaction))
            .expect("to init fee pools");
    }

    (transaction, fee_pools)
}
