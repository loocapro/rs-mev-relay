use crate::beacon::registrations::ValidatorRegistration;
use crate::types::Address;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use serde_with::DisplayFromStr;
// #[derive(Debug, Clone, PartialEq, Eq)]
// /// Internal implementation of the Validator type
// pub struct Validator(reth::rpc::types::relay::Validator);

/// Represents an entry of the `/relay/v1/builder/validators` endpoint
#[serde_as]
#[derive(PartialEq, Debug, Serialize, Deserialize, Clone)]
pub struct Validator {
    /// The slot number for the validator entry.
    #[serde_as(as = "DisplayFromStr")]
    pub slot: u64,
    /// The index of the validator.
    #[serde_as(as = "DisplayFromStr")]
    pub validator_index: u64,
    /// Details of the validator registration.
    pub entry: ValidatorRegistration,
}

impl Validator {
    /// Validates a fee recipient over the current fee_recipient.
    pub fn invalid_fee_recipient(&self, fee_recipient: Address) -> bool {
        self.entry.message.fee_recipient != fee_recipient
    }
}
