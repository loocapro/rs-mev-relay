use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use serde_with::DisplayFromStr;

use crate::blst::public_key::BlsPublicKey;

/// Beacon node response from  eth/v1/beacon/states/{state_id}/validators/{validator_id}
#[derive(Debug, Serialize, Deserialize)]
pub struct KnownValidatorResponse {
    execution_optimistic: bool,
    finalized: bool,
    /// List of validators
    pub data: ValidatorEntry,
}

impl KnownValidatorResponse {
    /// Returns true if the validator is active
    pub fn is_active(&self) -> bool {
        self.data.status.contains("active")
    }
}

/// Validator entry in the response from /eth/v1/beacon/states/head/validators
#[serde_as]
#[derive(Debug, Serialize, Default, Deserialize, Clone)]
pub struct ValidatorEntry {
    /// Index
    #[serde_as(as = "DisplayFromStr")]
    pub index: u64,
    balance: String,
    /// The status of then consensus validator
    status: String,
    /// Validator
    pub validator: Validator,
}

/// Validator in the response from /eth/v1/beacon/states/head/validators
#[serde_as]
#[derive(Debug, Serialize, Default, Deserialize, Eq, Clone, PartialEq)]
pub struct Validator {
    /// Public key
    pub pubkey: BlsPublicKey,
    withdrawal_credentials: String,
    effective_balance: String,
    slashed: bool,
    #[serde_as(as = "DisplayFromStr")]
    activation_eligibility_epoch: u64,
    #[serde_as(as = "DisplayFromStr")]
    activation_epoch: u64,
    #[serde_as(as = "DisplayFromStr")]
    exit_epoch: u64,
    #[serde_as(as = "DisplayFromStr")]
    withdrawable_epoch: u64,
}
