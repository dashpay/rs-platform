use thiserror::Error;

use crate::{consensus::ConsensusError, identity::script::Script};

#[derive(Error, Debug, Clone, PartialEq, Eq)]
#[error("Output script {output_script:?} must be either p2pkh or p2sh")]
pub struct InvalidIdentityCreditWithdrawalTransitionOutputScriptError {
    output_script: Script,
}

impl InvalidIdentityCreditWithdrawalTransitionOutputScriptError {
    pub fn new(output_script: Script) -> Self {
        Self { output_script }
    }

    pub fn output_script(&self) -> Script {
        self.output_script.clone()
    }
}

impl From<InvalidIdentityCreditWithdrawalTransitionOutputScriptError> for ConsensusError {
    fn from(err: InvalidIdentityCreditWithdrawalTransitionOutputScriptError) -> Self {
        Self::InvalidIdentityCreditWithdrawalTransitionOutputScriptError(err)
    }
}
