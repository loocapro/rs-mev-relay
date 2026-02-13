use crate::server::with_beacon_handle;
use crate::server::with_json_body;
use crate::server::with_storage;
use crate::server::WarpResult;
use beacon::api::SubmissionType;
use beacon::service::handle::BeaconHandle;
use relay_primitives::beacon::beacon_block::SignedBeaconBlockContent;
use relay_primitives::beacon::blinded_block::SignedBlindedBlock;
use relay_primitives::beacon::fork::ForkName;
use relay_primitives::beacon::BlindedBlockResponse;
use relay_primitives::beacon::Versioned;
use relay_primitives::blst::SignedRoot;
use relay_primitives::hex;
use relay_primitives::metrics::RelayMetrics;
use relay_storage::Storage;
use serde::Deserialize;
use serde::Serialize;
use tokio::time::Instant;
use tracing::error;
use tracing::info;
use warp::reject::Reject;
use warp::reply;
use warp::Filter;
use warp::Reply;

/// The error type for the post blinded block route.
#[derive(Debug, thiserror::Error, Serialize, Clone, Deserialize)]
pub enum PostBlindedBlockErr {
    /// The slot sent is too old compared to the current slot.
    #[error("No duty found from slot.")]
    NoDutyFound,
    /// The proposer index sent does not match the duty.
    #[error("Proposer index mismatch: expected {expected}, got {actual}.")]
    ProposerIndexMismatch {
        /// The expected proposer index.
        expected: u64,
        /// The actual proposer index.
        actual: u64,
    },
    /// The signature is invalid.
    #[error("Invalid signature.")]
    InvalidSignature,
    /// Could not find blinded block response for block hash.
    #[error("Could not find blinded block response for block hash.")]
    NoBlockFromBlockHash,
    /// Could not publish block.
    #[error("Could not publish block.")]
    PublishBlock,
    /// The validator is not authorized to send the blinded block.
    #[error("Not authorized to submit the blinded block.")]
    UnauthorizedSubmission,
}

impl Reject for PostBlindedBlockErr {}

/// Route for POST `/eth/v1/builder/blinded_blocks`
///
/// See also <https://ethereum.github.io/builder-specs/#/Builder/submitBlindedBlock>
///
/// Trust: We publish the unblinded block to the beacon node in a fire-and-forget task and
/// return the response to the validator without awaiting the publish. This is acceptable only
/// because the relay enforces a trust assumption with validators (API key at registration;
/// only registered validators can submit here). See the README.
pub fn post_blinded_blocks<S: Storage + Clone + Send + Sync>(
    storage: S,
    beacon_handle: BeaconHandle,
) -> impl Filter<Extract = (impl Reply,), Error = warp::Rejection> + Clone {
    let payload_limit = 1024 * 1024 * 8; // 8mb
    warp::path!("eth" / "v1" / "builder" / "blinded_blocks")
        .and(warp::post())
        .and(with_storage(storage))
        .and(with_beacon_handle(beacon_handle))
        .and(with_json_body::<SignedBlindedBlock>(payload_limit))
        .and_then(handler)
}

async fn handler<S: Storage>(
    storage: S,
    beacon_handle: BeaconHandle,
    versioned_signed_blinded_block: SignedBlindedBlock,
) -> WarpResult<impl Reply> {
    let slot = versioned_signed_blinded_block.slot();
    let started = Instant::now();

    let duty = match storage.find_duty_by_slot(slot) {
        Some(bid) => bid,
        None => {
            error!(?slot, "No duty found for slot");
            return Err(warp::reject::custom(PostBlindedBlockErr::NoDutyFound));
        }
    };

    let proposer_index = versioned_signed_blinded_block.proposer_index();
    if duty.validator_index != proposer_index {
        let expected = duty.validator_index;
        let actual = proposer_index;
        error!(
            ?slot,
            ?proposer_index,
            ?expected,
            ?actual,
            "Proposer index does not match duty"
        );
        return Err(warp::reject::custom(
            PostBlindedBlockErr::ProposerIndexMismatch { expected, actual },
        ));
    }

    let domain = storage.fork_data().compute_proposer_domain();
    let signature = versioned_signed_blinded_block.signature();
    let proposer_pubkey = duty.entry.message.pubkey;
    let blinded_block = versioned_signed_blinded_block.message();

    // we validate the MBS key on validator registration
    // so we expect that if the validator is inside the storage
    // it is authorized to interact with us
    if storage
        .read_validator_registration(&proposer_pubkey)
        .is_none()
    {
        error!(
            ?proposer_pubkey,
            "Validator is not authorized to submit the blinded block"
        );
        return Err(warp::reject::custom(
            PostBlindedBlockErr::UnauthorizedSubmission,
        ));
    }

    if !signature.verify(&proposer_pubkey, blinded_block.signing_root(domain.into())) {
        let encoded_domain = hex::encode(domain.as_ref());
        error!(
            ?slot,
            ?proposer_pubkey,
            ?encoded_domain,
            "Invalid signature"
        );
        return Err(warp::reject::custom(PostBlindedBlockErr::InvalidSignature));
    }

    let resp = match storage.read_blinded_block_response(proposer_pubkey) {
        Some(resp) => resp,
        None => {
            error!(
                ?proposer_pubkey,
                "No blinded block response found for proposer"
            );
            return Err(warp::reject::custom(
                PostBlindedBlockErr::NoBlockFromBlockHash,
            ));
        }
    };

    let exec_payload = resp.execution_payload();
    let blobs_data = resp.blobs_bundle().as_ref().clone();
    let extra_data = &exec_payload.extra_data;
    let encoded = hex::encode(extra_data.to_vec());

    let signed_beacon_block = SignedBeaconBlockContent::new(
        exec_payload.clone(),
        blinded_block,
        signature.clone(),
        blobs_data,
    );

    let block_number = signed_beacon_block.block_number();
    let slot = signed_beacon_block.slot();
    let num_txs = signed_beacon_block.num_txs();
    let block_hash = signed_beacon_block.block_hash();

    // Fire-and-forget: we return the response to the validator without awaiting the beacon
    // publish. Safe only under the relay–validator trust assumption (see route doc above).
    tokio::task::spawn(async move {
        beacon_handle
            .publish_block(signed_beacon_block, SubmissionType::Ssz)
            .await
            .map_err(|e| {
                error!(?e, "Failed to publish block");
                warp::reject::custom(PostBlindedBlockErr::PublishBlock)
            })
    });

    let elapsed = started.elapsed();
    let metrics = RelayMetrics::default();
    metrics
        .relay
        .set_blinded_block_latency(elapsed.as_millis() as u64);

    info!(
        target: "proposer_api::blinded_blocks",
        ?elapsed,
        ?block_hash,
        ?slot,
        ?block_number,
        ?num_txs,
        ?encoded,
        "POST /eth/v1/builder/blinded_blocks"
    );

    let versioned_resp = Versioned::<BlindedBlockResponse>::new(ForkName::Deneb, resp);
    storage.set_delivered_blocks(block_hash);
    Ok(reply::json(&versioned_resp))
}

#[cfg(test)]
mod tests {
    use crate::server::errors::handle_rejection;
    use crate::server::proposer_api::post_blinded_blocks;
    use beacon::api::SubmissionType;
    use beacon::service::BeaconService;
    use beacon::BeaconApi;

    use relay_primitives::beacon::beacon_block::SignedBeaconBlockContent;
    use relay_primitives::beacon::blinded_block::SignedBlindedBlock;

    use relay_primitives::beacon::fork::GetForkResponse;
    use relay_primitives::beacon::known_validator::KnownValidatorResponse;
    use relay_primitives::beacon::proposer_duties::ProposerDutiesResponse;
    use relay_primitives::beacon::registrations::{
        ValidatorRegistration, ValidatorRegistrationMessage,
    };
    use relay_primitives::beacon::sync_status::BeaconSyncStatusResponse;

    use relay_primitives::revm_primitives::HashMap;
    use warp::Filter;

    use relay_primitives::beacon::{BlindedBlockResponse, Versioned};
    use relay_primitives::blst::public_key::BlsPublicKey;
    use relay_primitives::blst::signature::BlsSignature;
    use relay_primitives::blst::signer::{BlsSigner, ForkDatas};

    use relay_primitives::events::PayloadAttributesData;

    use relay_primitives::storage::validator::Validator;
    use relay_primitives::types::{Address, B256};
    use relay_primitives::validator::PayloadAttributes;

    use relay_storage::in_memory::InMemoryStorage;
    use relay_storage::Storage;

    use std::str::FromStr;
    use std::sync::Arc;
    use warp::http::StatusCode;
    use warp::test::request;

    fn setup_storage() -> InMemoryStorage {
        InMemoryStorage::new(
            10.into(),
            payload_attr(0, Address::random()).into(),
            vec![],
            ForkDatas::default(),
            HashMap::new(),
            vec![],
        )
    }

    fn payload_attr(slot: u64, suggested_fee_recipient: Address) -> PayloadAttributesData {
        PayloadAttributesData {
            proposal_slot: slot,
            parent_block_root: relay_primitives::B256::ZERO,
            parent_block_number: 0,
            parent_block_hash: relay_primitives::B256::ZERO,
            proposer_index: 0,
            payload_attributes: PayloadAttributes {
                timestamp: 0,
                prev_randao: relay_primitives::B256::ZERO,
                suggested_fee_recipient: *suggested_fee_recipient.as_ref(),
                withdrawals: Some(vec![]),
                parent_beacon_block_root: None,
            },
        }
    }

    #[derive(Clone)]
    struct TestApi;

    impl BeaconApi for TestApi {
        type Config = ();

        fn new(_config: Self::Config) -> Self {
            TestApi
        }
        async fn get_known_validator(
            &self,
            _pubkey: BlsPublicKey,
        ) -> eyre::Result<KnownValidatorResponse> {
            unimplemented!()
        }

        async fn get_fork_data(&self) -> eyre::Result<GetForkResponse> {
            unimplemented!()
        }
        async fn sync_status(&self) -> eyre::Result<BeaconSyncStatusResponse> {
            unimplemented!()
        }

        async fn publish_block(
            &self,
            _block: SignedBeaconBlockContent,
            _submission_type: SubmissionType,
        ) -> eyre::Result<()> {
            Ok(())
        }

        async fn proposer_duties(&self, _epoch: u64) -> eyre::Result<ProposerDutiesResponse> {
            unimplemented!()
        }
    }

    #[tokio::test]
    async fn test_invalid_signature() {
        let storage = setup_storage();
        let proposer = BlsPublicKey::random();
        storage.set_validator_registration(proposer, validator_registration(proposer));
        let api = Arc::new(TestApi::new(()));
        let (beacon_service, beacon_handle) = BeaconService::new(api);
        tokio::spawn(async move {
            beacon_service.await;
        });

        let blinded_block = signed_blinded_block();

        let duties = duties(proposer);
        let blinded_block_resp = blinded_block_resp();

        storage.set_proposer_duties(duties);
        storage.set_blinded_block_response(proposer, blinded_block_resp.data);
        let post_blinded_block =
            post_blinded_blocks(storage, beacon_handle).recover(handle_rejection);
        let resp = request()
            .method("POST")
            .path("/eth/v1/builder/blinded_blocks")
            .json(&blinded_block)
            .reply(&post_blinded_block)
            .await;

        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        let msg = String::from_utf8(resp.body().to_vec())
            .expect("Failed to parse response body as UTF-8");
        let decoded_msg: String =
            serde_json::from_str(&msg).expect("Failed to decode JSON response");

        assert_eq!(decoded_msg, "Invalid signature.");
    }

    #[test]
    fn can_deserialize() {
        let signed_blinded_block = signed_blinded_block();
        assert_eq!(signed_blinded_block.slot(), 46);
        assert_eq!(signed_blinded_block.proposer_index(), 26);
        assert_eq!(
            signed_blinded_block.block_hash(),
            B256::from_str("0xfe739553c4dca9f03fb1ed1773fbefe394d525d98d9012a22517d4976a95fe79")
                .unwrap()
        );
        let blinded_block_resp = blinded_block_resp();
        assert_eq!(
            blinded_block_resp.data.execution_payload.block_hash,
            B256::from_str("0xfe739553c4dca9f03fb1ed1773fbefe394d525d98d9012a22517d4976a95fe79")
                .unwrap()
        );
    }

    #[tokio::test]
    async fn can_submit_blinded_block_and_receive_exec_block() {
        let storage = setup_storage();
        let mut signer = BlsSigner::default();
        let mut blinded_block = signed_blinded_block();
        let mut msg = blinded_block.message().as_ref().clone();
        let signature = signer.proposer_sign(&mut msg);
        blinded_block.signature = signature;
        let pubkey = signer.public_key();
        storage.set_validator_registration(pubkey, validator_registration(pubkey));

        let duties = duties(pubkey);
        let blinded_block_resp = blinded_block_resp();
        let expected_exec_payload = blinded_block_resp.data.execution_payload();
        storage.set_proposer_duties(duties);
        storage.set_blinded_block_response(pubkey, blinded_block_resp.data);

        let api = Arc::new(TestApi::new(()));
        let (beacon_service, beacon_handle) = BeaconService::new(api);
        tokio::spawn(async move {
            beacon_service.await;
        });
        let post_blinded_block = post_blinded_blocks(storage.clone(), beacon_handle);
        let resp = request()
            .method("POST")
            .path("/eth/v1/builder/blinded_blocks")
            .json(&blinded_block)
            .reply(&post_blinded_block)
            .await;
        assert_eq!(resp.status(), 200);
        let json_resp: Versioned<BlindedBlockResponse> =
            serde_json::from_slice(resp.body()).unwrap();
        let exec_payload = json_resp.data.execution_payload();

        assert_eq!(exec_payload.block_hash, expected_exec_payload.block_hash);
        assert_eq!(
            exec_payload.transactions.len(),
            expected_exec_payload.transactions.len()
        );
    }

    fn validator_registration(pubkey: BlsPublicKey) -> ValidatorRegistration {
        let signature = BlsSignature::from_str(relay_primitives::test_utils::TEST_BLS_SIGNATURE).unwrap();

        ValidatorRegistration {
            signature,
            message: ValidatorRegistrationMessage {
                fee_recipient: Address::random(),
                gas_limit: 0,
                timestamp: 0,
                pubkey,
            },
        }
    }

    fn duties(pubkey: BlsPublicKey) -> Vec<Validator> {
        vec![Validator {
            slot: 46,
            validator_index: 26,
            entry: validator_registration(pubkey),
        }]
    }

    fn blinded_block_resp() -> Versioned<BlindedBlockResponse> {
        let json_string = r#"
      {
  "version": "deneb",
  "data": {
    "execution_payload": {
      "parent_hash": "0x6e8d3ca42def0f3bf60f415f60028a70c60033515118ebc419453d3d176dbc24",
      "fee_recipient": "0x0000000000000000000000000000000000000001",
      "state_root": "0xcf8e0d4e9587369b2301d0790347320302cc0943d5a1884560367e8208d920f2",
      "receipts_root": "0xcf8e0d4e9587369b2301d0790347320302cc0943d5a1884560367e8208d920f2",
      "logs_bloom": "0x00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000",
      "prev_randao": "0xcf8e0d4e9587369b2301d0790347320302cc0943d5a1884560367e8208d920f2",
      "block_number": "1",
      "gas_limit": "1",
      "gas_used": "1",
      "timestamp": "1",
      "extra_data": "0xcf8e0d4e9587369b2301d0790347320302cc0943d5a1884560367e8208d920f2",
      "base_fee_per_gas": "1",
      "blob_gas_used": "1",
      "excess_blob_gas": "1",
      "block_hash": "0xfe739553c4dca9f03fb1ed1773fbefe394d525d98d9012a22517d4976a95fe79",
      "transactions": [
        "0x02f878831469668303f51d843b9ac9f9843b9aca0082520894c93269b73096998db66be0441e836d873535cb9c8894a19041886f000080c001a031cc29234036afbf9a1fb9476b463367cb1f957ac0b919b69bbc798436e604aaa018c4e9c3914eb27aadd0b91e10b18655739fcf8c1fc398763a9f1beecb8ddc86"
      ],
      "withdrawals": [
        {
          "index": "1",
          "validator_index": "1",
          "address": "0xabcf8e0d4e9587369b2301d0790347320302cc09",
          "amount": "32000000000"
        }
      ]
    },
    "blobs_bundle": {
      "commitments": [
        "0x8dab030c51e16e84be9caab84ee3d0b8bbec1db4a0e4de76439da8424d9b957370a10a78851f97e4b54d2ce1ab0d686f"
      ],
      "proofs": [
        "0xb4021b0de10f743893d4f71e1bf830c019e832958efd6795baf2f83b8699a9eccc5dc99015d8d4d8ec370d0cc333c06a"
      ],
      "blobs": []
    }
  }
}
"#;
        serde_json::from_str(json_string).unwrap()
    }

    use relay_primitives::blst::SignedRoot;

    #[test]
    fn can_validate_sig() {
        let storage = setup_storage();
        let mut signer = BlsSigner::default();
        let blinded_block = signed_blinded_block();
        let mut msg = blinded_block.message().as_ref().clone();
        let signature = signer.proposer_sign(&mut msg);
        let beacon_proposer_domain = storage.fork_data().compute_proposer_domain();
        let root = msg.signing_root(beacon_proposer_domain.into());
        let pubkey = signer.public_key();

        assert!(signature.verify(&pubkey, root))
    }

    fn signed_blinded_block() -> SignedBlindedBlock {
        let json_string = r#"
        {
  "message": {
    "slot": "46",
    "proposer_index": "26",
    "parent_root": "0x018ae046795a46532d3a9a38826f323ef34b8381c6fcc8c1a6ba81229663476b",
    "state_root": "0xbcca323c304aca7a104c490d3699b01856a5236317ba62eb4c98d2f64d67df47",
    "body": {
      "randao_reveal": "0x93b0506f7754cf1d73213cad2866d6db547b25e323660457b6c0d8bf8f4ee203a76e6a6007fd70af6334b0fb636832e109ef06082cb958eeb6ea739870d2fb3f631659d9d07631a1d9f3fbe99dc1fedc7dc997badcff549a167d45efd2cfdc85",
      "eth1_data": {
        "deposit_root": "0x8e9bcd43f7a4b257953cbc2b531c31f6361278e8a9bdb19f9da72658ead1ec25",
        "deposit_count": "0",
        "block_hash": "0x6e8d3ca42def0f3bf60f415f60028a70c60033515118ebc419453d3d176dbc24"
      },
      "graffiti": "0x0000000000000000000000000000000000000000000000000000000000000000",
      "proposer_slashings": [],
      "attester_slashings": [],
      "attestations": [
        {
          "aggregation_bits": "0x07",
          "data": {
            "slot": "45",
            "index": "0",
            "beacon_block_root": "0x018ae046795a46532d3a9a38826f323ef34b8381c6fcc8c1a6ba81229663476b",
            "source": {
              "epoch": "0",
              "root": "0x0000000000000000000000000000000000000000000000000000000000000000"
            },
            "target": {
              "epoch": "1",
              "root": "0xaae421d67d88fe420dc7dd1fc37199ed8e523cdabba46aa9ffa499dd7195a87b"
            }
          },
          "signature": "0x82bb1d0497ccce8a1e761fff78a1862b711fc5c5043d35774cceae092324f0408a969b3b49775e58419f2bd27b7792cd1858d3dedc24b5a6d1fd98c89f7bb21be9de9f4668a51fa5d1a64ca4585c841a9dbd6ab757a1a9f6b9b39d9efa47f63a"
        }
      ],
      "deposits": [],
      "voluntary_exits": [],
      "sync_aggregate": {
        "sync_committee_bits": "0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
        "sync_committee_signature": "0x8cc00bf45244dd3dc8d7980fb307f9addca49471986969567ae9d3473595bf76b8b372bd9fb4c16754d1f3d4589c42230018a045924151e44574255a0d538a2534d9279dd835b7a6e896522de29d0f0e69159e7aa843803843a02a1217bc1432"
      },
      "execution_payload_header": {
        "parent_hash": "0xb05e822983bf40f95fd29745a6f197ced438547d7ba2ea1ffe4585e0aa8a85d8",
        "fee_recipient": "0x0000000000000000000000000000000000000001",
        "state_root": "0x0b61a994773b5e9b6789034559386a62d4831fb74efd5277573dd0913803e2f5",
        "receipts_root": "0x66ba7d16f10900d45932f3667e48904816300d1471c7b177f7c1ec597e057194",
        "logs_bloom": "0x00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000",
        "prev_randao": "0x0704821e9081571c9abed4967fd180f945fd4fa8799492e5bc34997659df7888",
        "block_number": "44",
        "gas_limit": "30000000",
        "gas_used": "2121000",
        "timestamp": "1712160326",
        "extra_data": "0x7465737400000000000000000000000000000000000000000000000000000000",
        "base_fee_per_gas": "2807727",
        "block_hash": "0xfe739553c4dca9f03fb1ed1773fbefe394d525d98d9012a22517d4976a95fe79",
        "transactions_root": "0x1c22cb966103ab6be8819a478cbdaed75138204745d6c53397bbb7c070518645",
        "withdrawals_root": "0x792930bbd5baac43bcc798ee49aa8185ef76bb3b44ba62b91d86ae569e4bb535",
        "blob_gas_used": "0",
        "excess_blob_gas": "0"
      },
      "bls_to_execution_changes": [],
      "blob_kzg_commitments": []
    }
  },
  "signature": "0x8b31d25f6560a9e51a6ab6617ae1601d75fb044c9592ae0d7ee1cdb721d4451971327fc3948287937d1269926fd9b3a709f7fb95835be8b1c0ad34f0b5b1e61add41400c48b461a96c20a143894b990217b7b2fad5a876e523d896bf8f281eae"
}
"#;
        serde_json::from_str(json_string).unwrap()
    }
}
