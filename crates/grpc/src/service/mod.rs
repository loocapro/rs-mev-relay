use std::sync::Arc;

use crate::gen::{mev_relay_server::MevRelay, SubmitBlockRequest, SubmitBlockResponse};
use relay_primitives::{
    blst::{public_key::BlsPublicKey, signature::BlsSignature, signer::ForkDatas},
    storage::{self, bid_submission::BidSubmission, bid_traces::BidTrace},
    types::{BlobsBundle, ExecutionPayload},
};
use relay_storage::Storage;
use tonic::{Request, Response};
use tracing::{error, warn};

/// The GRPC service implementation
#[derive(Debug)]
pub struct RelayService<S: Storage> {
    storage: S,
    fork_data: ForkDatas,
}

impl<S: Storage> RelayService<S> {
    /// Create a new instance of the MEV Relay GRPC Server
    pub fn new(storage: S, fork_data: ForkDatas) -> Self {
        Self { storage, fork_data }
    }

    /// Static validation on the bid and the execution payload on this steps:
    /// - Skip bids with value 0
    /// - Skip bids with invalid builder
    /// - Skip bids for past slots
    /// - Payload attributes validation: fee_recipient, prev_randao, and slot
    /// - Duty validation: fee_recipient
    #[allow(clippy::result_large_err)]
    fn validate_bid_and_payload(
        &self,
        bid: &BidTrace,
        execution_payload: &ExecutionPayload,
    ) -> Result<storage::validator::Validator, tonic::Status> {
        // Skip bids with value 0
        if bid.value.as_ref().is_zero() {
            error!("Block value 0");
            return Err(tonic::Status::invalid_argument("Block value 0."));
        }

        let builder = BlsPublicKey::from(bid.builder_pubkey);

        // Skip bids with invalid builder
        if !self.storage.is_whitelisted_builder(&builder) {
            error!("Invalid builder");
            return Err(tonic::Status::invalid_argument("Invalid builder address."));
        }

        // Skip bids for past slots
        let head_slot = self.storage.read_head_slot();
        if !head_slot.is_next_slot(bid.slot) {
            error!(?head_slot, ?bid.slot, "Submission for past slot");
            return Err(tonic::Status::invalid_argument("Submission for past slot."));
        }

        // Payload attributes validation
        let attrs = self.storage.read_payload_attributes();
        if let Err(error_message) = attrs.validate_bid(bid.slot, execution_payload.prev_randao) {
            error!("{:?}", &error_message);
            return Err(tonic::Status::invalid_argument(error_message.to_string()));
        }

        // Duty validation
        let duty = self.storage.find_duty_by_slot(bid.slot).ok_or_else(|| {
            error!("No duty found");
            tonic::Status::invalid_argument("No duty found.")
        })?;

        Ok(duty)
    }

    /// Verifies that the bid is signed from the given signature.
    #[allow(clippy::result_large_err)]
    fn verify_signature(
        &self,
        bid: &BidTrace,
        signature: &BlsSignature,
    ) -> Result<(), tonic::Status> {
        let builder_domain = self.fork_data.compute_builder_domain();
        let is_verified = bid.verify_signature(signature, builder_domain);

        if !is_verified {
            error!("Invalid signature");
            return Err(tonic::Status::invalid_argument("Invalid signature"));
        }

        Ok(())
    }

    /// Retrieves and compares the current best bid for a given slot to the incoming bid value.
    fn is_bid_higher(&self, bid: &BidTrace) -> bool {
        let slot = bid.slot;
        let parent_hash = bid.parent_hash;
        let proposer = bid.proposer_pubkey;
        if let Some(current_best) = self.storage.read_best_bid(slot, parent_hash, proposer) {
            return bid > current_best.bid_trace();
        }
        true
    }

    /// Updates the best bid for the given slot.
    fn update_best(
        &self,
        bid: BidTrace,
        exec_payload: ExecutionPayload,
        blobs_bundle: BlobsBundle,
        signature: BlsSignature,
        proposer_key: BlsPublicKey,
    ) {
        let slot = bid.slot;
        let exec_payload = Arc::new(exec_payload);
        let blobs_bundle = Arc::new(blobs_bundle);
        let submission = BidSubmission::new(bid, exec_payload, blobs_bundle, signature);
        let blinded_block_response = submission.to_blinded_block_response();

        self.storage.set_best_bid(slot, Arc::new(submission));
        self.storage
            .set_blinded_block_response(proposer_key, blinded_block_response);
    }
}

#[tonic::async_trait]
impl<S: Storage + Send + Sync + 'static> MevRelay for RelayService<S> {
    async fn submit_block(
        &self,
        request: Request<SubmitBlockRequest>,
    ) -> Result<Response<SubmitBlockResponse>, tonic::Status> {
        let req = request.into_inner();

        let bid = req.bid_trace.ok_or_else(|| {
            error!("No bid");
            tonic::Status::invalid_argument("Missing bid")
        })?;

        let exec_payload = req.execution_payload.ok_or_else(|| {
            error!("No Execution Payload");
            tonic::Status::invalid_argument("Missing Execution Payload")
        })?;

        let blobs = req.blobs_bundle.ok_or_else(|| {
            error!("No blobs bundle");
            tonic::Status::invalid_argument("Missing Blobs Bundle")
        })?;

        let bid = BidTrace::try_from(bid).map_err(|e| {
            error!("Invalid bid: {:?}", e);
            tonic::Status::invalid_argument(e.to_string())
        })?;

        // if bid not higher than current best no reason to continue
        if !self.is_bid_higher(&bid) {
            warn!("Incoming bid is below or equal to the current best bid");
            let reply = SubmitBlockResponse {
                code: 400,
                message: "Bid below best.".into(),
            };
            return Ok(Response::new(reply));
        }

        let exec_payload = ExecutionPayload::try_from(exec_payload).map_err(|e| {
            error!("Invalid execution payload: {:?}", e);
            tonic::Status::invalid_argument(e.to_string())
        })?;
        let duty = self.validate_bid_and_payload(&bid, &exec_payload)?;

        let signature = BlsSignature::from(req.signature);

        self.verify_signature(&bid, &signature)?;

        let blobs = BlobsBundle::try_from(blobs).map_err(|e| {
            error!("Invalid blobs bundle: {:?}", e);
            tonic::Status::invalid_argument(e.to_string())
        })?;

        self.update_best(
            bid,
            exec_payload,
            blobs,
            signature,
            duty.entry.message.pubkey,
        );

        let reply = SubmitBlockResponse {
            code: 200,
            message: "Success".into(),
        };

        Ok(Response::new(reply))
    }
}
