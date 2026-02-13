use crate::{gen::mev_relay_server::MevRelayServer, service::RelayService};
use relay_primitives::blst::signer::ForkDatas;
use relay_storage::Storage;
use std::net::SocketAddr;
use tonic::transport::Server;
use tracing::info;

/// MEV Relay GRPC Server implementation
#[derive(Debug)]
pub struct GrpcServer;

impl GrpcServer {
    /// Create a new instance of the MEV Relay GRPC Server
    pub async fn start<S: Storage + Send + Sync + 'static>(
        url: SocketAddr,
        storage: S,
        fork_data: ForkDatas,
    ) {
        let service = MevRelayServer::new(RelayService::new(storage, fork_data));
        info!("GRPC server started at {}", url);
        Server::builder()
            .add_service(service)
            .serve(url)
            .await
            .unwrap();
    }
}

#[cfg(test)]
mod test {

    use crate::gen::mev_relay_client::MevRelayClient;
    use crate::gen::{BidTrace, BlobsBundle, ExecutionPayload, SubmitBlockRequest};
    use relay_primitives::beacon::registrations::{
        ValidatorRegistration, ValidatorRegistrationMessage,
    };
    use relay_primitives::blst::public_key::BlsPublicKey;
    use relay_primitives::blst::signature::BlsSignature;
    use relay_primitives::blst::signer::{BlsSigner, ForkDatas};
    use relay_primitives::events::PayloadAttributesData;
    use relay_primitives::revm_primitives::HashMap;
    use relay_primitives::B256;

    use super::GrpcServer;
    use relay_primitives::storage::validator::Validator;
    use relay_primitives::types::{Address, U256};
    use relay_primitives::validator::PayloadAttributes;

    use relay_storage::in_memory::InMemoryStorage;
    use relay_storage::Storage;
    use socket2::{Domain, Socket, Type};

    use std::net::TcpListener;
    use std::sync::Arc;
    use tokio::task::JoinHandle;
    use tonic::transport::Channel;
    use tonic::Request;

    fn get_random_available_port() -> u16 {
        let addr = &"127.0.0.1:0".parse::<SocketAddr>().unwrap().into();
        let socket = Socket::new(Domain::IPV4, Type::STREAM, None).unwrap();
        socket.set_reuse_address(true).unwrap();
        socket.bind(addr).unwrap();
        socket.listen(1).unwrap();
        let listener = TcpListener::from(socket);
        listener.local_addr().unwrap().port()
    }

    async fn get_client(url: String) -> MevRelayClient<Channel> {
        let formatted = format!("http://{}", url);
        loop {
            match MevRelayClient::connect(formatted.clone()).await {
                Ok(client) => return client,
                Err(_) => println!("waiting for server connection"),
            }
        }
    }

    async fn setup(
        builders: Vec<BlsPublicKey>,
    ) -> (MevRelayClient<Channel>, InMemoryStorage, JoinHandle<()>) {
        let port = get_random_available_port();
        let string_addr = format!("127.0.0.1:{port}");
        let addr = string_addr.parse().unwrap();
        let storage = InMemoryStorage::new(
            10.into(),
            payload_attr(0, Address::random()).into(),
            builders,
            ForkDatas::default(),
            HashMap::new(),
            vec![],
        );
        let server = GrpcServer::start(addr, storage.clone(), ForkDatas::default());
        let handle = tokio::spawn(server);
        let client = get_client(string_addr).await;
        (client, storage, handle)
    }

    #[tokio::test]
    async fn invalid_builder() {
        let (mut client, _, handle) = setup(vec![]).await;
        test_execution_payload_none(&mut client).await;
        test_bid_none(&mut client).await;
        test_invalid_builder(&mut client).await;
        handle.abort();
    }

    #[tokio::test]
    async fn slot_validation() {
        let enabled_builders = BlsPublicKey::default();
        let (mut client, _, handle) = setup(vec![enabled_builders]).await;
        test_invalid_slot(&mut client).await;
        handle.abort();
    }

    #[tokio::test]
    async fn attributes_validation() {
        let enabled_builders = BlsPublicKey::default();
        let (mut client, storage, handle) = setup(vec![enabled_builders]).await;
        test_invalid_attributes(&mut client, "Payload attributes: Invalid slot.".to_string()).await;
        let invalid_attr_by_proposer_fee = payload_attr(11, Address::random());
        storage.set_payload_attributes(invalid_attr_by_proposer_fee.into());
        test_invalid_attributes(
            &mut client,
            "Payload attributes: Invalid prev_randao.".to_string(),
        )
        .await;
        let invalid_attr_by_prev_randao = payload_attr(11, Address::zero());
        storage.set_payload_attributes(invalid_attr_by_prev_randao.into());
        test_invalid_attributes(
            &mut client,
            "Payload attributes: Invalid prev_randao.".to_string(),
        )
        .await;
        handle.abort();
    }

    #[tokio::test]
    async fn duties_validation() {
        let enabled_builders = BlsPublicKey::default();
        let (mut client, storage, handle) = setup(vec![enabled_builders]).await;
        let valid_attrs = payload_attr(11, Address::zero());
        storage.set_payload_attributes(valid_attrs.into());
        test_invalid_duties(&mut client, "No duty found.".to_string()).await;
        let invalid_duties_by_proposer_fee = proposer_duties(11, Address::random());
        storage.set_proposer_duties(invalid_duties_by_proposer_fee);
        test_invalid_duties(&mut client, "Invalid signature".to_string()).await;
        handle.abort();
    }
    use std::net::SocketAddr;
    use std::str::FromStr;
    #[tokio::test]
    async fn invalid_signature() {
        let enabled_builder = *relay_primitives::test_utils::test_bls_pubkey();

        let (mut client, storage, handle) = setup(vec![enabled_builder]).await;
        let signature = BlsSignature::from_str(relay_primitives::test_utils::TEST_BLS_SIGNATURE).unwrap();

        let mut bid = bid_trace(11, "1000".to_string());
        bid.builder_pubkey = enabled_builder.serialize().to_vec();
        let request = Request::new(SubmitBlockRequest {
            bid_trace: Some(bid),
            execution_payload: Some(execution_payload(B256::ZERO)),
            signature: signature.to_vec(),
            blobs_bundle: Some(blobs_bundle()),
        });
        let valid_attrs = payload_attr(11, Address::zero());
        storage.set_payload_attributes(valid_attrs.into());
        let valid_duty = proposer_duties(11, Address::zero());
        storage.set_proposer_duties(valid_duty);
        let response = client.submit_block(request).await;
        assert!(response.is_err(), "Expected an error for the test case");
        let error = response.err().unwrap();
        assert_eq!(
            error.code(),
            tonic::Code::InvalidArgument,
            "Expected InvalidArgument error"
        );
        assert_eq!(
            error.message(),
            "Invalid signature",
            "Error message mismatch"
        );
        handle.abort();
    }

    #[tokio::test]
    async fn valid_submission() {
        let signer = BlsSigner::default();
        let enabled_builder = BlsPublicKey::from_str(&signer.public_key().to_string()).unwrap();

        let (mut client, storage, handle) = setup(vec![enabled_builder]).await;

        let mut bid = bid_trace(11, "1000".to_string());
        bid.builder_pubkey = enabled_builder.serialize().to_vec();
        let mut signable_bid: relay_primitives::storage::bid_traces::BidTrace =
            bid.clone().try_into().unwrap();
        let signature = signer.sign(&mut signable_bid);
        let signature = BlsSignature::from_str(&signature.to_string())
            .unwrap()
            .to_vec();

        let request = Request::new(SubmitBlockRequest {
            bid_trace: Some(bid),
            execution_payload: Some(execution_payload(B256::ZERO)),
            signature,
            blobs_bundle: Some(blobs_bundle()),
        });
        let valid_attrs = payload_attr(11, Address::zero());
        storage.set_payload_attributes(valid_attrs.into());
        let valid_duty = proposer_duties(11, Address::zero());
        storage.set_proposer_duties(valid_duty);
        client.submit_block(request).await.unwrap();
        let parent_hash = signable_bid.parent_hash;
        let proposer_pubkey = signable_bid.proposer_pubkey;
        let best_bid = storage
            .read_best_bid(11, parent_hash, proposer_pubkey)
            .unwrap();
        assert_eq!(best_bid.bid_trace().value, U256::from(1000));
        handle.abort();
    }

    #[tokio::test]
    async fn concurrent_submissions() {
        let signer = BlsSigner::default();
        let enabled_builder = BlsPublicKey::from_str(&signer.public_key().to_string()).unwrap();

        let (client, storage, handle) = setup(vec![enabled_builder]).await;

        let valid_attrs = payload_attr(11, Address::zero());
        storage.set_payload_attributes(valid_attrs.into());
        let valid_duty = proposer_duties(11, Address::zero());
        storage.set_proposer_duties(valid_duty);

        // Number of concurrent tasks
        let n = 1000;
        // Starting bid value
        let start_value = 1000;

        // Use a barrier to make sure all tasks start at roughly the same time
        let barrier = Arc::new(tokio::sync::Barrier::new(n + 1));

        let mut handles = Vec::new();

        for i in 0..n {
            let mut client = client.clone();
            let barrier = barrier.clone();
            let signer = signer.clone();
            let bid_value = start_value + i * 100; // Increment bid value for each task

            let handle = tokio::spawn(async move {
                barrier.wait().await; // Wait for all tasks to be ready

                let mut bid = bid_trace(11, bid_value.to_string());

                bid.builder_pubkey = enabled_builder.serialize().to_vec();
                let mut signable_bid: relay_primitives::storage::bid_traces::BidTrace =
                    bid.clone().try_into().unwrap();

                let signature = signer.sign(&mut signable_bid);
                let signature = BlsSignature::from_str(&signature.to_string())
                    .unwrap()
                    .to_vec();

                let request = Request::new(SubmitBlockRequest {
                    bid_trace: Some(bid),
                    execution_payload: Some(execution_payload(B256::ZERO)),
                    signature,
                    blobs_bundle: Some(blobs_bundle()),
                });

                client.submit_block(request).await.unwrap();
            });

            handles.push(handle);
        }

        barrier.wait().await; // Release the barrier for all tasks to start
        for handle in handles {
            handle.await.unwrap(); // Wait for all tasks to complete
        }
        let p_hash = relay_primitives::types::B256::from_str(&B256::ZERO.to_string()).unwrap();
        let pubkey = *relay_primitives::test_utils::test_bls_pubkey();

        // Check the best bid
        let best_bid = storage.read_best_bid(11, p_hash, pubkey).unwrap();
        let amount: u64 = (start_value + (n - 1) * 100).try_into().unwrap(); // The highest bid value

        assert_eq!(best_bid.bid_trace().value, U256::from(amount)); // The highest bid value

        handle.abort();
    }

    async fn test_invalid_duties(client: &mut MevRelayClient<Channel>, error_message: String) {
        let mut bid = bid_trace(11, "1000".to_string());
        let signature = BlsSignature::from_str(relay_primitives::test_utils::TEST_BLS_SIGNATURE).unwrap();

        bid.builder_pubkey = BlsPublicKey::default().serialize().to_vec(); // Assuming this triggers the validation error
        let request = Request::new(SubmitBlockRequest {
            bid_trace: Some(bid),
            execution_payload: Some(execution_payload(B256::ZERO)), // Assuming execution_payload() provides a valid object
            signature: signature.to_vec(),
            blobs_bundle: Some(blobs_bundle()),
        });

        let response = client.submit_block(request).await;
        assert!(response.is_err(), "Expected an error for the test case");
        let error = response.err().unwrap();
        assert_eq!(
            error.code(),
            tonic::Code::InvalidArgument,
            "Expected InvalidArgument error"
        );
        assert_eq!(error.message(), error_message, "Error message mismatch");
    }

    async fn test_invalid_attributes(client: &mut MevRelayClient<Channel>, error_message: String) {
        let mut bid = bid_trace(11, "1000".to_string());
        bid.builder_pubkey = BlsPublicKey::default().serialize().to_vec(); // Assuming this triggers the validation error
        let request = Request::new(SubmitBlockRequest {
            bid_trace: Some(bid),
            execution_payload: Some(execution_payload(B256::random())), // Assuming execution_payload() provides a valid object
            signature: vec![],
            blobs_bundle: Some(blobs_bundle()),
        });

        let response = client.submit_block(request).await;
        assert!(response.is_err(), "Expected an error for the test case");
        let error = response.err().unwrap();
        assert_eq!(
            error.code(),
            tonic::Code::InvalidArgument,
            "Expected InvalidArgument error"
        );
        assert_eq!(error.message(), error_message, "Error message mismatch");
    }

    async fn test_invalid_slot(client: &mut MevRelayClient<Channel>) {
        // Construct request with a valid slot
        let mut bid = bid_trace(10, "1000".to_string());
        bid.builder_pubkey = BlsPublicKey::default().serialize().to_vec();
        let request = Request::new(SubmitBlockRequest {
            bid_trace: Some(bid),
            execution_payload: Some(execution_payload(B256::random())), // Assuming execution_payload() provides a valid object
            signature: vec![],
            blobs_bundle: Some(blobs_bundle()),
        });

        let response = client.submit_block(request).await;
        assert!(response.is_err(), "Expected an error for invalid builder");
        let error = response.err().unwrap();
        assert_eq!(
            error.code(),
            tonic::Code::InvalidArgument,
            "Expected InvalidArgument error"
        );
        assert_eq!(
            error.message(),
            "Submission for past slot.",
            "Error message mismatch"
        );
    }

    async fn test_execution_payload_none(client: &mut MevRelayClient<Channel>) {
        // Construct request with execution_payload set to None
        let request = Request::new(SubmitBlockRequest {
            bid_trace: Some(bid_trace(0, "1000".to_string())), // Assuming bid_trace() provides a valid object
            execution_payload: None,
            signature: vec![],
            blobs_bundle: Some(blobs_bundle()),
        });

        let response = client.submit_block(request).await;
        assert!(
            response.is_err(),
            "Expected an error when execution payload is None"
        );
        let error = response.err().unwrap();
        assert_eq!(
            error.code(),
            tonic::Code::InvalidArgument,
            "Expected InvalidArgument error"
        );
        assert_eq!(
            error.message(),
            "Missing Execution Payload",
            "Error message mismatch"
        );
    }

    async fn test_bid_none(client: &mut MevRelayClient<Channel>) {
        // Construct request with bid_trace set to None
        let request = Request::new(SubmitBlockRequest {
            bid_trace: None,
            execution_payload: Some(execution_payload(B256::random())), // Assuming execution_payload() provides a valid object
            signature: vec![],
            blobs_bundle: Some(blobs_bundle()),
        });

        let response = client.submit_block(request).await;
        assert!(
            response.is_err(),
            "Expected an error when bid trace is None"
        );
        let error = response.err().unwrap();
        assert_eq!(
            error.code(),
            tonic::Code::InvalidArgument,
            "Expected InvalidArgument error"
        );
        assert_eq!(error.message(), "Missing bid", "Error message mismatch");
    }

    async fn test_invalid_builder(client: &mut MevRelayClient<Channel>) {
        // Construct request with an invalid builder
        let mut bid = bid_trace(0, "1000".to_string());
        bid.builder_pubkey = BlsPublicKey::random().serialize().to_vec();
        let request = Request::new(SubmitBlockRequest {
            bid_trace: Some(bid),
            execution_payload: Some(execution_payload(B256::random())), // Assuming execution_payload() provides a valid object
            signature: vec![],
            blobs_bundle: Some(blobs_bundle()),
        });

        let response = client.submit_block(request).await;
        assert!(response.is_err(), "Expected an error for invalid builder");
        let error = response.err().unwrap();
        assert_eq!(
            error.code(),
            tonic::Code::InvalidArgument,
            "Expected InvalidArgument error"
        );
        assert_eq!(
            error.message(),
            "Invalid builder address.",
            "Error message mismatch"
        );
    }

    fn proposer_duties(slot: u64, fee_recipient: Address) -> Vec<Validator> {
        vec![Validator {
            slot,
            validator_index: 0,
            entry: validator_registration(fee_recipient),
        }]
    }

    fn validator_registration(fee_recipient: Address) -> ValidatorRegistration {
        let signature = BlsSignature::from_str(relay_primitives::test_utils::TEST_BLS_SIGNATURE).unwrap();

        ValidatorRegistration {
            signature,
            message: ValidatorRegistrationMessage {
                fee_recipient,
                gas_limit: 0,
                timestamp: 0,
                pubkey: BlsPublicKey::random(),
            },
        }
    }

    fn payload_attr(slot: u64, suggested_fee_recipient: Address) -> PayloadAttributesData {
        PayloadAttributesData {
            proposal_slot: slot,
            parent_block_root: B256::ZERO,
            parent_block_number: 0,
            parent_block_hash: B256::ZERO,
            proposer_index: 0,
            payload_attributes: PayloadAttributes {
                timestamp: 0,
                prev_randao: B256::ZERO,
                suggested_fee_recipient: *suggested_fee_recipient.as_ref(),
                withdrawals: Some(vec![]),
                parent_beacon_block_root: None,
            },
        }
    }

    fn bid_trace(slot: u64, value: String) -> BidTrace {
        let pubkey = *relay_primitives::test_utils::test_bls_pubkey();

        BidTrace {
            slot,
            parent_hash: vec![0; 32],
            block_hash: vec![0; 32],
            builder_pubkey: pubkey.serialize().to_vec(),
            proposer_pubkey: pubkey.serialize().to_vec(),
            proposer_fee_recipient: Address::zero().to_vec(),
            gas_limit: 0,
            gas_used: 0,
            value,
        }
    }

    fn execution_payload(prev_randao: B256) -> ExecutionPayload {
        ExecutionPayload {
            parent_hash: vec![0; 32],
            state_root: vec![0; 32],
            receipts_root: vec![0; 32],
            logs_bloom: vec![0; 256],
            prev_randao: prev_randao.to_vec(),
            extra_data: vec![0; 32],
            base_fee_per_gas: vec![0; 32],
            fee_recipient: Address::random().to_vec(),
            block_hash: vec![0; 32],
            transactions: vec![],
            withdrawals: vec![],
            block_number: 0,
            gas_limit: 0,
            timestamp: 0,
            gas_used: 0,
            blob_gas_used: 0,
            excess_blob_gas: 0,
        }
    }

    fn blobs_bundle() -> BlobsBundle {
        BlobsBundle {
            commitments: vec![],
            proofs: vec![],
            blobs: vec![],
        }
    }
}
