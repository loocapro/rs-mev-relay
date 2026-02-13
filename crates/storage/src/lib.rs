//! # Storage Crate
//!
//! The `storage` crate provides an abstract trait interface for storage operations, allowing for flexible storage backends.
//! This design enables developers to implement the trait for different storage mechanisms without changing the client code that uses this storage functionality.
//! The primary goal is to decouple the storage operations from the specific details of underlying storage systems, such as databases, file systems, or in-memory storage, thereby increasing modularity and the ability to test components independently.
//!
#![deny(unused_must_use, rust_2018_idioms)]

use in_memory::ValidatorRegistrations;
use relay_primitives::{
    beacon::{registrations::ValidatorRegistration, BlindedBlockResponse},
    blst::{public_key::BlsPublicKey, signer::ForkDatas},
    storage::{
        bid_submission::BidSubmission, head_slot::HeadSlot, payload_attributes::PayloadAttributes,
        validator::Validator,
    },
    types::B256,
};
use std::sync::Arc;

/// In memory storage module.
pub mod in_memory;
/// Helper trait to implement different types of storage.
pub trait Storage {
    /// The fork data identifies che current fork version of the chain and the current genesis validators root.
    fn fork_data(&self) -> ForkDatas;
    /// Insert the latest head slot once the beacon node stream emits a new head event.
    fn set_head_slot(&self, slot: HeadSlot);
    /// Reads the latest head slot, used for payload validation.
    fn read_head_slot(&self) -> HeadSlot;
    /// Inserts the latest payload attributes once the beacon node stream emits a new payload attributes event.
    fn set_payload_attributes(&self, payload_attr: PayloadAttributes);
    /// Reads the latest payload attributes, used for payload validation.
    fn read_payload_attributes(&self) -> PayloadAttributes;
    /// Inserts the latest validator registrations, consumed by the proposer API on POST /eth/v1/beacon/validators.
    /// This is called through mev-boost on start up of a validator.
    fn set_validator_registration(&self, key: BlsPublicKey, registration: ValidatorRegistration);

    /// Inserts the latest validator registrations, consumed by the proposer API on POST /eth/v1/beacon/validators.
    fn set_validator_registrations(&self, registrations: ValidatorRegistrations);
    /// Checks if the validator registrations are empty.
    fn empty_validator_regs(&self) -> bool;
    /// Reads the latest validator registrations.
    fn read_validator_registration(&self, key: &BlsPublicKey) -> Option<ValidatorRegistration>;

    /// Inserts the latest proposer duties. It is triggered every epoch and contains 2 epochs worth of duties (64).
    fn set_proposer_duties(&self, duties: Vec<Validator>);
    /// Reads the latest proposer duties.
    fn read_proposer_duties(&self) -> Vec<Validator>;
    /// Checks if a given key is whitelisted as a builder.
    fn is_whitelisted_builder(&self, key: &BlsPublicKey) -> bool;
    /// Given a slot, returns the validator registered with the relay that has the duty to propose a block for that slot.
    fn find_duty_by_slot(&self, slot: u64) -> Option<Validator>;
    /// Inserts the best bid submission for a given slot.
    fn set_best_bid(&self, slot: u64, bid: Arc<BidSubmission>);
    /// Reads the best bid submission for a given slot.
    fn read_best_bid(
        &self,
        slot: u64,
        parent_hash: B256,
        validator: BlsPublicKey,
    ) -> Option<Arc<BidSubmission>>;
    /// Best bid by block number.
    fn read_best_bid_by_block_number(&self, block_number: u64) -> Option<Arc<BidSubmission>>;
    /// Best bid by slot.
    fn read_best_bid_by_slot(&self, slot: u64) -> Option<Arc<BidSubmission>>;
    /// Inserts the blinded block response for a given proposer.
    fn set_blinded_block_response(
        &self,
        proposer: BlsPublicKey,
        blinded_block_resp: BlindedBlockResponse,
    );
    /// Reads the blinded block response for a given proposer.
    fn read_blinded_block_response(&self, proposer: BlsPublicKey) -> Option<BlindedBlockResponse>;
    /// Inserts the delivered blocks for a given block hash.
    fn set_delivered_blocks(&self, block_hash: B256);
    /// Checks if a given block hash has been delivered.
    fn record_relayed_block(&self, block_hash: B256);
}
