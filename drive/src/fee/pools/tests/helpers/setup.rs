use crate::drive::Drive;
use crate::fee::pools::fee_pools::FeePools;
use grovedb::Transaction;
use tempfile::TempDir;

pub struct SetupFeePoolsOptions {
    pub create_fee_pool_trees: bool,
}

impl Default for SetupFeePoolsOptions {
    fn default() -> SetupFeePoolsOptions {
        SetupFeePoolsOptions {
            create_fee_pool_trees: true,
        }
    }
}

pub fn setup_drive() -> Drive {
    let tmp_dir = TempDir::new().unwrap();
    let drive: Drive = Drive::open(tmp_dir, None).expect("should open Drive successfully");

    drive
}

pub fn setup_fee_pools<'a>(
    drive: &'a Drive,
    options: Option<SetupFeePoolsOptions>,
) -> (Transaction<'a>, FeePools) {
    let options = options.unwrap_or(SetupFeePoolsOptions::default());

    drive
        .create_root_tree(None)
        .expect("should create root tree successfully");

    let transaction = drive.grove.start_transaction();

    let fee_pools = FeePools::new();

    if options.create_fee_pool_trees {
        fee_pools
            .create_fee_pool_trees(&drive)
            .expect("should init fee pools");

        drive
            .apply_current_batch(true, Some(&transaction))
            .expect("should apply batch");
    }

    (transaction, fee_pools)
}
