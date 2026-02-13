//! # Primitives Crate
//!
//! The `primitives` crate defines and re-exports the fundamental types used across your Rust project.
//! It serves as a central repository for all primitive types, ensuring consistency and reducing
//! duplication across the project. This crate simplifies the process of managing dependencies and
//! types in a large codebase by providing a single source of truth for primitive types.
#![warn(missing_docs, unreachable_pub, rustdoc::all)]
#![deny(unused_must_use, rust_2018_idioms)]

/// Beacon node related types
pub mod beacon;

/// BLST related types
pub mod blst;

/// Prometheus metrics
pub mod metrics;
/// Storage related types
pub mod storage;
/// Reth wrapped types for easy trait implementation.
pub mod types;

/// Test-only constants (addresses, hashes, BLS fixtures). Not linked to any real chain or account.
pub mod test_utils;
/// Helper type alias for boxed futures.
pub type BoxedFuture<T> = Pin<Box<dyn Future<Output = eyre::Result<T>> + Send + 'static>>;

/// Server side events helper types
pub mod events {
    pub use mev_share_sse::{client::EventStream as SseEventStream, client::SseError, EventClient};
    pub use reth::rpc::types::beacon::events::PayloadAttributesEvent;
    pub use reth::rpc::types::beacon::events::{BlockEvent, HeadEvent, PayloadAttributesData};
    pub use reth::rpc::types::beacon::{BlsPublicKey, BlsSignature};
}

/// Validator related types
pub mod validator {
    pub use reth::rpc::types::engine::PayloadAttributes;
    pub use reth::rpc::types::relay::{
        Validator, ValidatorRegistration, ValidatorRegistrationMessage,
    };
}
use futures::Future;
pub use reth::primitives::*;
pub use reth::tasks::{TaskSpawner, TokioTaskExecutor};
use std::pin::Pin;
