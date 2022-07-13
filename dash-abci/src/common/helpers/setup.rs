use crate::platform::Platform;

pub fn setup_platform() -> Platform {
    let tmp_dir = TempDir::new().unwrap();
    let drive: Platform = Platform::open(tmp_dir, None).expect("should open Platform successfully");

    drive
}

pub fn setup_platform_with_initial_state_structure<'a>() -> (Platform, Transaction<'a>) {
    let platform = setup_platform();
    let transaction = platform.drive.grove.start_transaction();
    platform
        .drive
        .create_initial_state_structure(None)
        .expect("should create root tree successfully");

    (platform, transaction)
}
