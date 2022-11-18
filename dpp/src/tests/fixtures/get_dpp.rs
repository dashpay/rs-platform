use crate::{dash_platform_protocol::DPPOptions, state_repository::MockStateRepositoryLike, DashPlatformProtocol, NativeBlsValidator};

// TODO creation of DPP object for testing needs to be improved
pub fn get_dpp() -> DashPlatformProtocol<MockStateRepositoryLike, NativeBlsValidator> {
    DashPlatformProtocol::new(
        DPPOptions {
            current_protocol_version: None,
        },
        MockStateRepositoryLike::new(),
        NativeBlsValidator::default()
    )
    .unwrap()
}
