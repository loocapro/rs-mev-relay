//! # Beacon Crate
//!
//! The `beacon` crate provides an interface for interacting with beacon nodes, facilitating both
//! server-side events and client API calls. It aims to abstract the complexity of beacon node
//! interactions into a streamlined, Rust-centric API, enabling developers to easily integrate
//! beacon node functionalities into their applications.

#![warn(missing_docs, unreachable_pub, rustdoc::all)]
#![deny(unused_must_use, rust_2018_idioms)]

use api::SubmissionType;
use events::stream::BeaconEventsStream;
use eyre::Result;
use futures::Future;
use relay_primitives::{
    beacon::{
        beacon_block::SignedBeaconBlockContent, fork::GetForkResponse,
        known_validator::KnownValidatorResponse, proposer_duties::ProposerDutiesResponse,
        sync_status::BeaconSyncStatusResponse,
    },
    blst::public_key::BlsPublicKey,
};

/// Beacon apis interface.
pub mod api;
/// Beacon events interface.
pub mod events;

/// Long running service to interact with beacon nodes in an async manner.
pub mod service;

/// Helper trait to abstract beacon node api calls.
pub trait BeaconApi: Send + Sync {
    /// The configuration type for the beacon api.
    type Config;
    /// Constructor method to create a new beacon api instance.
    fn new(config: Self::Config) -> Self;
    /// Returns the sync status of the beacon node.
    fn sync_status(&self) -> impl Future<Output = Result<BeaconSyncStatusResponse>> + Send;
    /// Returns the proposer duties of the beacon node for a given epoch.
    fn proposer_duties(
        &self,
        epoch: u64,
    ) -> impl Future<Output = Result<ProposerDutiesResponse>> + Send;
    /// Publish a block to the beacon node.
    /// https://ethereum.github.io/beacon-APIs/#/Beacon/publishBlock
    fn publish_block(
        &self,
        block: SignedBeaconBlockContent,
        submission_type: SubmissionType,
    ) -> impl Future<Output = Result<()>> + Send;
    /// Gets the fork data of the chain used to sign and verify signatures.
    /// Note: the debug rpc namespace must be enabled from the CL.
    /// https://ethereum.github.io/beacon-APIs/#/Debug/getStateV2
    fn get_fork_data(&self) -> impl Future<Output = Result<GetForkResponse>> + Send;

    /// Gets the known validator data for a given public key.
    fn get_known_validator(
        &self,
        pubkey: BlsPublicKey,
    ) -> impl Future<Output = eyre::Result<KnownValidatorResponse>> + Send;
}

/// Trait to get a connection to the beacon network and subscribe to events.
pub trait BeaconConnection: Send + Sync {
    /// Converts the `BeaconEventsClient` into a `BeaconEventsStream`.
    fn stream(&self) -> impl Future<Output = BeaconEventsStream> + Send;
}
