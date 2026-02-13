use crate::types::{Address, B256};

/// Internal implementation of the PayloadAttributes event
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct PayloadAttributes(reth::rpc::types::beacon::events::PayloadAttributesData);

impl From<reth::rpc::types::beacon::events::PayloadAttributesData> for PayloadAttributes {
    fn from(data: reth::rpc::types::beacon::events::PayloadAttributesData) -> Self {
        Self(data)
    }
}

impl PayloadAttributes {
    /// Returns the proposal slot of the event.
    pub fn proposal_slot(&self) -> u64 {
        self.0.proposal_slot
    }
    /// Returns the parent block number of the event.
    pub fn parent_block_number(&self) -> u64 {
        self.0.parent_block_number
    }
    /// returns the parent block hash of the event.
    pub fn parent_hash(&self) -> B256 {
        self.0.parent_block_hash.into()
    }
    /// Returns the suggested fee recipient of the event.
    pub fn suggested_fee_recipient(&self) -> Address {
        Address::from(self.0.payload_attributes.suggested_fee_recipient)
    }
    /// Returns the prev_randao of the event.
    pub fn prev_randao(&self) -> B256 {
        B256::from(self.0.payload_attributes.prev_randao)
    }

    /// Validates the bid against the payload attributes.
    ///
    /// Checks if the provided slot matches the proposal slot, the fee recipient is valid,
    /// and the `prev_randao` value matches. Returns `None` if all validations pass,
    /// or an error message indicating the reason for failure.
    pub fn validate_bid(&self, slot: u64, prev_randao: B256) -> Result<(), ValidationErrors> {
        if self.0.proposal_slot != slot {
            return Err(ValidationErrors::Slot);
        }

        if self.prev_randao() != prev_randao {
            return Err(ValidationErrors::PrevRandao);
        }
        Ok(())
    }
}

/// Errors that can occur during the validation of a bid against the payload attributes.
#[derive(Debug, PartialEq, Eq, Clone, thiserror::Error)]
pub enum ValidationErrors {
    /// The slot does not match the payload attributes proposal slot
    #[error("Payload attributes: Invalid slot.")]
    Slot,
    /// The prev_randao value does not match the payload attributes prevrandao.
    #[error("Payload attributes: Invalid prev_randao.")]
    PrevRandao,
}
