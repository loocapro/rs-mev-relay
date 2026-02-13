use crate::Storage;
use parking_lot::RwLock;
use relay_primitives::{
    beacon::{registrations::ValidatorRegistration, BlindedBlockResponse},
    blst::{public_key::BlsPublicKey, signer::ForkDatas},
    metrics::RelayMetrics,
    revm_primitives::HashMap,
    storage::{
        bid_submission::BidSubmission, head_slot::HeadSlot, payload_attributes::PayloadAttributes,
        validator::Validator,
    },
    types::B256,
};
use std::sync::Arc;

#[derive(Hash, Eq, PartialEq, Debug, Clone)]
pub(crate) struct SlotHashKey {
    slot: u64,
    hash: B256,
    validator: BlsPublicKey,
}

/// The stored validator registrations
pub type ValidatorRegistrations = HashMap<BlsPublicKey, ValidatorRegistration>;
type BestBidBySlotHashKey = HashMap<SlotHashKey, Arc<BidSubmission>>;
type BestBidByBlockNumber = HashMap<u64, Arc<BidSubmission>>;
type BestBidBySlot = HashMap<u64, Arc<BidSubmission>>;
type BlindedBlockRespPerBlock = HashMap<BlsPublicKey, BlindedBlockResponse>;

/// In-memory storage inner structure.
#[derive(Debug)]
pub(crate) struct InMemoryStorageInner {
    /// The latest slot.
    pub(crate) slot: HeadSlot,
    /// The latest payload attributes.
    pub(crate) payload_attributes: PayloadAttributes,
    /// The list of proposer duties, it gets updated every epoch.
    pub(crate) proposer_duties: Vec<Validator>,
    /// The list of registered validators via beacon api endpoint: /eth/v1/beacon/validators
    pub(crate) validator_registrations: ValidatorRegistrations,
    /// The list of white listed builders, enabled to submit bids.
    pub(crate) wlist_builders: Vec<BlsPublicKey>,
    /// List of best bids indexed by slot.
    pub(crate) best_bid_per_slot: BestBidBySlotHashKey,
    /// List of best bids indexed by slot number.
    pub(crate) best_bid_by_slot: BestBidBySlot,
    /// List of best bids indexed by block number.
    pub(crate) best_bid_by_block_number: BestBidByBlockNumber,
    /// The fork data used to sign and verify signatures.
    pub(crate) fork_data: ForkDatas,
    /// Responses for blinded blocks requests.
    pub(crate) blinded_block_resp: BlindedBlockRespPerBlock,
    /// Delivered blocks.
    pub(crate) delivered_blocks: Vec<B256>,
}

/// In-memory storage implementation.
#[derive(Debug, Clone)]
pub struct InMemoryStorage {
    /// The inner structure contains the actual storage data.
    inner: Arc<RwLock<InMemoryStorageInner>>,
    metrics: RelayMetrics,
}

impl InMemoryStorage {
    /// Creates a new in-memory storage.
    pub fn new(
        slot: HeadSlot,
        payload_attributes: PayloadAttributes,
        wlist_builders: Vec<BlsPublicKey>,
        fork_data: ForkDatas,
        validator_registrations: ValidatorRegistrations,
        proposer_duties: Vec<Validator>,
    ) -> InMemoryStorage {
        let metrics = RelayMetrics::default();
        let proposal_slot = payload_attributes.proposal_slot();
        metrics.chain.set_proposal_slot(proposal_slot);
        let p_block = payload_attributes.parent_block_number();
        metrics.chain.set_block(p_block);

        InMemoryStorage {
            inner: Arc::new(RwLock::new(InMemoryStorageInner {
                slot,
                payload_attributes,
                proposer_duties,
                validator_registrations,
                wlist_builders,
                best_bid_per_slot: HashMap::new(),
                fork_data,
                blinded_block_resp: HashMap::new(),
                best_bid_by_block_number: HashMap::new(),
                best_bid_by_slot: HashMap::new(),
                delivered_blocks: Vec::new(),
            })),
            metrics,
        }
    }
}

impl Storage for InMemoryStorage {
    fn record_relayed_block(&self, block_hash: B256) {
        if self.inner.read().delivered_blocks.contains(&block_hash) {
            self.metrics.relay.inc_relayed_blocks();
        } else {
            self.metrics.relay.inc_non_relayed_blocks();
        }
    }
    fn set_delivered_blocks(&self, block_hash: B256) {
        self.metrics.relay.inc_blinded_blocks();
        self.inner.write().delivered_blocks.push(block_hash);
    }
    fn fork_data(&self) -> ForkDatas {
        self.inner.read().fork_data.clone()
    }
    fn set_head_slot(&self, slot: HeadSlot) {
        self.inner.write().slot = slot;
    }

    fn read_head_slot(&self) -> HeadSlot {
        self.inner.read().slot
    }

    fn set_payload_attributes(&self, payload_attr: PayloadAttributes) {
        let block_hash = payload_attr.parent_hash();
        let parent_block_number = payload_attr.parent_block_number();
        let proposal_slot = payload_attr.proposal_slot();
        self.metrics.chain.set_block(parent_block_number);
        self.metrics.chain.set_proposal_slot(proposal_slot);
        self.record_relayed_block(block_hash);
        self.inner.write().payload_attributes = payload_attr;
    }

    fn read_payload_attributes(&self) -> PayloadAttributes {
        self.inner.read().payload_attributes.clone()
    }

    fn empty_validator_regs(&self) -> bool {
        self.inner.read().validator_registrations.is_empty()
    }

    fn set_validator_registration(&self, key: BlsPublicKey, registration: ValidatorRegistration) {
        self.inner
            .write()
            .validator_registrations
            .insert(key, registration);
    }

    fn set_validator_registrations(&self, registrations: ValidatorRegistrations) {
        let count = registrations.len();
        self.inner.write().validator_registrations = registrations;
        self.metrics.relay.set_registered_validators(count);
    }

    fn read_validator_registration(&self, key: &BlsPublicKey) -> Option<ValidatorRegistration> {
        self.inner.read().validator_registrations.get(key).cloned()
    }

    fn set_proposer_duties(&self, duties: Vec<Validator>) {
        self.inner.write().proposer_duties = duties;
    }
    fn find_duty_by_slot(&self, slot: u64) -> Option<Validator> {
        self.inner
            .read()
            .proposer_duties
            .iter()
            .find(|duty| duty.slot == slot)
            .cloned()
    }
    fn read_proposer_duties(&self) -> Vec<Validator> {
        self.inner.read().proposer_duties.clone()
    }
    fn is_whitelisted_builder(&self, key: &BlsPublicKey) -> bool {
        self.inner.read().wlist_builders.contains(key)
    }
    fn set_best_bid(&self, slot: u64, bid: Arc<BidSubmission>) {
        let trace = bid.bid_trace();
        let execution_payload = bid.execution_payload();
        let key = SlotHashKey {
            slot,
            hash: trace.parent_hash,
            validator: trace.proposer_pubkey,
        };

        let mut inner = self.inner.write();
        inner.best_bid_per_slot.insert(key, bid.clone());
        inner
            .best_bid_by_block_number
            .insert(execution_payload.block_number, bid);
    }

    fn read_best_bid_by_block_number(&self, block_number: u64) -> Option<Arc<BidSubmission>> {
        self.inner
            .read()
            .best_bid_by_block_number
            .get(&block_number)
            .cloned()
    }

    fn read_best_bid_by_slot(&self, slot: u64) -> Option<Arc<BidSubmission>> {
        self.inner.read().best_bid_by_slot.get(&slot).cloned()
    }

    fn set_blinded_block_response(
        &self,
        proposer: BlsPublicKey,
        blinded_block_resp: BlindedBlockResponse,
    ) {
        self.inner
            .write()
            .blinded_block_resp
            .insert(proposer, blinded_block_resp);
    }

    fn read_blinded_block_response(&self, proposer: BlsPublicKey) -> Option<BlindedBlockResponse> {
        self.inner.read().blinded_block_resp.get(&proposer).cloned()
    }
    fn read_best_bid(
        &self,
        slot: u64,
        parent_hash: B256,
        validator: BlsPublicKey,
    ) -> Option<Arc<BidSubmission>> {
        let key = SlotHashKey {
            slot,
            hash: parent_hash,
            validator,
        };
        self.inner.read().best_bid_per_slot.get(&key).cloned()
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use crate::{in_memory::InMemoryStorage, Storage};
    use relay_primitives::{
        beacon::registrations::{ValidatorRegistration, ValidatorRegistrationMessage},
        blst::{public_key::BlsPublicKey, signature::BlsSignature, signer::ForkDatas},
        events::PayloadAttributesData,
        revm_primitives::HashMap,
        storage::validator::Validator,
        validator::PayloadAttributes,
        Address, B256,
    };

    #[tokio::test]
    async fn can_store_under_high_concurrency() {
        let storage = InMemoryStorage::new(
            0.into(),
            payload_attr(0).into(),
            vec![],
            ForkDatas::default(),
            HashMap::new(),
            vec![],
        );
        let tasks: Vec<_> = (0..10000)
            .map(|i| {
                let storage_clone = storage.clone();
                tokio::spawn(async move {
                    perform_storage_operations(&storage_clone, i).await;
                })
            })
            .collect();

        let results = futures::future::join_all(tasks).await;
        assert!(results.iter().all(|result| result.is_ok()));
    }

    async fn perform_storage_operations(storage: &InMemoryStorage, i: u64) {
        let key = BlsPublicKey::random();
        let registration = validator_registration(i);
        let duty = proposer_duties(i);
        let slot = i;
        let payload = payload_attr(i);

        // Write operations
        storage.set_validator_registration(key, registration.clone());
        storage.set_proposer_duties(duty.clone());
        storage.set_head_slot(slot.into());
        storage.set_payload_attributes(payload.clone().into());

        // Read and assert operations
        assert_storage_contents(storage, &key, &registration, &duty, slot, &payload);

        // Update operations
        update_storage_contents(storage, i, &key);
    }

    fn update_storage_contents(storage: &InMemoryStorage, i: u64, key: &BlsPublicKey) {
        let new_registration = validator_registration(i + 1);
        storage.set_validator_registration(*key, new_registration.clone());
        let new_duty = proposer_duties(i + 1);
        storage.set_proposer_duties(new_duty.clone());
        let new_slot = i + 1;
        storage.set_head_slot(new_slot.into());
        let new_payload = payload_attr(i + 1);
        storage.set_payload_attributes(new_payload.clone().into());

        // Assert updated contents
        assert_eq!(
            storage.read_validator_registration(key),
            Some(new_registration)
        );

        assert_eq!(storage.read_proposer_duties(), new_duty);
        assert_eq!(storage.read_head_slot(), new_slot.into());
        assert_eq!(storage.read_payload_attributes(), new_payload.into());
    }

    fn assert_storage_contents(
        storage: &InMemoryStorage,
        key: &BlsPublicKey,
        registration: &ValidatorRegistration,
        duty: &[Validator],
        slot: u64,
        payload: &PayloadAttributesData,
    ) {
        assert_eq!(
            storage.read_validator_registration(key),
            Some(registration.clone())
        );
        assert_eq!(storage.read_proposer_duties(), duty);
        assert_eq!(storage.read_head_slot(), slot.into());
        assert_eq!(storage.read_payload_attributes(), payload.clone().into());
    }

    fn payload_attr(timestamp: u64) -> PayloadAttributesData {
        PayloadAttributesData {
            proposal_slot: 0,
            parent_block_root: B256::ZERO,
            parent_block_number: 0,
            parent_block_hash: B256::ZERO,
            proposer_index: 0,
            payload_attributes: PayloadAttributes {
                timestamp,
                prev_randao: B256::ZERO,
                suggested_fee_recipient: Address::ZERO,
                withdrawals: Some(vec![]),
                parent_beacon_block_root: None,
            },
        }
    }

    fn proposer_duties(slot: u64) -> Vec<Validator> {
        vec![Validator {
            slot,
            validator_index: 0,
            entry: validator_registration(slot),
        }]
    }

    fn validator_registration(timestamp: u64) -> ValidatorRegistration {
        let signature = BlsSignature::from_str(relay_primitives::test_utils::TEST_BLS_SIGNATURE).unwrap();

        ValidatorRegistration {
            signature,
            message: ValidatorRegistrationMessage {
                fee_recipient: Address::ZERO.into(),
                gas_limit: 0,
                timestamp,
                pubkey: BlsPublicKey::random(),
            },
        }
    }
}
