use metrics::{Counter, Gauge};
use reth_metrics::Metrics;

// POST blocks
// bids per slot

/// Metrics for the chain state.
#[derive(Metrics, Clone)]
#[metrics(scope = "beacon")]
pub struct Chain {
    /// The current slot of the chain.
    proposal_slot: Gauge,
    /// The current block of the chain.
    block: Gauge,
}

impl Chain {
    /// Keeps track of the current slot.
    pub fn set_proposal_slot(&self, slot: u64) {
        self.proposal_slot.set(slot as f64);
    }
    /// Keeps track of the current parent block.
    pub fn set_block(&self, block: u64) {
        self.block.set(block as f64);
    }
}

/// Metrics for the relay.
#[derive(Metrics, Clone)]
#[metrics(scope = "relay")]
pub struct Relay {
    /// Registered validators via POST /validators
    registered_validators: Gauge,
    /// Consensus known validators
    known_validators: Gauge,
    /// Number of headers returned via GET /header
    headers: Counter,
    /// Number of blinded blocks delivered via POST /blinded_blocks
    blinded_blocks: Counter,
    /// Values for the delivered headers
    header_values: Gauge,
    /// Number of mined blocks relayed.
    relayed_blocks: Counter,
    /// Number of mined blocks non relayed.
    non_relayed_blocks: Counter,
    /// Number of transactions in the relayed block.
    tx_count: Gauge,
    /// The latency that takes to return the blinded block to the validator
    blinded_block_latency: Gauge,
    /// The latency that takes to return the header to the validator
    header_latency: Gauge,
}

impl Relay {
    /// Sets the latency for the blinded block.
    pub fn set_blinded_block_latency(&self, latency: u64) {
        self.blinded_block_latency.set(latency as f64);
    }
    /// Sets the latency for the header.
    pub fn set_header_latency(&self, latency: u64) {
        self.header_latency.set(latency as f64);
    }
    /// Sets the number of registered validators.
    pub fn set_registered_validators(&self, count: usize) {
        self.registered_validators.set(count as f64);
    }
    /// Sets the number of known validators.
    pub fn set_known_validators(&self, count: u64) {
        self.known_validators.set(count as f64);
    }
    /// Increments the number of headers returned by the relay.
    pub fn inc_headers(&self) {
        self.headers.increment(1);
    }
    /// Sets the values for the delivered headers.
    pub fn set_header_values(&self, value: u64) {
        self.header_values.set(value as f64);
    }
    /// Sets the number of transactions in the relayed block.
    pub fn set_tx_count(&self, count: usize) {
        self.tx_count.set(count as f64);
    }
    /// Increments the number of blinded blocks delivered by the relay.
    pub fn inc_blinded_blocks(&self) {
        self.blinded_blocks.increment(1);
    }
    /// Increments the number of mined blocks relayed.
    pub fn inc_relayed_blocks(&self) {
        self.relayed_blocks.increment(1);
    }
    /// Increments the number of mined blocks non relayed.
    pub fn inc_non_relayed_blocks(&self) {
        self.non_relayed_blocks.increment(1);
    }
}

/// Metrics for the relay.
#[derive(Clone, Debug, Default)]
pub struct RelayMetrics {
    /// Chain related metrics
    pub chain: Chain,
    /// Relay related metrics
    pub relay: Relay,
}
